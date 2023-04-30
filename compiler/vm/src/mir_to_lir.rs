use crate::{
    fiber::InstructionPointer,
    heap::{Builtin, Closure, Heap, HirId, Int, Tag, Text},
    lir::{Instruction, Lir, StackOffset},
};
use candy_frontend::{
    cst::CstDb,
    error::{CompilerError, CompilerErrorPayload},
    hir,
    id::CountableId,
    mir::{Body, Expression, Id, Mir},
    mir_optimize::OptimizeMir,
    module::Module,
    rich_ir::ToRichIr,
    tracing::TracingConfig,
};
use itertools::Itertools;
use rustc_hash::FxHashSet;
use std::sync::Arc;

pub fn compile_lir<Db>(
    db: &Db,
    module: Module,
    tracing: TracingConfig,
) -> (Arc<Lir>, Arc<FxHashSet<CompilerError>>)
where
    Db: CstDb + OptimizeMir,
{
    let (mir, errors) = db
        .optimized_mir(module.clone(), tracing)
        .unwrap_or_else(|error| {
            let payload = CompilerErrorPayload::Module(error);
            let mir = Mir::build(|body| {
                let reason = body.push_text(payload.to_string());
                let responsible = body.push_hir_id(hir::Id::user());
                body.push_panic(reason, responsible);
            });
            let errors = vec![CompilerError::for_whole_module(module.clone(), payload)]
                .into_iter()
                .collect();
            (Arc::new(mir), Arc::new(errors))
        });

    let mut constant_heap = Heap::default();
    let mut instructions = vec![];
    let start = compile_lambda(
        &mut constant_heap,
        &mut instructions,
        &FxHashSet::default(),
        &[],
        Id::from_usize(0),
        &mir.body,
    );

    let module_closure = Closure::create(&mut constant_heap, &[], 0, start);
    let responsible_module =
        HirId::create(&mut constant_heap, hir::Id::new(module.clone(), vec![]));

    let lir = Lir {
        module,
        constant_heap,
        instructions,
        module_closure,
        responsible_module,
    };
    (Arc::new(lir), errors)
}

fn compile_lambda(
    heap: &mut Heap,
    instructions: &mut Vec<Instruction>,
    captured: &FxHashSet<Id>,
    parameters: &[Id],
    responsible_parameter: Id,
    body: &Body,
) -> InstructionPointer {
    let mut context = LoweringContext::default();
    for captured in captured {
        context.stack.push(*captured);
    }
    for parameter in parameters {
        context.stack.push(*parameter);
    }
    context.stack.push(responsible_parameter);

    for (id, expression) in body.iter() {
        context.compile_expression(heap, instructions, id, expression);
    }

    if matches!(
        context.instructions.last().unwrap(),
        Instruction::Call { .. },
    ) {
        let Instruction::Call { num_args } = context.instructions.pop().unwrap() else { unreachable!() };
        context.instructions.push(Instruction::TailCall {
            num_locals_to_pop: context.stack.len() - 1,
            num_args,
        });
    } else {
        let dummy_id = Id::from_usize(0);
        context.emit(
            dummy_id,
            Instruction::PopMultipleBelowTop(context.stack.len() - 1),
        );
        context.emit(dummy_id, Instruction::Return);
    }

    let start = instructions.len().into();
    instructions.append(&mut context.instructions);
    start
}

#[derive(Default)]
struct LoweringContext {
    stack: Vec<Id>,
    instructions: Vec<Instruction>,
}
impl LoweringContext {
    fn compile_expression(
        &mut self,
        heap: &mut Heap,
        instructions: &mut Vec<Instruction>,
        id: Id,
        expression: &Expression,
    ) {
        match expression {
            Expression::Int(int) => {
                let int = Int::create_from_bigint(heap, int.clone());
                self.emit(id, Instruction::PushConstant(int.into()))
            }
            Expression::Text(text) => {
                let text = Text::create(heap, text);
                self.emit(id, Instruction::PushConstant(text.into()))
            }
            Expression::Reference(reference) => {
                self.emit_push_from_stack(*reference);
                self.stack.replace_top_id(id);
            }
            Expression::Symbol(symbol) => {
                let tag = Tag::create_from_str(heap, symbol, None);
                self.emit(id, Instruction::PushConstant(tag.into()));
            }
            Expression::Builtin(builtin) => {
                let builtin = Builtin::create(*builtin);
                self.emit(id, Instruction::PushConstant(builtin.into()));
            }
            Expression::List(items) => {
                for item in items {
                    self.emit_push_from_stack(*item);
                }
                self.emit(
                    id,
                    Instruction::CreateList {
                        num_items: items.len(),
                    },
                );
            }
            Expression::Struct(fields) => {
                for (key, value) in fields {
                    self.emit_push_from_stack(*key);
                    self.emit_push_from_stack(*value);
                }
                self.emit(
                    id,
                    Instruction::CreateStruct {
                        num_fields: fields.len(),
                    },
                );
            }
            Expression::HirId(hir_id) => {
                let hir_id = HirId::create(heap, hir_id.clone());
                self.emit(id, Instruction::PushConstant(hir_id.into()));
            }
            Expression::Lambda {
                original_hirs: _,
                parameters,
                responsible_parameter,
                body,
            } => {
                let captured = expression.captured_ids();
                let instructions = compile_lambda(
                    heap,
                    instructions,
                    &captured,
                    parameters,
                    *responsible_parameter,
                    body,
                );

                self.emit(
                    id,
                    Instruction::CreateClosure {
                        captured: captured
                            .iter()
                            .map(|id| self.stack.find_id(*id))
                            .collect_vec(),
                        num_args: parameters.len(),
                        body: instructions,
                    },
                );
            }
            Expression::Parameter => {
                panic!("The MIR should not contain any parameter expressions.")
            }
            Expression::Call {
                function,
                arguments,
                responsible,
            } => {
                self.emit_push_from_stack(*function);
                for argument in arguments {
                    self.emit_push_from_stack(*argument);
                }
                self.emit_push_from_stack(*responsible);
                self.emit(
                    id,
                    Instruction::Call {
                        num_args: arguments.len(),
                    },
                );
            }
            Expression::UseModule { .. } => {
                panic!("MIR still contains use. This should have been optimized out.");
            }
            Expression::Panic {
                reason,
                responsible,
            } => {
                self.emit_push_from_stack(*reason);
                self.emit_push_from_stack(*responsible);
                self.emit(id, Instruction::Panic);
            }
            Expression::Multiple(_) => {
                panic!("The MIR shouldn't contain multiple expressions anymore.");
            }
            Expression::TraceCallStarts {
                hir_call,
                function,
                arguments,
                responsible,
            } => {
                self.emit_push_from_stack(*hir_call);
                self.emit_push_from_stack(*function);
                for argument in arguments {
                    self.emit_push_from_stack(*argument);
                }
                self.emit_push_from_stack(*responsible);
                self.emit(
                    id,
                    Instruction::TraceCallStarts {
                        num_args: arguments.len(),
                    },
                );
            }
            Expression::TraceCallEnds { return_value } => {
                self.emit_push_from_stack(*return_value);
                self.emit(id, Instruction::TraceCallEnds);
            }
            Expression::TraceExpressionEvaluated {
                hir_expression,
                value,
            } => {
                self.emit_push_from_stack(*hir_expression);
                self.emit_push_from_stack(*value);
                self.emit(id, Instruction::TraceExpressionEvaluated);
            }
            Expression::TraceFoundFuzzableClosure {
                hir_definition,
                closure,
            } => {
                self.emit_push_from_stack(*hir_definition);
                self.emit_push_from_stack(*closure);
                self.emit(id, Instruction::TraceFoundFuzzableClosure);
            }
        }
    }

    fn emit_push_from_stack(&mut self, id: Id) {
        let offset = self.stack.find_id(id);
        self.emit(id, Instruction::PushFromStack(offset));
    }
    fn emit(&mut self, id: Id, instruction: Instruction) {
        instruction.apply_to_stack(&mut self.stack, id);
        self.instructions.push(instruction);
    }
}

trait StackExt {
    fn pop_multiple(&mut self, n: usize);
    fn find_id(&self, id: Id) -> StackOffset;
    fn replace_top_id(&mut self, id: Id);
}
impl StackExt for Vec<Id> {
    fn pop_multiple(&mut self, n: usize) {
        for _ in 0..n {
            self.pop();
        }
    }
    fn find_id(&self, id: Id) -> StackOffset {
        self.iter()
            .rev()
            .position(|it| *it == id)
            .unwrap_or_else(|| {
                panic!(
                    "Id {} not found in stack: {}",
                    id.to_rich_ir(),
                    self.iter().map(|it| it.to_rich_ir()).join(" "),
                )
            })
    }
    fn replace_top_id(&mut self, id: Id) {
        self.pop().unwrap();
        self.push(id);
    }
}
