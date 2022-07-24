use super::{
    heap::{Heap, ObjectData, ObjectPointer},
    tracer::{TraceEntry, Tracer},
    use_provider::UseProvider,
    value::{Closure, Value},
};
use crate::{
    compiler::{hir::Id, lir::Instruction},
    input::Input,
};
use itertools::Itertools;
use log;
use std::{collections::HashMap, mem};

/// A VM can execute some byte code.
#[derive(Clone)]
pub struct Vm {
    pub status: Status,
    next_instruction: InstructionPointer,
    pub heap: Heap,
    pub data_stack: Vec<ObjectPointer>,
    pub call_stack: Vec<InstructionPointer>,
    pub import_stack: Vec<Input>,
    pub tracer: Tracer,
    pub fuzzable_closures: Vec<(Id, Closure)>,
    pub num_instructions_executed: usize,
}

#[derive(Clone, Debug)]
pub enum Status {
    Running,
    Done,
    Panicked { reason: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct InstructionPointer {
    /// Pointer to the closure object that is currently running code.
    closure: ObjectPointer,

    /// Index of the next instruction to run.
    instruction: usize,
}
impl InstructionPointer {
    fn null_pointer() -> Self {
        Self {
            closure: 0,
            instruction: 0,
        }
    }
    fn start_of_closure(closure: ObjectPointer) -> Self {
        Self {
            closure,
            instruction: 0,
        }
    }
}

pub struct TearDownResult {
    pub return_value: Value,
    pub fuzzable_closures: Vec<(Id, Closure)>,
}

impl Vm {
    pub fn new() -> Self {
        Self {
            status: Status::Done,
            next_instruction: InstructionPointer::null_pointer(),
            heap: Heap::new(),
            data_stack: vec![],
            call_stack: vec![],
            import_stack: vec![],
            tracer: Tracer::default(),
            fuzzable_closures: vec![],
            num_instructions_executed: 0,
        }
    }

    pub fn set_up_closure_execution<U: UseProvider>(
        &mut self,
        use_provider: &U,
        closure: Closure,
        arguments: Vec<Value>,
    ) {
        assert!(matches!(self.status, Status::Done));
        assert!(self.data_stack.is_empty());
        assert!(self.call_stack.is_empty());

        assert_eq!(closure.num_args, arguments.len());

        for arg in arguments {
            let address = self.heap.import(arg);
            self.data_stack.push(address);
        }
        let address = self.heap.import(Value::Closure(closure.clone()));
        self.data_stack.push(address);

        self.status = Status::Running;
        self.run_instruction(
            use_provider,
            Instruction::Call {
                num_args: closure.num_args,
            },
        );
    }
    pub fn tear_down_closure_execution(&mut self) -> TearDownResult {
        assert!(matches!(self.status, Status::Done));
        let return_value = self.data_stack.pop().unwrap();
        let return_value = self.heap.export(return_value);
        TearDownResult {
            return_value,
            fuzzable_closures: mem::replace(&mut self.fuzzable_closures, vec![]),
        }
    }

    pub fn set_up_module_closure_execution<U: UseProvider>(
        &mut self,
        use_provider: &U,
        closure: Closure,
    ) {
        assert_eq!(closure.captured.len(), 0, "Called start_module_closure with a closure that is not a module closure (it captures stuff).");
        assert_eq!(closure.num_args, 0, "Called start_module_closure with a closure that is not a module closure (it has arguments).");
        self.set_up_closure_execution(use_provider, closure, vec![]);
    }
    pub fn tear_down_module_closure_execution(&mut self) -> TearDownResult {
        self.tear_down_closure_execution()
    }

    pub fn status(&self) -> Status {
        self.status.clone()
    }

    fn get_from_data_stack(&self, offset: usize) -> ObjectPointer {
        self.data_stack[self.data_stack.len() - 1 - offset as usize].clone()
    }

    pub fn run<U: UseProvider>(&mut self, use_provider: &U, mut num_instructions: usize) {
        assert!(
            matches!(self.status, Status::Running),
            "Called Vm::run on a vm that is not ready to run."
        );
        while matches!(self.status, Status::Running) && num_instructions > 0 {
            num_instructions -= 1;

            let current_closure = self.heap.get(self.next_instruction.closure);
            let current_body = if let ObjectData::Closure { body, .. } = &current_closure.data {
                body
            } else {
                panic!("The instruction pointer points to a non-closure.");
            };
            let instruction = current_body[self.next_instruction.instruction].clone();

            // log::trace!(
            //     "Data stack: {}",
            //     self.data_stack
            //         .iter()
            //         .map(|address| format!("{}", self.heap.export_without_dropping(*address)))
            //         .join(", ")
            // );
            // log::trace!(
            //     "Call stack: {}",
            //     self.call_stack
            //         .iter()
            //         .map(|ip| format!("{}:{}", ip.closure, ip.instruction))
            //         .join(", ")
            // );
            // log::trace!(
            //     "Instruction pointer: {}:{}",
            //     self.next_instruction.closure,
            //     self.next_instruction.instruction
            // );
            // log::trace!("Heap: {:?}", self.heap);

            log::trace!("Running instruction: {instruction:?}");
            self.next_instruction.instruction += 1;
            self.run_instruction(use_provider, instruction);
            self.num_instructions_executed += 1;

            if self.next_instruction == InstructionPointer::null_pointer() {
                self.status = Status::Done;
            }
        }
    }
    pub fn run_instruction<U: UseProvider>(&mut self, use_provider: &U, instruction: Instruction) {
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
            Instruction::CreateClosure {
                num_args,
                body,
                captured,
            } => {
                let captured = captured
                    .iter()
                    .map(|offset| self.get_from_data_stack(*offset))
                    .collect_vec();
                for address in &captured {
                    self.heap.dup(*address);
                }
                let address = self.heap.create(ObjectData::Closure {
                    captured,
                    num_args,
                    body,
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
                    ObjectData::Closure {
                        captured,
                        num_args: expected_num_args,
                        ..
                    } => {
                        if num_args != expected_num_args {
                            self.panic(format!("Closure expects {expected_num_args} parameters, but you called it with {num_args} arguments."));
                            return;
                        }

                        self.call_stack.push(self.next_instruction);
                        self.data_stack.append(&mut captured.clone());
                        for captured in captured {
                            self.heap.dup(captured);
                        }
                        self.data_stack.append(&mut args);
                        self.next_instruction =
                            InstructionPointer::start_of_closure(closure_address);
                    }
                    ObjectData::Builtin(builtin) => {
                        self.heap.drop(closure_address);
                        self.run_builtin_function(use_provider, &builtin, &args);
                    }
                    _ => {
                        self.panic("you can only call closures and builtins".to_string());
                    }
                };
            }
            Instruction::Needs => {
                let reason = self.data_stack.pop().unwrap();
                let condition = self.data_stack.pop().unwrap();

                let reason = match self.heap.export(reason) {
                    Value::Text(reason) => reason,
                    _ => {
                        self.panic("you can only use text as the reason of a `needs`".to_string());
                        return;
                    }
                };

                match self.heap.get(condition).data.clone() {
                    ObjectData::Symbol(symbol) => match symbol.as_str() {
                        "True" => {
                            self.data_stack.push(self.heap.import(Value::nothing()));
                        }
                        "False" => self.status = Status::Panicked { reason },
                        _ => {
                            self.panic("Needs expects True or False as a symbol.".to_string());
                        }
                    },
                    _ => {
                        self.panic("Needs expects a boolean symbol.".to_string());
                    }
                }
            }
            Instruction::Return => {
                self.heap.drop(self.next_instruction.closure);
                let caller = self.call_stack.pop().unwrap();
                self.next_instruction = caller;
            }
            Instruction::RegisterFuzzableClosure(id) => {
                let closure = self.data_stack.last().unwrap().clone();
                match self.heap.export_without_dropping(closure) {
                    Value::Closure(closure) => self.fuzzable_closures.push((id, closure)),
                    _ => panic!("Instruction RegisterFuzzableClosure executed, but stack top is not a closure."),
                }
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
                let reason = self.data_stack[self.data_stack.len() - 2];
                let condition = self.heap.export_without_dropping(condition);
                let reason = self.heap.export_without_dropping(reason);
                self.tracer.push(TraceEntry::NeedsStarted {
                    id,
                    condition,
                    reason,
                });
            }
            Instruction::TraceNeedsEnds => self.tracer.push(TraceEntry::NeedsEnded),
            Instruction::TraceModuleStarts { input } => {
                if self.import_stack.contains(&input) {
                    self.panic(format!(
                        "there's an import cycle ({})",
                        self.import_stack
                            .iter()
                            .skip_while(|it| **it != input)
                            .chain([&input])
                            .map(|input| format!("{input}"))
                            .join(" → "),
                    ));
                }
                self.import_stack.push(input.clone());
                self.tracer.push(TraceEntry::ModuleStarted { input });
            }
            Instruction::TraceModuleEnds => {
                self.import_stack.pop().unwrap();
                self.tracer.push(TraceEntry::ModuleEnded {
                    export_map: self
                        .heap
                        .export_without_dropping(*self.data_stack.last().unwrap()),
                })
            }
            Instruction::Error { id, error } => {
                self.panic(format!(
                    "The VM crashed because there was an error at {id}: {error:?}"
                ));
            }
        }
    }

    pub fn panic(&mut self, reason: String) -> Value {
        self.status = Status::Panicked { reason };
        Value::Symbol("Never".to_string())
    }
}
