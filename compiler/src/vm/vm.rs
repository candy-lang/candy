use super::{
    heap::{Heap, ObjectData, ObjectPointer},
    value::Value,
};
use crate::{
    compiler::{
        ast_to_hir::AstToHir,
        cst::CstDb,
        cst_to_ast::CstToAst,
        hir,
        lir::{Chunk, ChunkIndex, Instruction},
    },
    database::Database,
    input::Input,
    language_server::utils::LspPositionConversion,
};
use itertools::Itertools;
use log::{debug, error, trace};
use std::collections::HashMap;

/// A VM can execute some byte code.
pub struct Vm {
    pub chunks: Vec<Chunk>,
    pub status: Status,
    next_instruction: ByteCodePointer,
    pub heap: Heap,
    pub data_stack: Vec<ObjectPointer>,
    pub function_stack: Vec<ByteCodePointer>,
    pub debug_stack: Vec<DebugEntry>,
    pub fuzzable_closures: Vec<ObjectPointer>,
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

#[derive(Debug, Clone)]
pub struct DebugEntry {
    pub id: hir::Id,
    pub data_stack: Vec<Value>,
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
            trace!("Running instruction: {:?}", &instruction);
            self.next_instruction.instruction += 1;
            self.run_instruction(instruction);

            trace!(
                "Stack: {}",
                self.data_stack
                    .iter()
                    .map(|address| format!("{}", self.heap.export_without_dropping(*address)))
                    .join(", ")
            );
            trace!("Heap: {:?}", self.heap);

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

                        self.function_stack.push(self.next_instruction);
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
            Instruction::Return => {
                let caller = self.function_stack.pop().unwrap();
                self.next_instruction = caller;
            }
            Instruction::RegisterFuzzableClosure(_) => {
                let closure = self.data_stack.last().unwrap().clone();
                self.fuzzable_closures.push(closure);
            }
            Instruction::DebugValueEvaluated(id) => {
                let address = *self.data_stack.last().unwrap();
                let value = self.heap.export_without_dropping(address);
                debug!("{} = {}", id, value);
            }
            Instruction::DebugClosureEntered(id) => {
                let mut stack = vec![];
                for entry in &self.data_stack {
                    stack.push(self.heap.export_without_dropping(*entry));
                }

                self.debug_stack.push(DebugEntry {
                    id,
                    data_stack: stack,
                });
            }
            Instruction::DebugClosureExited => {
                let entry = self.debug_stack.pop().unwrap();
                trace!("Exited closure {}.", entry.id);
                trace!(
                    "Stack before: {:?}",
                    entry
                        .data_stack
                        .iter()
                        .map(|value| format!("{}", value))
                        .join(", ")
                );
                trace!(
                    "Stack after:  {:?}",
                    self.data_stack
                        .iter()
                        .map(|address| format!("{}", self.heap.export_without_dropping(*address)))
                        .join(", ")
                );
            }
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

    pub fn current_stack_trace(&self) -> Vec<DebugEntry> {
        self.debug_stack.clone()
    }
}

pub fn dump_panicked_vm(db: &Database, input: Input, vm: &Vm, value: Value) {
    error!("VM panicked: {:#?}", value);
    error!("Stack trace:");
    let (_, hir_to_ast_ids) = db.hir(input.clone()).unwrap();
    let (_, ast_to_cst_ids) = db.ast(input.clone()).unwrap();
    for entry in vm.current_stack_trace().into_iter().rev() {
        let hir_id = entry.id;
        let ast_id = hir_to_ast_ids[&hir_id].clone();
        let cst_id = ast_to_cst_ids[&ast_id];
        let cst = db.find_cst(input.clone(), cst_id);
        let start = db.offset_to_lsp(input.clone(), cst.span.start);
        let end = db.offset_to_lsp(input.clone(), cst.span.end);
        error!(
            "{}, {}, {:?}, {}:{} – {}:{}",
            hir_id, ast_id, cst_id, start.0, start.1, end.0, end.1
        );
    }
}
