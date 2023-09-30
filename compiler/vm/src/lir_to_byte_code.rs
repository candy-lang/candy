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
    lir::{Bodies, Body, BodyId, Constant, ConstantId, Constants, Expression, Id, Lir},
    lir_optimize::OptimizeLir,
    module::Module,
    tracing::TracingConfig,
    utils::HashMapExtension,
};
use extension_trait::extension_trait;
use itertools::Itertools;
use rustc_hash::{FxHashMap, FxHashSet};
use std::{mem, sync::Arc};

pub fn compile_byte_code<Db>(
    db: &Db,
    module: Module,
    tracing: TracingConfig,
) -> (ByteCode, Arc<FxHashSet<CompilerError>>)
where
    Db: CstDb + OptimizeLir,
{
    #[allow(clippy::map_unwrap_or)]
    let (lir, errors) = db
        .optimized_lir(module.clone(), tracing)
        .map(|(lir, errors)| (lir, errors))
        .unwrap_or_else(|error| {
            let mut constants = Constants::default();
            let payload = CompilerErrorPayload::Module(error);
            let reason_id = constants.push(payload.to_string());
            let responsible_id = constants.push(hir::Id::user());

            let mut body = Body::new(
                FxHashSet::from_iter([hir::Id::new(module.clone(), vec![])]),
                0,
                0,
            );
            let reason_id = body.push(Expression::Constant(reason_id));
            let responsible_id = body.push(Expression::Constant(responsible_id));
            body.push(Expression::Panic {
                reason: reason_id,
                responsible: responsible_id,
            });

            let mut bodies = Bodies::default();
            bodies.push(body);

            let lir = Lir::new(constants, bodies);
            let errors = vec![CompilerError::for_whole_module(module.clone(), payload)]
                .into_iter()
                .collect();
            (Arc::new(lir), Arc::new(errors))
        });
    let byte_code = LoweringContext::compile(module, lir.as_ref());
    (byte_code, errors)
}

struct LoweringContext<'c> {
    lir: &'c Lir,
    byte_code: ByteCode,
    constant_mapping: FxHashMap<ConstantId, InlineObject>,
    body_mapping: FxHashMap<BodyId, InstructionPointer>,
    stack: Vec<Id>,
    instructions: Vec<Instruction>,
}
impl<'c> LoweringContext<'c> {
    fn compile(module: Module, lir: &Lir) -> ByteCode {
        let mut constant_heap = Heap::default();

        // The body instruction pointer of the module function will be changed
        // from zero to the correct one once the instructions are compiled.
        let module_function = Function::create(&mut constant_heap, false, &[], 0, 0.into());
        let responsible_module = HirId::create(
            &mut constant_heap,
            false,
            hir::Id::new(module.clone(), vec![]),
        );

        let byte_code = ByteCode {
            module,
            constant_heap,
            instructions: vec![],
            origins: vec![],
            module_function,
            responsible_module,
        };
        let mut context = LoweringContext {
            lir,
            byte_code,
            constant_mapping: FxHashMap::default(),
            body_mapping: FxHashMap::default(),
            stack: vec![],
            instructions: vec![],
        };

        let start = context.compile_body(BodyId::from_usize(0));
        module_function.set_body(start);

        context.byte_code
    }

    fn get_body(&mut self, body_id: BodyId) -> InstructionPointer {
        self.body_mapping
            .get(&body_id)
            .copied()
            .unwrap_or_else(|| self.compile_body(body_id))
    }
    fn compile_body(&mut self, body_id: BodyId) -> InstructionPointer {
        let old_stack = mem::take(&mut self.stack);
        let old_instructions = mem::take(&mut self.instructions);

        let body = self.lir.bodies().get(body_id);
        for captured in body.captured_ids() {
            self.stack.push(captured);
        }
        for parameter in body.parameter_ids() {
            self.stack.push(parameter);
        }
        self.stack.push(body.responsible_parameter_id());

        for (id, expression) in body.ids_and_expressions() {
            self.compile_expression(id, expression);
        }

        if matches!(self.instructions.last().unwrap(), Instruction::Call { .. },) {
            let Instruction::Call { num_args } = self.instructions.pop().unwrap() else {
                unreachable!()
            };
            self.instructions.push(Instruction::TailCall {
                num_locals_to_pop: self.stack.len() - 1,
                num_args,
            });
        } else {
            let dummy_id = Id::from_usize(0);
            self.emit(
                dummy_id,
                Instruction::PopMultipleBelowTop(self.stack.len() - 1),
            );
            self.emit(dummy_id, Instruction::Return);
        }

        let num_instructions = self.instructions.len();
        let start = self.byte_code.instructions.len().into();
        self.byte_code.instructions.append(&mut self.instructions);
        self.byte_code
            .origins
            .extend((0..num_instructions).map(|_| body.original_hirs().clone()));
        self.body_mapping.force_insert(body_id, start);

        self.stack = old_stack;
        self.instructions = old_instructions;

        start
    }

    fn compile_expression(&mut self, id: Id, expression: &Expression) {
        match expression {
            Expression::CreateTag { symbol, value } => {
                let symbol = self
                    .byte_code
                    .constant_heap
                    .default_symbols()
                    .get(symbol)
                    .unwrap_or_else(|| {
                        Text::create(&mut self.byte_code.constant_heap, false, symbol)
                    });

                self.emit_reference_to(*value);
                self.emit(id, Instruction::CreateTag { symbol });
            }
            Expression::CreateList(items) => {
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
            Expression::CreateStruct(fields) => {
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
            Expression::CreateFunction { captured, body_id } => {
                let instruction_pointer = self.get_body(*body_id);
                for captured in captured {
                    self.emit_reference_to(*captured);
                }
                self.emit(
                    id,
                    Instruction::CreateFunction {
                        captured: captured.iter().map(|id| self.stack.find_id(*id)).collect(),
                        num_args: self.lir.bodies().get(*body_id).parameter_count(),
                        body: instruction_pointer,
                    },
                );
            }
            Expression::Constant(constant_id) => {
                let value = self
                    .constant_mapping
                    .get(constant_id)
                    .copied()
                    .unwrap_or_else(|| self.compile_constant(*constant_id));
                self.emit(id, Instruction::PushConstant(value));
            }
            Expression::Reference(referenced) => {
                let offset = self.stack.find_id(*referenced);
                self.emit(id, Instruction::PushFromStack(offset));
            }
            Expression::Call {
                function,
                arguments,
                responsible,
            } => {
                self.emit_reference_to(*function);
                for argument in arguments {
                    self.emit_reference_to(*argument);
                }
                self.emit_reference_to(*responsible);
                self.emit(
                    id,
                    Instruction::Call {
                        num_args: arguments.len(),
                    },
                );
            }
            Expression::Panic {
                reason,
                responsible,
            } => {
                self.emit_reference_to(*reason);
                self.emit_reference_to(*responsible);
                self.emit(id, Instruction::Panic);
            }
            Expression::Dup {
                id: id_to_dup,
                amount,
            } => {
                self.emit_reference_to(*id_to_dup);
                self.emit(id, Instruction::Dup { amount: *amount });
            }
            Expression::Drop(id_to_drop) => {
                self.emit_reference_to(*id_to_drop);
                self.emit(id, Instruction::Drop);
            }
            Expression::TraceCallStarts {
                hir_call,
                function,
                arguments,
                responsible,
            } => {
                self.emit_reference_to(*hir_call);
                self.emit_reference_to(*function);
                for argument in arguments {
                    self.emit_reference_to(*argument);
                }
                self.emit_reference_to(*responsible);
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

    fn compile_constant(&mut self, id: ConstantId) -> InlineObject {
        let constant: InlineObject = match self.lir.constants().get(id) {
            Constant::Int(int) => {
                Int::create_from_bigint(&mut self.byte_code.constant_heap, false, int.clone())
                    .into()
            }
            Constant::Text(text) => {
                Text::create(&mut self.byte_code.constant_heap, false, text).into()
            }
            Constant::Tag { symbol, value } => {
                let symbol = Text::create(&mut self.byte_code.constant_heap, false, symbol);
                let value = value.map(|id| self.constant_mapping[&id]);
                Tag::create_with_value_option(
                    &mut self.byte_code.constant_heap,
                    false,
                    symbol,
                    value,
                )
                .into()
            }
            Constant::Builtin(builtin) => Builtin::create(*builtin).into(),
            Constant::List(items) => List::create(
                &mut self.byte_code.constant_heap,
                false,
                &items
                    .iter()
                    .map(|id| self.constant_mapping[id])
                    .collect_vec(),
            )
            .into(),
            Constant::Struct(fields) => Struct::create(
                &mut self.byte_code.constant_heap,
                false,
                &fields
                    .iter()
                    .map(|(key, value)| (self.constant_mapping[key], self.constant_mapping[value]))
                    .collect(),
            )
            .into(),
            Constant::HirId(hir_id) => {
                HirId::create(&mut self.byte_code.constant_heap, false, hir_id.clone()).into()
            }
            Constant::Function(body_id) => {
                let body = self.get_body(*body_id);
                Function::create(
                    &mut self.byte_code.constant_heap,
                    false,
                    &[],
                    self.lir.bodies().get(*body_id).parameter_count(),
                    body,
                )
                .into()
            }
        };
        self.constant_mapping.force_insert(id, constant);
        constant
    }

    fn emit_reference_to(&mut self, id: Id) {
        let offset = self.stack.find_id(id);
        self.emit(id, Instruction::PushFromStack(offset));
    }
    fn emit(&mut self, id: Id, instruction: Instruction) {
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
            .unwrap_or_else(|| panic!("Id {id} not found in stack: {}", self.iter().join(" ")))
    }
}
