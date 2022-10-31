use super::{
    ast_to_hir::AstToHir,
    cst::CstDb,
    error::CompilerError,
    hir::{self, Body, Expression},
    lir::{Instruction, Lir, StackOffset},
};
use crate::{builtin_functions::BuiltinFunction, module::Module};
use itertools::Itertools;
use num_bigint::BigUint;
use std::sync::Arc;

#[salsa::query_group(HirToLirStorage)]
pub trait HirToLir: CstDb + AstToHir {
    fn lir(&self, module: Module) -> Option<Arc<Lir>>;
}

fn lir(db: &dyn HirToLir, module: Module) -> Option<Arc<Lir>> {
    let (hir, _) = db.hir(module)?;
    let instructions = compile_lambda(&[], &[], &hir);
    Some(Arc::new(Lir { instructions }))
}

fn compile_lambda(captured: &[hir::Id], parameters: &[hir::Id], body: &Body) -> Vec<Instruction> {
    let mut context = LoweringContext::default();
    for captured in captured {
        context.stack.push(captured.clone());
    }
    for parameter in parameters {
        context.stack.push(parameter.clone());
    }

    for (id, expression) in &body.expressions {
        context.compile_expression(id, expression);
    }

    context.emit_pop_multiple_below_top(body.expressions.len() - 1);
    context.emit_pop_multiple_below_top(parameters.len());
    context.emit_pop_multiple_below_top(captured.len());
    context.emit_return();

    assert_eq!(context.stack.len(), 1); // The stack should only contain the return value.

    context.instructions
}

#[derive(Default)]
struct LoweringContext {
    stack: Vec<hir::Id>,
    instructions: Vec<Instruction>,
}
impl LoweringContext {
    fn compile_expression(&mut self, id: &hir::Id, expression: &Expression) {
        match expression {
            Expression::Int(int) => self.emit_create_int(id.clone(), int.clone()),
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
                let captured = lambda.captured_ids(id);
                let instructions = compile_lambda(&captured, &lambda.parameters, &lambda.body);

                self.emit_create_closure(
                    id.clone(),
                    captured
                        .iter()
                        .map(|id| self.stack.find_id(id))
                        .collect_vec(),
                    lambda.parameters.len(),
                    instructions,
                    !lambda.fuzzable,
                );
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
                self.emit_start_responsibility(id.clone());
                self.emit_trace_call_starts(id.clone(), arguments.len());
                self.emit_call(id.clone(), arguments.len());
                self.emit_trace_call_ends();
                self.emit_end_responsibility();
            }
            Expression::Builtin(builtin) => {
                self.emit_create_builtin(id.clone(), *builtin);
            }
            Expression::UseModule {
                current_module,
                relative_path,
            } => {
                self.emit_push_from_stack(relative_path.clone());
                self.emit_use_module(id.clone(), current_module.clone());
            }
            Expression::Needs { condition, reason } => {
                self.emit_push_from_stack(condition.clone());
                self.emit_push_from_stack(reason.clone());
                self.emit_trace_needs_starts(id.clone());
                self.emit_needs(id.clone());
                self.emit_trace_needs_ends();
            }
            Expression::Error { errors, .. } => {
                self.emit_errors(id.clone(), errors.clone());
            }
        };
        self.emit_trace_value_evaluated(id.clone());
    }

    fn emit_create_int(&mut self, id: hir::Id, int: BigUint) {
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
    fn emit_create_closure(
        &mut self,
        id: hir::Id,
        captured: Vec<StackOffset>,
        num_args: usize,
        instructions: Vec<Instruction>,
        is_curly: bool,
    ) {
        self.emit(Instruction::CreateClosure {
            id: id.clone(),
            captured,
            num_args,
            body: instructions,
            is_curly,
        });
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
    fn emit_return(&mut self) {
        self.emit(Instruction::Return);
    }
    fn emit_use_module(&mut self, id: hir::Id, current_module: Module) {
        self.stack.pop(); // relative path
        self.emit(Instruction::UseModule { current_module });
        self.stack.push(id); // exported definitions
    }
    fn emit_start_responsibility(&mut self, responsible: hir::Id) {
        self.emit(Instruction::StartResponsibility(responsible));
    }
    fn emit_end_responsibility(&mut self) {
        self.emit(Instruction::EndResponsibility);
    }
    fn emit_needs(&mut self, id: hir::Id) {
        self.stack.pop(); // reason
        self.stack.pop(); // condition
        self.emit(Instruction::Needs);
        self.stack.push(id); // Nothing
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
    fn emit_errors(&mut self, id: hir::Id, errors: Vec<CompilerError>) {
        self.emit(Instruction::Error {
            id: id.clone(),
            errors,
        });
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
