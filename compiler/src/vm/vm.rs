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
    pub(super) stack: Vec<StackEntry>,
    stack_trace: Vec<hir::Id>,
    pub(super) heap: Heap,
}

#[derive(Clone)]
pub enum Status {
    Running,
    Done(Value),
    Panicked(Value),
}

/// Stack entries point either to instructions or to objects.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StackEntry {
    ByteCode(ByteCodePointer),
    Object(ObjectPointer),
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
            stack: vec![],
            stack_trace: vec![],
            heap: Heap::new(),
        }
    }

    pub fn status(&self) -> Status {
        self.status.clone()
    }

    fn get_from_stack(&self, offset: usize) -> StackEntry {
        self.stack[self.stack.len() - 1 - offset as usize].clone()
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

            {
                let function_stack = self.stack_trace.clone();
                let local_data_stack = self
                    .stack
                    .iter()
                    .rev()
                    .take_while(|entry| matches!(entry, StackEntry::Object(_)))
                    .collect_vec()
                    .into_iter()
                    .rev()
                    .collect_vec();
                debug!(
                    "Stack: {}{}",
                    function_stack.into_iter().join("..., "),
                    local_data_stack
                        .into_iter()
                        .map(|entry| match entry {
                            StackEntry::ByteCode(_) => unreachable!(),
                            StackEntry::Object(address) =>
                                format!(", {}", self.heap.export_without_dropping(*address)),
                        })
                        .join("")
                );
            }

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
                self.stack.push(StackEntry::Object(address));
            }
            Instruction::CreateText(text) => {
                let address = self.heap.create(ObjectData::Text(text));
                self.stack.push(StackEntry::Object(address));
            }
            Instruction::CreateSymbol(symbol) => {
                let address = self.heap.create(ObjectData::Symbol(symbol));
                self.stack.push(StackEntry::Object(address));
            }
            Instruction::CreateStruct { num_entries } => {
                let mut key_value_addresses = vec![];
                for _ in 0..(2 * num_entries) {
                    match self.stack.pop().unwrap() {
                        StackEntry::ByteCode(_) => panic!("Struct can only contain objects."),
                        StackEntry::Object(address) => key_value_addresses.push(address),
                    }
                }
                let mut entries = HashMap::new();
                for mut key_and_value in &key_value_addresses.into_iter().rev().chunks(2) {
                    let key = key_and_value.next().unwrap();
                    let value = key_and_value.next().unwrap();
                    assert_eq!(key_and_value.next(), None);
                    entries.insert(key, value);
                }
                let address = self.heap.create(ObjectData::Struct(entries));
                self.stack.push(StackEntry::Object(address));
            }
            Instruction::CreateClosure(chunk_index) => {
                let stack = self.stack.clone();
                for entry in &stack {
                    match entry {
                        StackEntry::ByteCode(_) => {}
                        StackEntry::Object(address) => {
                            self.heap.dup(*address);
                        }
                    }
                }
                let address = self.heap.create(ObjectData::Closure {
                    captured: stack,
                    body: chunk_index,
                });
                self.stack.push(StackEntry::Object(address));
            }
            Instruction::PopMultipleBelowTop(n) => {
                let top = self.stack.pop().unwrap();
                for _ in 0..n {
                    if let StackEntry::Object(address) = self.stack.pop().unwrap() {
                        self.heap.drop(address);
                    }
                }
                self.stack.push(top);
            }
            Instruction::PushFromStack(offset) => {
                let entry = self.get_from_stack(offset);
                if let StackEntry::Object(address) = &entry {
                    self.heap.dup(*address);
                }
                self.stack.push(entry);
            }
            Instruction::Call => {
                let closure_address = match self.stack.pop().unwrap() {
                    StackEntry::ByteCode(_) => panic!(),
                    StackEntry::Object(address) => address,
                };
                let (captured, body) = match &self.heap.get(closure_address).data {
                    ObjectData::Closure { captured, body } => (captured.clone(), *body),
                    _ => panic!("Can't call non-closure."),
                };
                let num_args = self.chunks[body].num_args;
                let mut args = vec![];
                for _ in 0..num_args {
                    let address = match self.stack.pop().unwrap() {
                        StackEntry::ByteCode(_) => {
                            panic!("You can only pass objects as arguments.")
                        }
                        StackEntry::Object(address) => address,
                    };
                    args.push(address);
                }
                self.stack.push(StackEntry::ByteCode(self.next_instruction));
                self.stack.append(&mut captured.clone());
                for arg in args {
                    self.stack.push(StackEntry::Object(arg));
                }
                self.heap.drop(closure_address);
                self.next_instruction = ByteCodePointer {
                    chunk: body,
                    instruction: 0,
                };
            }
            Instruction::Return => {
                let return_value = self.stack.pop().unwrap();
                let caller = match self.stack.pop().unwrap() {
                    StackEntry::ByteCode(address) => address,
                    StackEntry::Object(_) => panic!("Return with no caller on stack"),
                };
                self.stack.push(return_value);
                self.next_instruction = caller;
            }
            Instruction::Builtin(builtin_function) => {
                self.run_builtin_function(builtin_function);
            }
            Instruction::DebugValueEvaluated(_) => {}
            Instruction::DebugClosureEntered(hir_id) => self.stack_trace.push(hir_id),
            Instruction::DebugClosureExited => {
                self.stack_trace.pop().unwrap();
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
        self.stack_trace.clone()
    }
}
