use super::{
    ast_to_hir::AstToHir,
    cst::CstDb,
    cst_to_ast::CstToAst,
    hir::{self, Body, Expression},
    lir::{Chunk, ChunkIndex, Instruction, Lir, StackOffset},
};
use crate::input::Input;
use std::{collections::HashMap, sync::Arc};

#[salsa::query_group(HirToLirStorage)]
pub trait HirToLir: CstDb + AstToHir {
    fn lir(&self, input: Input) -> Option<Arc<Lir>>;
}

fn lir(db: &dyn HirToLir, input: Input) -> Option<Arc<Lir>> {
    let (hir, _) = db.hir(input.clone())?;
    let mut context = LoweringContext::new();
    context.compile_body(&hir);
    assert_eq!(context.stack_size, 0); // TODO
    assert!(context.ids.values().all(|it| it.is_empty())); // TODO
    Some(Arc::new(context.lir))
}

struct LoweringContext {
    stack_size: usize,
    /// Maps ids to stack indices where it appears.
    ids: HashMap<hir::Id, Vec<usize>>,
    lir: Lir,
    current_chunk_index: ChunkIndex,
}
impl LoweringContext {
    fn new() -> LoweringContext {
        LoweringContext {
            stack_size: 0,
            ids: HashMap::new(),
            lir: Lir {
                chunks: vec![Chunk::new()],
            },
            current_chunk_index: 0,
        }
    }

    fn compile_body(&mut self, body: &Body) {
        for (id, expression) in &body.expressions {
            self.compile_expression(id, expression);
        }

        self.add(Instruction::PopMultipleBelowTop(body.expressions.len() - 1));
        for (id, expression) in &body.expressions {
            self.notify_stack_entry_removed(id);
        }
    }
    fn compile_expression(&mut self, id: &hir::Id, expression: &Expression) {
        match expression {
            Expression::Int(int) => self.add(Instruction::CreateInt(*int)),
            Expression::Text(text) => self.add(Instruction::CreateText(text.clone())),
            Expression::Reference(id) => self.push_from_stack(id),
            Expression::Symbol(symbol) => self.add(Instruction::CreateSymbol(symbol.clone())),
            Expression::Struct(entries) => {
                for (key, value) in entries {
                    self.push_from_stack(key);
                    self.push_from_stack(value);
                }
                self.add(Instruction::CreateStruct {
                    num_entries: entries.len(),
                });
                for (key, value) in entries {
                    self.notify_stack_entry_removed(key);
                    self.notify_stack_entry_removed(value);
                }
            }
            Expression::Lambda(lambda) => {
                let old_chunk_index = self.current_chunk_index;
                self.lir.chunks.push(Chunk::new());
                self.current_chunk_index = self.lir.chunks.len() - 1;
                let chunk_index = self.current_chunk_index;

                for (i, argument) in lambda.parameters.iter().enumerate() {
                    self.notify_new_stack_entry(&(lambda.first_id.clone() + i));
                }

                self.compile_body(&lambda.body);

                self.add(Instruction::PopMultipleBelowTop(lambda.parameters.len()));
                for (i, argument) in lambda.parameters.iter().enumerate() {
                    self.notify_stack_entry_removed(&(lambda.first_id.clone() + i));
                }

                self.add(Instruction::Return);

                self.current_chunk_index = old_chunk_index;

                self.add(Instruction::CreateClosure(chunk_index))
            }
            Expression::Body(body) => self.compile_body(body),
            Expression::Call {
                function,
                arguments,
            } => {
                let builtin_function = if let &[builtin_function_index] = &function.local[..] {
                    if let Some(builtin_function) =
                        crate::builtin_functions::VALUES.get(builtin_function_index)
                    {
                        Some(*builtin_function)
                    } else {
                        None
                    }
                } else {
                    None
                };

                for argument in arguments {
                    self.push_from_stack(argument);
                }

                if let Some(builtin_function) = builtin_function {
                    self.add(Instruction::Builtin(builtin_function));
                } else {
                    self.push_from_stack(function);
                    self.add(Instruction::Call);
                }
                for argument in arguments {
                    self.notify_stack_entry_removed(argument);
                }
            }
            Expression::Error { child, errors } => self.add(Instruction::Error(id.to_owned())),
        };
        self.notify_new_stack_entry(id);
        self.add(Instruction::DebugValueEvaluated(id.clone()));
    }

    fn push_from_stack(&mut self, id: &hir::Id) {
        self.add(Instruction::PushFromStack(self.find_in_stack(id)));
        self.notify_new_stack_entry(id);
    }
    fn notify_new_stack_entry(&mut self, id: &hir::Id) {
        self.ids
            .entry(id.to_owned())
            .or_insert_with(|| vec![])
            .push(self.stack_size);
        self.stack_size += 1;
    }
    fn notify_stack_entry_removed(&mut self, id: &hir::Id) {
        self.ids.get_mut(id).unwrap().pop().unwrap();
        self.stack_size -= 1;
    }
    fn find_in_stack(&self, id: &hir::Id) -> StackOffset {
        dbg!(self.stack_size - 1) - dbg!(self.ids[id].last().unwrap())
    }

    fn add(&mut self, instruction: Instruction) {
        let mut chunk = self.lir.chunks.get_mut(self.current_chunk_index).unwrap();
        chunk.instructions.push(instruction);
    }
}
