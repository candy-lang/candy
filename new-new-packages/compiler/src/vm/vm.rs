use crate::{
    builtin_functions::BuiltinFunction,
    compiler::lir::{Chunk, ChunkIndex, Instruction},
};
use itertools::Itertools;
use log::debug;
use std::collections::HashMap;

/// A VM can execute some byte code.
#[derive(Debug)]
pub struct Vm {
    chunks: Vec<Chunk>,
    status: Status,
    ip: ByteCodePointer, // instruction pointer
    stack: Vec<StackEntry>,
    heap: HashMap<ObjectPointer, Object>,
    next_heap_address: ObjectPointer,
}

#[derive(Debug, Clone)]
pub enum Status {
    Running,
    Done(Object),
    Panicked(Object),
}

// TODO: Stack entries point either to instructions or to objects. In the
// future, we should unify both cases.
#[derive(Debug, Clone, Copy)]
pub enum StackEntry {
    ByteCode(ByteCodePointer),
    Object(ObjectPointer),
}
#[derive(Debug, Clone, Copy)]
pub struct ByteCodePointer {
    chunk: ChunkIndex,
    instruction: usize,
}
type ObjectPointer = usize;

// TODO: Later add a reference counter. For now, we're just leaking memory
// because objects are never freed.
#[derive(Debug, Clone)] // TODO: rm Clone
pub struct Object {
    reference_count: usize,
    data: ObjectData,
}
#[derive(Clone, Debug)] // TODO: rm Clone
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
            ip: ByteCodePointer {
                chunk: chunks.len() - 1,
                instruction: 0,
            },
            stack: vec![],
            heap: HashMap::new(),
            next_heap_address: 1000000,
        }
    }

    pub fn status(&self) -> Status {
        self.status.clone()
    }

    fn get_from_stack(&self, offset: usize) -> StackEntry {
        self.stack[self.stack.len() - 1 - offset as usize].clone()
    }
    fn get_from_heap(&mut self, address: ObjectPointer) -> &mut Object {
        self.heap.get_mut(&address).unwrap()
    }

    fn create_object(&mut self, object: ObjectData) -> ObjectPointer {
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
    fn free_object(&mut self, address: ObjectPointer) {
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
            ObjectData::Closure { captured, body } => {
                for entry in captured {
                    if let StackEntry::Object(address) = entry {
                        self.drop(address);
                    }
                }
            }
        }
    }

    fn dup(&mut self, address: ObjectPointer) {
        self.get_from_heap(address).reference_count += 1;
    }
    fn drop(&mut self, address: ObjectPointer) {
        let object = self.get_from_heap(address);
        object.reference_count -= 1;
        if object.reference_count == 0 {
            self.free_object(address);
        }
    }

    pub fn run(&mut self, mut num_instructions: u16) {
        assert!(
            matches!(self.status, Status::Running),
            "Called Vm::run on a vm that is not ready to run."
        );
        while matches!(self.status, Status::Running) && num_instructions > 0 {
            num_instructions -= 1;
            let instruction = self.chunks[self.ip.chunk].instructions[self.ip.instruction].clone();
            debug!("Executing instruction: {:?}", &instruction);
            self.ip.instruction += 1;
            self.run_instruction(instruction);

            if self.ip.instruction >= self.chunks[self.ip.chunk].instructions.len() {
                self.status = Status::Done(Object {
                    reference_count: 0,
                    data: ObjectData::Symbol("Nothing".to_string()),
                });
            }
        }
    }
    fn run_instruction(&mut self, instruction: Instruction) {
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
                self.stack.push(StackEntry::ByteCode(self.ip));
                self.stack.append(&mut captured.clone());
                for arg in args {
                    self.stack.push(StackEntry::Object(arg));
                }
                self.drop(closure_address);
                self.ip = ByteCodePointer {
                    chunk: body,
                    instruction: 0,
                };
            }
            Instruction::Return => {
                let return_value = self.stack.pop().unwrap();
                // TODO: The closure should take care of removing all captured
                // variables from the stack. That's only possible once we
                // analyze what variables are closed over in the first place.
                // For now, we simply pop until we find a byte code address,
                // which is the address we pushed to remember the caller.
                let caller = loop {
                    match self.stack.pop().unwrap() {
                        StackEntry::ByteCode(address) => break address,
                        StackEntry::Object(_) => {}
                    }
                };
                self.stack.push(return_value);
                self.ip = caller;
            }
            Instruction::Builtin(builtin) => {
                todo!("Implement builtins")
            }
            Instruction::DebugValueEvaluated(_) => {}
            Instruction::Error(_) => {
                // TODO: Fail gracefully.
                panic!("The VM crashed because there was an error in previous compilation stages.")
            }
        }
    }
}
