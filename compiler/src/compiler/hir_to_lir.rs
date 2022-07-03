use super::{
    ast_to_hir::AstToHir,
    cst::CstDb,
    hir::{self, Body, Expression},
    lir::{Chunk, ChunkIndex, Instruction, Lir, StackOffset},
};
use crate::{builtin_functions::BuiltinFunction, input::Input};
use itertools::Itertools;
use std::{mem::swap, sync::Arc};

#[salsa::query_group(HirToLirStorage)]
pub trait HirToLir: CstDb + AstToHir {
    fn lir(&self, input: Input) -> Option<Arc<Lir>>;
}

fn lir(db: &dyn HirToLir, input: Input) -> Option<Arc<Lir>> {
    let (hir, _) = db.hir(input.clone())?;

    let mut context = LoweringContext::default();
    context.compile_body(&hir);
    let lir = context.finalize();

    Some(Arc::new(lir))
}

#[derive(Default)]
struct ChunkRegistry {
    chunks: Vec<Chunk>,
}
impl ChunkRegistry {
    fn register_chunk(&mut self, chunk: Chunk) -> ChunkIndex {
        let index = self.chunks.len();
        self.chunks.push(chunk);
        index
    }
}

#[derive(Default)]
struct LoweringContext {
    registry: ChunkRegistry,
    stack: Vec<hir::Id>,
    instructions: Vec<Instruction>,
}
impl LoweringContext {
    fn finalize(mut self) -> Lir {
        self.stack.pop().unwrap(); // Top-level has no return value.
        assert!(self.stack.is_empty());

        self.registry.register_chunk(Chunk {
            num_args: 0,
            instructions: self.instructions,
        });
        Lir {
            chunks: self.registry.chunks,
        }
    }

    fn compile_body(&mut self, body: &Body) {
        let stack_size_before = self.stack.len();
        for (id, expression) in &body.expressions {
            self.compile_expression(id, expression);
        }
        self.emit_pop_multiple_below_top(body.expressions.len() - 1);
        assert_eq!(self.stack.len(), stack_size_before + 1); // extra return value
    }
    fn compile_expression(&mut self, id: &hir::Id, expression: &Expression) {
        log::trace!("Stack: {:?}", self.stack);
        log::trace!("Compiling expression {expression:?}");

        match expression {
            Expression::Int(int) => self.emit_create_int(id.clone(), *int),
            Expression::Text(text) => self.emit_create_text(id.clone(), text.clone()),
            Expression::Reference(reference) => {
                self.emit_push_from_stack(reference.clone());
                self.stack.replace_top_id(id.clone());
            }
            Expression::Symbol(symbol) => self.emit_create_symbol(id.clone(), symbol.clone()),
            Expression::Struct(entries) => {
                for (key, value) in entries {
                    self.emit_push_from_stack(key.clone());
                    self.emit_push_from_stack(value.clone());
                }
                self.emit_create_struct(id.clone(), entries.len());
            }
            Expression::Lambda(lambda) => {
                let mut registry = ChunkRegistry::default();
                swap(&mut self.registry, &mut registry);
                let captured_stack = self.stack.clone();
                let mut lambda_context = LoweringContext {
                    registry,
                    stack: captured_stack.clone(),
                    instructions: vec![],
                };
                for parameter in &lambda.parameters {
                    lambda_context.stack.push(parameter.clone());
                }
                lambda_context.compile_body(&lambda.body);
                lambda_context.emit_pop_multiple_below_top(lambda.parameters.len());
                lambda_context.emit_pop_multiple_below_top(captured_stack.len());
                lambda_context.emit_return();
                swap(&mut self.registry, &mut lambda_context.registry);

                let lambda_chunk = Chunk {
                    num_args: lambda.parameters.len(),
                    instructions: lambda_context.instructions,
                };
                let chunk_index = self.registry.register_chunk(lambda_chunk);
                self.emit_create_closure(id.clone(), chunk_index);
                if lambda.fuzzable {
                    self.emit_register_fuzzable_closure(id.clone());
                }
            }
            Expression::Call {
                function,
                arguments,
            } => {
                for argument in arguments {
                    self.emit_push_from_stack(argument.clone());
                }

                self.emit_push_from_stack(function.clone());
                self.emit_trace_call_starts(id.clone(), arguments.len());
                self.emit_call(id.clone(), arguments.len());
                self.emit_trace_call_ends();
            }
            Expression::Builtin(builtin) => {
                self.emit_create_builtin(id.clone(), *builtin);
            }
            Expression::Needs { condition, message } => {
                self.emit_push_from_stack(*message.clone());
                self.emit_push_from_stack(*condition.clone());
                self.emit_trace_needs_starts(id.clone());
                self.emit_needs(id.clone());
                self.emit_trace_needs_ends();
            }
            Expression::Error { .. } => self.emit_error(id.to_owned()),
        };
        self.emit_trace_value_evaluated(id.clone());
    }

    fn emit_create_int(&mut self, id: hir::Id, int: u64) {
        self.emit(Instruction::CreateInt(int));
        self.stack.push(id);
    }
    fn emit_create_text(&mut self, id: hir::Id, text: String) {
        self.emit(Instruction::CreateText(text));
        self.stack.push(id);
    }
    fn emit_create_symbol(&mut self, id: hir::Id, symbol: String) {
        self.emit(Instruction::CreateSymbol(symbol));
        self.stack.push(id);
    }
    fn emit_create_struct(&mut self, id: hir::Id, num_entries: usize) {
        self.emit(Instruction::CreateStruct { num_entries });
        self.stack.pop_multiple(2 * num_entries);
        self.stack.push(id);
    }
    fn emit_create_closure(&mut self, id: hir::Id, chunk: usize) {
        self.emit(Instruction::CreateClosure(chunk));
        self.stack.push(id);
    }
    fn emit_create_builtin(&mut self, id: hir::Id, builtin: BuiltinFunction) {
        self.emit(Instruction::CreateBuiltin(builtin));
        self.stack.push(id);
    }
    fn emit_pop_multiple_below_top(&mut self, n: usize) {
        self.emit(Instruction::PopMultipleBelowTop(n));
        let top = self.stack.pop().unwrap();
        self.stack.pop_multiple(n);
        self.stack.push(top);
    }
    fn emit_push_from_stack(&mut self, id: hir::Id) {
        let offset = self.stack.find_id(&id);
        self.emit(Instruction::PushFromStack(offset));
        self.stack.push(id);
    }
    fn emit_call(&mut self, id: hir::Id, num_args: usize) {
        self.emit(Instruction::Call { num_args });
        self.stack.pop(); // closure/builtin
        self.stack.pop_multiple(num_args);
        self.stack.push(id);
    }
    fn emit_needs(&mut self, id: hir::Id) {
        self.stack.pop(); // condition
        self.stack.pop(); // message
        self.emit(Instruction::Needs);
        self.stack.push(id); // Nothing
    }
    fn emit_return(&mut self) {
        self.emit(Instruction::Return);
    }
    fn emit_register_fuzzable_closure(&mut self, id: hir::Id) {
        self.emit(Instruction::RegisterFuzzableClosure(id));
    }
    fn emit_trace_value_evaluated(&mut self, id: hir::Id) {
        self.emit(Instruction::TraceValueEvaluated(id));
    }
    fn emit_trace_call_starts(&mut self, id: hir::Id, num_args: usize) {
        self.emit(Instruction::TraceCallStarts { id, num_args });
    }
    fn emit_trace_call_ends(&mut self) {
        self.emit(Instruction::TraceCallEnds);
    }
    fn emit_trace_needs_starts(&mut self, id: hir::Id) {
        self.emit(Instruction::TraceNeedsStarts { id });
    }
    fn emit_trace_needs_ends(&mut self) {
        self.emit(Instruction::TraceNeedsEnds);
    }
    fn emit_error(&mut self, id: hir::Id) {
        self.emit(Instruction::Error(id.clone()));
        self.stack.push(id);
    }

    fn emit(&mut self, instruction: Instruction) {
        self.instructions.push(instruction);
    }
}

trait StackExt {
    fn pop_multiple(&mut self, n: usize);
    fn find_id(&self, id: &hir::Id) -> StackOffset;
    fn replace_top_id(&mut self, id: hir::Id);
}
impl StackExt for Vec<hir::Id> {
    fn pop_multiple(&mut self, n: usize) {
        for _ in 0..n {
            self.pop();
        }
    }
    fn find_id(&self, id: &hir::Id) -> StackOffset {
        self.iter()
            .rev()
            .position(|it| it == id)
            .unwrap_or_else(|| panic!("Id {} not found in stack: {}", id, self.iter().join(" ")))
    }
    fn replace_top_id(&mut self, id: hir::Id) {
        self.pop().unwrap();
        self.push(id);
    }
}
