use crate::{
    byte_code::{ByteCode, Instruction, StackOffset},
    heap::{Builtin, Function, Heap, HirId, InlineObject, Int, List, Struct, Tag, Text},
    instruction_pointer::InstructionPointer,
};
use candy_frontend::{
    cst::CstDb,
    error::{CompilerError, CompilerErrorPayload},
    hir,
    id::CountableId,
    mir::{Body, Expression, Id, Mir},
    mir_optimize::OptimizeMir,
    module::Module,
    tracing::TracingConfig,
};
use extension_trait::extension_trait;
use itertools::Itertools;
use rustc_hash::{FxHashMap, FxHashSet};
use std::sync::Arc;

pub fn compile_byte_code<Db>(
    db: &Db,
    module: Module,
    tracing: TracingConfig,
) -> (ByteCode, Arc<FxHashSet<CompilerError>>)
where
    Db: CstDb + OptimizeMir,
{
    #[allow(clippy::map_unwrap_or)]
    let (mir, errors) = db
        .optimized_mir(module.clone(), tracing)
        .map(|(mir, _, errors)| (mir, errors))
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

    // The body instruction pointer of the module function will be changed from
    // zero to the correct one once the instructions are compiled.
    let module_function = Function::create(&mut constant_heap, false, &[], 0, 0.into());
    let responsible_module = HirId::create(
        &mut constant_heap,
        false,
        hir::Id::new(module.clone(), vec![]),
    );

    let mut byte_code = ByteCode {
        module: module.clone(),
        constant_heap,
        instructions: vec![],
        origins: vec![],
        module_function,
        responsible_module,
    };

    let start = compile_function(
        &mut byte_code,
        &mut FxHashMap::default(),
        &FxHashSet::from_iter([hir::Id::new(module, vec![])]),
        &FxHashSet::default(),
        &[],
        &mir.body,
    );
    module_function.set_body(start);

    (byte_code, errors)
}

fn compile_function(
    byte_code: &mut ByteCode,
    constants: &mut FxHashMap<Id, InlineObject>,
    original_hirs: &FxHashSet<hir::Id>,
    captured: &FxHashSet<Id>,
    parameters: &[Id],
    body: &Body,
) -> InstructionPointer {
    let mut context = LoweringContext {
        byte_code,
        constants,
        stack: vec![],
        instructions: vec![],
    };
    for captured in captured {
        context.stack.push(*captured);
    }
    for parameter in parameters {
        context.stack.push(*parameter);
    }

    for (id, expression) in body.iter() {
        context.compile_expression(id, expression);
    }
    // Expressions may not push things onto the stack, but to the constant heap
    // instead.
    if context.stack.last() != Some(&body.return_value()) {
        context.emit_reference_to(body.return_value());
    }

    if matches!(
        context.instructions.last().unwrap(),
        Instruction::Call { .. },
    ) {
        let Instruction::Call { num_args } = context.instructions.pop().unwrap() else {
            unreachable!()
        };
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

    let mut instructions = context.instructions;
    let num_instructions = instructions.len();
    let start = byte_code.instructions.len().into();
    byte_code.instructions.append(&mut instructions);
    byte_code
        .origins
        .extend((0..num_instructions).map(|_| original_hirs.clone()));
    start
}

struct LoweringContext<'c> {
    byte_code: &'c mut ByteCode,
    constants: &'c mut FxHashMap<Id, InlineObject>,
    stack: Vec<Id>,
    instructions: Vec<Instruction>,
}
impl<'c> LoweringContext<'c> {
    fn compile_expression(&mut self, id: Id, expression: &Expression) {
        match expression {
            Expression::Int(int) => {
                let int =
                    Int::create_from_bigint(&mut self.byte_code.constant_heap, false, int.clone());
                self.constants.insert(id, int.into());
            }
            Expression::Text(text) => {
                let text = Text::create(&mut self.byte_code.constant_heap, false, text);
                self.constants.insert(id, text.into());
            }
            Expression::Reference(referenced) => {
                if let Some(&constant) = self.constants.get(referenced) {
                    self.constants.insert(id, constant);
                } else {
                    let offset = self.stack.find_id(*referenced);
                    self.emit(id, Instruction::PushFromStack(offset));
                }
            }
            Expression::Tag { symbol, value } => {
                let symbol = self
                    .byte_code
                    .constant_heap
                    .default_symbols()
                    .get(symbol)
                    .unwrap_or_else(|| {
                        Text::create(&mut self.byte_code.constant_heap, false, symbol)
                    });

                if let Some(value) = value {
                    if let Some(value) = self.constants.get(value) {
                        let tag = Tag::create_with_value(
                            &mut self.byte_code.constant_heap,
                            false,
                            symbol,
                            *value,
                        );
                        self.constants.insert(id, tag.into());
                    } else {
                        self.emit_reference_to(*value);
                        self.emit(id, Instruction::CreateTag { symbol });
                    }
                } else {
                    let tag = Tag::create(symbol);
                    self.constants.insert(id, tag.into());
                }
            }
            Expression::Builtin(builtin) => {
                let builtin = Builtin::create(*builtin);
                self.constants.insert(id, builtin.into());
            }
            Expression::List(items) => {
                if let Some(items) = items
                    .iter()
                    .map(|item| self.constants.get(item).copied())
                    .collect::<Option<Vec<_>>>()
                {
                    let list = List::create(&mut self.byte_code.constant_heap, false, &items);
                    self.constants.insert(id, list.into());
                } else {
                    for item in items {
                        self.emit_reference_to(*item);
                    }
                    self.emit(
                        id,
                        Instruction::CreateList {
                            num_items: items.len(),
                        },
                    );
                }
            }
            Expression::Struct(fields) => {
                if let Some(fields) = fields
                    .iter()
                    .map(|(key, value)| try {
                        (*self.constants.get(key)?, *self.constants.get(value)?)
                    })
                    .collect::<Option<FxHashMap<_, _>>>()
                {
                    let struct_ = Struct::create(&mut self.byte_code.constant_heap, false, &fields);
                    self.constants.insert(id, struct_.into());
                } else {
                    for (key, value) in fields {
                        self.emit_reference_to(*key);
                        self.emit_reference_to(*value);
                    }
                    self.emit(
                        id,
                        Instruction::CreateStruct {
                            num_fields: fields.len(),
                        },
                    );
                }
            }
            Expression::HirId(hir_id) => {
                let hir_id =
                    HirId::create(&mut self.byte_code.constant_heap, false, hir_id.clone());
                self.constants.insert(id, hir_id.into());
            }
            Expression::Function {
                original_hirs,
                parameters,
                body,
            } => {
                let captured = expression
                    .captured_ids()
                    .into_iter()
                    .filter(|captured| !self.constants.contains_key(captured))
                    .collect();

                let instructions = compile_function(
                    self.byte_code,
                    self.constants,
                    original_hirs,
                    &captured,
                    parameters,
                    body,
                );

                if captured.is_empty() {
                    let function = Function::create(
                        &mut self.byte_code.constant_heap,
                        false,
                        &[],
                        parameters.len(),
                        instructions,
                    );
                    self.constants.insert(id, function.into());
                } else {
                    for captured in &captured {
                        self.emit_reference_to(*captured);
                    }
                    self.emit(
                        id,
                        Instruction::CreateFunction {
                            captured: captured
                                .iter()
                                .map(|id| self.stack.find_id(*id))
                                .collect_vec(),
                            num_args: parameters.len(),
                            body: instructions,
                        },
                    );
                }
            }
            Expression::Parameter => {
                panic!("The MIR should not contain any parameter expressions.")
            }
            Expression::Call {
                function,
                arguments,
            } => {
                self.emit_reference_to(*function);
                for argument in arguments {
                    self.emit_reference_to(*argument);
                }
                self.emit(
                    id,
                    Instruction::Call {
                        num_args: arguments.len(),
                    },
                );
            }
            Expression::UseModule { .. } => {
                // Calls of the use function are completely inlined and, if
                // they're not statically known, are replaced by panics.
                // The only way a use can still be in the MIR is if the tracing
                // of evaluated expressions is enabled. We can emit any nonsense
                // here, since the instructions will never be executed anyway.
                // We just push an empty struct, as if the imported module
                // hadn't exported anything.
                self.emit(id, Instruction::CreateStruct { num_fields: 0 });
            }
            Expression::Panic {
                reason,
                responsible,
            } => {
                self.emit_reference_to(*reason);
                self.emit_reference_to(*responsible);
                self.emit(id, Instruction::Panic);
            }
            Expression::TraceCallStarts {
                hir_call,
                function,
                arguments,
            } => {
                self.emit_reference_to(*hir_call);
                self.emit_reference_to(*function);
                for argument in arguments {
                    self.emit_reference_to(*argument);
                }
                self.emit(
                    id,
                    Instruction::TraceCallStarts {
                        num_args: arguments.len(),
                    },
                );
            }
            Expression::TraceCallEnds { return_value } => {
                self.emit_reference_to(*return_value);
                self.emit(id, Instruction::TraceCallEnds);
            }
            Expression::TraceExpressionEvaluated {
                hir_expression,
                value,
            } => {
                self.emit_reference_to(*hir_expression);
                self.emit_reference_to(*value);
                self.emit(id, Instruction::TraceExpressionEvaluated);
            }
            Expression::TraceFoundFuzzableFunction {
                hir_definition,
                function,
            } => {
                self.emit_reference_to(*hir_definition);
                self.emit_reference_to(*function);
                self.emit(id, Instruction::TraceFoundFuzzableFunction);
            }
        }
    }

    fn emit_reference_to(&mut self, id: Id) {
        if let Some(constant) = self.constants.get(&id) {
            self.emit(id, Instruction::PushConstant(*constant));
        } else {
            let offset = self.stack.find_id(id);
            self.emit(id, Instruction::PushFromStack(offset));
        }
    }
    fn emit(&mut self, id: Id, instruction: Instruction) {
        if matches!(instruction, Instruction::PopMultipleBelowTop(0)) {
            return;
        }
        instruction.apply_to_stack(&mut self.stack, id);
        self.instructions.push(instruction);
    }
}

#[extension_trait]
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
            .unwrap_or_else(|| panic!("Id {id} not found in stack: {}", self.iter().join(" "),))
    }
}
