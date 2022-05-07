use crate::compiler::{
    hir,
    lir::{Chunk, ChunkIndex, Instruction},
};
use itertools::Itertools;
use log::debug;
use std::collections::HashMap;

use super::value::Value;

/// A VM can execute some byte code.
pub struct Vm {
    pub(super) chunks: Vec<Chunk>,
    pub(super) status: Status,
    next_instruction: ByteCodePointer,
    pub(super) stack: Vec<StackEntry>,
    heap: HashMap<ObjectPointer, Object>,
    next_heap_address: ObjectPointer,
    stack_trace: Vec<hir::Id>,
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
pub type ObjectPointer = usize;

#[derive(Debug, Clone)] // TODO: remove Clone once it's no longer needed
pub struct Object {
    reference_count: usize,
    data: ObjectData,
}
#[derive(Clone, Debug)] // TODO: remove Clone once it's no longer needed
pub enum ObjectData {
    Int(u64),
    Text(String),
    Symbol(String),
    Struct(HashMap<ObjectPointer, ObjectPointer>),
    Closure {
        // TODO: This could later be just a vector of object pointers, but for
        // now we capture the whole stack.
        captured: Vec<StackEntry>,
        body: ChunkIndex,
    },
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
            heap: HashMap::new(),
            next_heap_address: 0,
            stack_trace: vec![],
        }
    }

    pub fn status(&self) -> Status {
        self.status.clone()
    }

    fn get_from_stack(&self, offset: usize) -> StackEntry {
        self.stack[self.stack.len() - 1 - offset as usize].clone()
    }
    fn get_from_heap(&self, address: ObjectPointer) -> &Object {
        self.heap.get(&address).unwrap()
    }
    fn get_mut_from_heap(&mut self, address: ObjectPointer) -> &mut Object {
        self.heap.get_mut(&address).unwrap()
    }

    pub fn create_object(&mut self, object: ObjectData) -> ObjectPointer {
        let address = self.next_heap_address;
        self.heap.insert(
            address,
            Object {
                reference_count: 1,
                data: object,
            },
        );
        self.next_heap_address += 1;
        address
    }
    pub fn free_object(&mut self, address: ObjectPointer) {
        let object = self.heap.remove(&address).unwrap();
        assert_eq!(object.reference_count, 0);
        match object.data {
            ObjectData::Int(_) => {}
            ObjectData::Text(_) => {}
            ObjectData::Symbol(_) => {}
            ObjectData::Struct(entries) => {
                for (key, value) in entries {
                    self.drop(key);
                    self.drop(value);
                }
            }
            ObjectData::Closure { captured, .. } => {
                for entry in captured {
                    if let StackEntry::Object(address) = entry {
                        self.drop(address);
                    }
                }
            }
        }
    }

    fn dup(&mut self, address: ObjectPointer) {
        self.get_mut_from_heap(address).reference_count += 1;
    }
    fn drop(&mut self, address: ObjectPointer) {
        let object = self.get_mut_from_heap(address);
        object.reference_count -= 1;
        if object.reference_count == 0 {
            self.free_object(address);
        }
    }

    pub(super) fn import(&mut self, value: Value) -> ObjectPointer {
        let value = match value {
            Value::Int(int) => ObjectData::Int(int),
            Value::Text(text) => ObjectData::Text(text),
            Value::Symbol(symbol) => ObjectData::Symbol(symbol),
            Value::Struct(struct_) => {
                let mut entries = HashMap::new();
                for (key, value) in struct_ {
                    let key = self.import(key);
                    let value = self.import(value);
                    entries.insert(key, value);
                }
                ObjectData::Struct(entries)
            }
            Value::Closure { captured, body } => ObjectData::Closure { captured, body },
        };
        self.create_object(value)
    }
    pub(super) fn export(&mut self, address: ObjectPointer) -> Value {
        let value = self.export_helper(address);
        self.drop(address);
        value
    }
    fn export_helper(&self, address: ObjectPointer) -> Value {
        match &self.get_from_heap(address).data {
            ObjectData::Int(int) => Value::Int(*int),
            ObjectData::Text(text) => Value::Text(text.clone()),
            ObjectData::Symbol(symbol) => Value::Symbol(symbol.clone()),
            ObjectData::Struct(struct_) => {
                let mut entries = im::HashMap::new();
                for (key, value) in struct_ {
                    let key = self.export_helper(*key);
                    let value = self.export_helper(*value);
                    entries.insert(key, value);
                }
                Value::Struct(entries)
            }
            ObjectData::Closure { captured, body } => Value::Closure {
                captured: captured.clone(),
                body: *body,
            },
        }
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
                "Stack: {}",
                self.stack
                    .iter()
                    .map(|entry| match entry {
                        StackEntry::ByteCode(byte_code) => {
                            format!("here#{}:{}", byte_code.chunk, byte_code.instruction)
                        }
                        StackEntry::Object(address) => format!("{}", self.export_helper(*address)),
                    })
                    .join(", ")
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
                let address = self.create_object(ObjectData::Int(int));
                self.stack.push(StackEntry::Object(address));
            }
            Instruction::CreateText(text) => {
                let address = self.create_object(ObjectData::Text(text));
                self.stack.push(StackEntry::Object(address));
            }
            Instruction::CreateSymbol(symbol) => {
                let address = self.create_object(ObjectData::Symbol(symbol));
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
                let address = self.create_object(ObjectData::Struct(entries));
                self.stack.push(StackEntry::Object(address));
            }
            Instruction::CreateClosure(chunk_index) => {
                let stack = self.stack.clone();
                for entry in &stack {
                    match entry {
                        StackEntry::ByteCode(_) => {}
                        StackEntry::Object(address) => {
                            self.dup(*address);
                        }
                    }
                }
                let address = self.create_object(ObjectData::Closure {
                    captured: stack,
                    body: chunk_index,
                });
                self.stack.push(StackEntry::Object(address));
            }
            Instruction::PopMultipleBelowTop(n) => {
                let top = self.stack.pop().unwrap();
                for _ in 0..n {
                    if let StackEntry::Object(address) = self.stack.pop().unwrap() {
                        self.drop(address);
                    }
                }
                self.stack.push(top);
            }
            Instruction::PushFromStack(offset) => {
                let entry = self.get_from_stack(offset);
                if let StackEntry::Object(address) = &entry {
                    self.dup(*address);
                }
                self.stack.push(entry);
            }
            Instruction::Call => {
                let closure_address = match self.stack.pop().unwrap() {
                    StackEntry::ByteCode(_) => panic!(),
                    StackEntry::Object(address) => address,
                };
                let (captured, body) = match &self.get_from_heap(closure_address).data {
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
                self.drop(closure_address);
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
