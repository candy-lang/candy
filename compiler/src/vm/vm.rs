use super::{
    heap::{Heap, ObjectData, ObjectPointer},
    tracer::{TraceEntry, Tracer},
    value::Value,
};
use crate::compiler::{
    hir::Id,
    lir::{Chunk, ChunkIndex, Instruction},
};
use itertools::Itertools;
use log;
use std::collections::HashMap;

/// A VM can execute some byte code.
#[derive(Clone)]
pub struct Vm {
    pub chunks: Vec<Chunk>,
    pub status: Status,
    next_instruction: ByteCodePointer,
    pub heap: Heap,
    pub data_stack: Vec<ObjectPointer>,
    pub call_stack: Vec<ByteCodePointer>,
    pub tracer: Tracer,
    pub fuzzable_closures: Vec<(Id, ObjectPointer)>,
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
            call_stack: vec![],
            tracer: Tracer::default(),
            fuzzable_closures: vec![],
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
            log::trace!("Running instruction: {:?}", &instruction);
            self.next_instruction.instruction += 1;
            self.run_instruction(instruction);

            log::trace!(
                "Stack: {}",
                self.data_stack
                    .iter()
                    .map(|address| format!("{}", self.heap.export_without_dropping(*address)))
                    .join(", ")
            );
            log::trace!("Heap: {:?}", self.heap);

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
            Instruction::CreateClosure(chunk) => {
                let captured = self.data_stack.clone();
                for address in &captured {
                    self.heap.dup(*address);
                }
                let address = self.heap.create(ObjectData::Closure {
                    captured,
                    body: chunk,
                });
                self.data_stack.push(address);
            }
            Instruction::CreateBuiltin(builtin) => {
                let address = self.heap.create(ObjectData::Builtin(builtin));
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
            Instruction::Call { num_args } => {
                let closure_address = self.data_stack.pop().unwrap();
                let mut args = vec![];
                for _ in 0..num_args {
                    args.push(self.data_stack.pop().unwrap());
                }
                args.reverse();

                match self.heap.get(closure_address).data.clone() {
                    ObjectData::Closure { captured, body } => {
                        let expected_num_args = self.chunks[body].num_args;
                        if num_args != expected_num_args {
                            self.panic(format!("Closure expects {} parameters, but you called it with {} arguments.", expected_num_args, num_args));
                            return;
                        }

                        self.call_stack.push(self.next_instruction);
                        self.data_stack.append(&mut captured.clone());
                        for captured in captured {
                            self.heap.dup(captured);
                        }
                        self.data_stack.append(&mut args);
                        self.heap.drop(closure_address);
                        self.next_instruction = ByteCodePointer {
                            chunk: body,
                            instruction: 0,
                        };
                    }
                    ObjectData::Builtin(builtin) => {
                        self.run_builtin_function(&builtin, &args);
                    }
                    _ => panic!("Can only call closures and builtins."),
                };
            }
            Instruction::Needs => {
                let condition = self.data_stack.pop().unwrap();
                let message = self.data_stack.pop().unwrap();

                match self.heap.get(condition).data.clone() {
                    ObjectData::Symbol(symbol) => match symbol.as_str() {
                        "True" => {
                            self.data_stack.push(self.heap.import(Value::nothing()));
                        }
                        "False" => {
                            self.status =
                                Status::Panicked(self.heap.export_without_dropping(message))
                        }
                        _ => {
                            self.panic(format!("Needs expects True or False as a symbol."));
                        }
                    },
                    _ => {
                        self.panic(format!("Needs expects a boolean symbol."));
                    }
                }
            }
            Instruction::Return => {
                let caller = self.call_stack.pop().unwrap();
                self.next_instruction = caller;
            }
            Instruction::RegisterFuzzableClosure(id) => {
                let closure = self.data_stack.last().unwrap().clone();
                self.fuzzable_closures.push((id, closure));
            }
            Instruction::TraceValueEvaluated(id) => {
                let address = *self.data_stack.last().unwrap();
                let value = self.heap.export_without_dropping(address);
                self.tracer.push(TraceEntry::ValueEvaluated { id, value });
            }
            Instruction::TraceCallStarts { id, num_args } => {
                let closure_address = self.data_stack.last().unwrap();
                let closure = self.heap.export_without_dropping(*closure_address);

                let mut args = vec![];
                let stack_size = self.data_stack.len();
                for i in 0..num_args {
                    let address = self.data_stack[stack_size - i - 2];
                    let argument = self.heap.export_without_dropping(address);
                    args.push(argument);
                }
                args.reverse();

                self.tracer
                    .push(TraceEntry::CallStarted { id, closure, args });
            }
            Instruction::TraceCallEnds => {
                let return_value_address = self.data_stack.last().unwrap();
                let return_value = self.heap.export_without_dropping(*return_value_address);

                self.tracer.push(TraceEntry::CallEnded { return_value });
            }
            Instruction::TraceNeedsStarts { id } => {
                let condition = self.data_stack[self.data_stack.len() - 1];
                let message = self.data_stack[self.data_stack.len() - 2];
                let condition = self.heap.export_without_dropping(condition);
                let message = self.heap.export_without_dropping(message);
                self.tracer.push(TraceEntry::NeedsStarted {
                    id,
                    condition,
                    message,
                });
            }
            Instruction::TraceNeedsEnds => self.tracer.push(TraceEntry::NeedsEnded),
            Instruction::Error(error) => {
                self.panic(
                    format!("The VM crashed because there was an error in previous compilation stages: {:?}", error),
                );
            }
        }
    }

    pub fn panic(&mut self, message: String) -> Value {
        self.status = Status::Panicked(Value::Text(message));
        Value::Symbol("Never".to_string())
    }

    pub fn run_closure(&mut self, closure_address: ObjectPointer, arguments: Vec<Value>) {
        let num_args = arguments.len();
        for arg in arguments {
            let address = self.heap.import(arg);
            self.data_stack.push(address);
        }
        self.data_stack.push(closure_address);
        self.run_instruction(Instruction::Call { num_args });

        self.status = Status::Running;
    }
}
