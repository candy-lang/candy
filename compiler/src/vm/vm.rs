use super::{
    heap::{Heap, ObjectData, ObjectPointer},
    value::Value,
};
use crate::compiler::{
    hir,
    lir::{Chunk, ChunkIndex, Instruction},
};
use itertools::Itertools;
use log::debug;
use std::collections::HashMap;

/// A VM can execute some byte code.
pub struct Vm {
    pub(super) chunks: Vec<Chunk>,
    pub(super) status: Status,
    next_instruction: ByteCodePointer,
    pub(super) heap: Heap,
    pub(super) data_stack: Vec<ObjectPointer>,
    pub(super) function_stack: Vec<ByteCodePointer>,
    pub(super) debug_stack: Vec<hir::Id>,
}

#[derive(Clone)]
pub enum Status {
    Running,
    Done(Value),
    Panicked(Value),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ByteCodePointer {
    chunk: ChunkIndex,
    instruction: usize,
}

impl Vm {
    pub fn new(chunks: Vec<Chunk>) -> Self {
        Self {
            chunks: chunks.clone(),
            status: Status::Running,
            next_instruction: ByteCodePointer {
                chunk: chunks.len() - 1,
                instruction: 0,
            },
            heap: Heap::new(),
            data_stack: vec![],
            function_stack: vec![],
            debug_stack: vec![],
        }
    }

    pub fn status(&self) -> Status {
        self.status.clone()
    }

    fn get_from_data_stack(&self, offset: usize) -> ObjectPointer {
        self.data_stack[self.data_stack.len() - 1 - offset as usize].clone()
    }

    pub fn run(&mut self, mut num_instructions: u16) {
        assert!(
            matches!(self.status, Status::Running),
            "Called Vm::run on a vm that is not ready to run."
        );
        while matches!(self.status, Status::Running) && num_instructions > 0 {
            num_instructions -= 1;
            let instruction = self.chunks[self.next_instruction.chunk].instructions
                [self.next_instruction.instruction]
                .clone();
            debug!("Executing instruction: {:?}", &instruction);
            self.next_instruction.instruction += 1;
            self.run_instruction(instruction);

            debug!(
                "Stack: {}{}",
                self.debug_stack.iter().join("..., "),
                self.data_stack
                    .iter()
                    .map(|address| format!(", {}", self.heap.export_without_dropping(*address)))
                    .join("")
            );

            if self.next_instruction.instruction
                >= self.chunks[self.next_instruction.chunk].instructions.len()
            {
                self.status = Status::Done(Value::nothing());
            }
        }
    }
    pub fn run_instruction(&mut self, instruction: Instruction) {
        match instruction {
            Instruction::CreateInt(int) => {
                let address = self.heap.create(ObjectData::Int(int));
                self.data_stack.push(address);
            }
            Instruction::CreateText(text) => {
                let address = self.heap.create(ObjectData::Text(text));
                self.data_stack.push(address);
            }
            Instruction::CreateSymbol(symbol) => {
                let address = self.heap.create(ObjectData::Symbol(symbol));
                self.data_stack.push(address);
            }
            Instruction::CreateStruct { num_entries } => {
                let mut key_value_addresses = vec![];
                for _ in 0..(2 * num_entries) {
                    key_value_addresses.push(self.data_stack.pop().unwrap());
                }
                let mut entries = HashMap::new();
                for mut key_and_value in &key_value_addresses.into_iter().rev().chunks(2) {
                    let key = key_and_value.next().unwrap();
                    let value = key_and_value.next().unwrap();
                    assert_eq!(key_and_value.next(), None);
                    entries.insert(key, value);
                }
                let address = self.heap.create(ObjectData::Struct(entries));
                self.data_stack.push(address);
            }
            Instruction::CreateClosure(chunk_index) => {
                let stack = self.data_stack.clone();
                for address in &stack {
                    self.heap.dup(*address);
                }
                let address = self.heap.create(ObjectData::Closure {
                    captured: stack,
                    body: chunk_index,
                });
                self.data_stack.push(address);
            }
            Instruction::PopMultipleBelowTop(n) => {
                let top = self.data_stack.pop().unwrap();
                for _ in 0..n {
                    let address = self.data_stack.pop().unwrap();
                    self.heap.drop(address);
                }
                self.data_stack.push(top);
            }
            Instruction::PushFromStack(offset) => {
                let address = self.get_from_data_stack(offset);
                self.heap.dup(address);
                self.data_stack.push(address);
            }
            Instruction::Call => {
                let closure_address = self.data_stack.pop().unwrap();
                let (captured, body) = match &self.heap.get(closure_address).data {
                    ObjectData::Closure { captured, body } => (captured.clone(), *body),
                    _ => panic!("Can't call non-closure."),
                };
                let num_args = self.chunks[body].num_args;
                let mut args = vec![];
                for _ in 0..num_args {
                    args.push(self.data_stack.pop().unwrap());
                }
                self.function_stack.push(self.next_instruction);
                self.data_stack.append(&mut captured.clone());
                self.data_stack.append(&mut args);
                self.heap.drop(closure_address);
                self.next_instruction = ByteCodePointer {
                    chunk: body,
                    instruction: 0,
                };
            }
            Instruction::Return => {
                let caller = self.function_stack.pop().unwrap();
                self.next_instruction = caller;
            }
            Instruction::Builtin(builtin_function) => {
                self.run_builtin_function(builtin_function);
            }
            Instruction::DebugValueEvaluated(_) => {}
            Instruction::DebugClosureEntered(hir_id) => self.debug_stack.push(hir_id),
            Instruction::DebugClosureExited => {
                self.debug_stack.pop().unwrap();
            }
            Instruction::Error(_) => {
                self.panic(
                    "The VM crashed because there was an error in previous compilation stages."
                        .to_string(),
                );
            }
        }
    }

    pub fn panic(&mut self, message: String) -> Value {
        self.status = Status::Panicked(Value::Text(message));
        Value::Symbol("Never".to_string())
    }

    pub fn current_stack_trace(&self) -> Vec<hir::Id> {
        self.debug_stack.clone()
    }
}
