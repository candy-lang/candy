use super::{
    heap::{Builtin, Closure, Data, Heap, Pointer},
    tracer::{TraceEntry, Tracer},
    use_provider::{DbUseProvider, UseProvider},
};
use crate::{
    compiler::{hir::Id, lir::Instruction},
    database::Database,
    module::Module,
};
use itertools::Itertools;
use std::collections::HashMap;
use tracing::{info, trace};

const TRACE: bool = false;

/// A VM can execute some byte code.
#[derive(Clone)]
pub struct Vm {
    pub status: Status,
    next_instruction: InstructionPointer,
    pub heap: Heap,
    pub data_stack: Vec<Pointer>,
    pub call_stack: Vec<InstructionPointer>,
    pub import_stack: Vec<Module>,
    pub tracer: Tracer,
    pub fuzzable_closures: Vec<(Id, Pointer)>,
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
    closure: Pointer,

    /// Index of the next instruction to run.
    instruction: usize,
}
impl InstructionPointer {
    fn null_pointer() -> Self {
        Self {
            closure: Pointer::null(),
            instruction: 0,
        }
    }
    fn start_of_closure(closure: Pointer) -> Self {
        Self {
            closure,
            instruction: 0,
        }
    }
}

pub struct TearDownResult {
    pub heap: Heap,
    pub result: Result<Pointer, String>,
    pub fuzzable_closures: Vec<(Id, Pointer)>,
    pub tracer: Tracer,
}

impl Vm {
    fn new_with_heap(heap: Heap) -> Self {
        Self {
            status: Status::Done,
            next_instruction: InstructionPointer::null_pointer(),
            heap,
            data_stack: vec![],
            call_stack: vec![],
            import_stack: vec![],
            tracer: Tracer::default(),
            fuzzable_closures: vec![],
            num_instructions_executed: 0,
        }
    }
    pub fn new_for_running_closure<U: UseProvider>(
        heap: Heap,
        use_provider: &U,
        closure: Pointer,
        arguments: &[Pointer],
    ) -> Self {
        let mut vm = Self::new_with_heap(heap);

        vm.data_stack.extend(arguments);
        vm.data_stack.push(closure);

        vm.status = Status::Running;
        vm.run_instruction(
            use_provider,
            Instruction::Call {
                num_args: arguments.len(),
            },
        );
        vm
    }
    pub fn new_for_running_module_closure<U: UseProvider>(
        use_provider: &U,
        closure: Closure,
    ) -> Self {
        assert_eq!(closure.captured.len(), 0, "Called start_module_closure with a closure that is not a module closure (it captures stuff).");
        assert_eq!(closure.num_args, 0, "Called start_module_closure with a closure that is not a module closure (it has arguments).");
        let mut heap = Heap::default();
        let closure = heap.create_closure(closure);
        Self::new_for_running_closure(heap, use_provider, closure, &[])
    }
    pub fn tear_down(mut self) -> TearDownResult {
        let result = match self.status {
            Status::Running => panic!("Called `tear_down` on a VM that's still running."),
            Status::Done => Ok(self.data_stack.pop().unwrap()),
            Status::Panicked { reason } => Err(reason),
        };
        TearDownResult {
            heap: self.heap,
            result,
            fuzzable_closures: self.fuzzable_closures,
            tracer: self.tracer,
        }
    }

    pub fn status(&self) -> Status {
        self.status.clone()
    }

    fn get_from_data_stack(&self, offset: usize) -> Pointer {
        self.data_stack[self.data_stack.len() - 1 - offset as usize]
    }

    pub fn run<U: UseProvider>(&mut self, use_provider: &U, mut num_instructions: usize) {
        assert!(
            matches!(self.status, Status::Running),
            "Called Vm::run on a vm that is not ready to run."
        );
        while matches!(self.status, Status::Running) && num_instructions > 0 {
            num_instructions -= 1;

            let current_closure = self.heap.get(self.next_instruction.closure);
            let current_body = if let Data::Closure(Closure { body, .. }) = &current_closure.data {
                body
            } else {
                panic!("The instruction pointer points to a non-closure.");
            };
            let instruction = current_body[self.next_instruction.instruction].clone();

            if TRACE {
                trace!(
                    "Data stack: {}",
                    self.data_stack
                        .iter()
                        .map(|it| it.format(&self.heap))
                        .join(", ")
                );
                trace!(
                    "Call stack: {}",
                    self.call_stack
                        .iter()
                        .map(|ip| format!("{}:{}", ip.closure, ip.instruction))
                        .join(", ")
                );
                trace!(
                    "Instruction pointer: {}:{}",
                    self.next_instruction.closure,
                    self.next_instruction.instruction
                );
                trace!("Heap: {:?}", self.heap);
                trace!("Running instruction: {instruction:?}");
            }

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
                let address = self.heap.create_int(int.into());
                self.data_stack.push(address);
            }
            Instruction::CreateText(text) => {
                let address = self.heap.create_text(text);
                self.data_stack.push(address);
            }
            Instruction::CreateSymbol(symbol) => {
                let address = self.heap.create_symbol(symbol);
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
                let address = self.heap.create_struct(entries);
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
                let address = self.heap.create_closure(Closure {
                    captured,
                    num_args,
                    body,
                });
                self.data_stack.push(address);
            }
            Instruction::CreateBuiltin(builtin) => {
                let address = self.heap.create_builtin(builtin);
                self.data_stack.push(address);
            }
            Instruction::PushFromStack(offset) => {
                let address = self.get_from_data_stack(offset);
                self.heap.dup(address);
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
            Instruction::Call { num_args } => {
                let closure_address = self.data_stack.pop().unwrap();
                let mut args = vec![];
                for _ in 0..num_args {
                    args.push(self.data_stack.pop().unwrap());
                }
                args.reverse();

                match self.heap.get(closure_address).data.clone() {
                    Data::Closure(Closure {
                        captured,
                        num_args: expected_num_args,
                        ..
                    }) => {
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
                    Data::Builtin(Builtin { function: builtin }) => {
                        self.heap.drop(closure_address);
                        self.run_builtin_function(use_provider, &builtin, &args);
                    }
                    _ => {
                        self.panic("you can only call closures and builtins".to_string());
                    }
                };
            }
            Instruction::Return => {
                self.heap.drop(self.next_instruction.closure);
                let caller = self.call_stack.pop().unwrap();
                self.next_instruction = caller;
            }
            Instruction::UseModule { current_module } => {
                let relative_path = self.data_stack.pop().unwrap();
                match self.use_module(use_provider, current_module, relative_path) {
                    Ok(()) => {}
                    Err(reason) => {
                        self.panic(reason);
                    }
                }
            }
            Instruction::Needs => {
                let reason = self.data_stack.pop().unwrap();
                let condition = self.data_stack.pop().unwrap();

                let reason = match self.heap.get(reason).data.clone() {
                    Data::Text(reason) => reason.value,
                    _ => {
                        self.panic("you can only use text as the reason of a `needs`".to_string());
                        return;
                    }
                };

                match self.heap.get(condition).data.clone() {
                    Data::Symbol(symbol) => match symbol.value.as_str() {
                        "True" => {
                            self.data_stack.push(self.heap.create_nothing());
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
            Instruction::RegisterFuzzableClosure(id) => {
                let closure = *self.data_stack.last().unwrap();
                if !matches!(self.heap.get(closure).data, Data::Closure(_)) {
                    panic!("Instruction RegisterFuzzableClosure executed, but stack top is not a closure.");
                }
                self.heap.dup(closure);
                self.fuzzable_closures.push((id, closure));
            }
            Instruction::TraceValueEvaluated(id) => {
                let value = *self.data_stack.last().unwrap();
                self.heap.dup(value);
                self.tracer.push(TraceEntry::ValueEvaluated { id, value });
            }
            Instruction::TraceCallStarts { id, num_args } => {
                let closure = *self.data_stack.last().unwrap();
                self.heap.dup(closure);

                let mut args = vec![];
                let stack_size = self.data_stack.len();
                for i in 0..num_args {
                    let argument = self.data_stack[stack_size - i - 2];
                    self.heap.dup(argument);
                    args.push(argument);
                }
                args.reverse();

                self.tracer
                    .push(TraceEntry::CallStarted { id, closure, args });
            }
            Instruction::TraceCallEnds => {
                let return_value = *self.data_stack.last().unwrap();
                self.heap.dup(return_value);
                self.tracer.push(TraceEntry::CallEnded { return_value });
            }
            Instruction::TraceNeedsStarts { id } => {
                let condition = self.data_stack[self.data_stack.len() - 1];
                let reason = self.data_stack[self.data_stack.len() - 2];
                self.heap.dup(condition);
                self.heap.dup(reason);
                self.tracer.push(TraceEntry::NeedsStarted {
                    id,
                    condition,
                    reason,
                });
            }
            Instruction::TraceNeedsEnds => self.tracer.push(TraceEntry::NeedsEnded),
            Instruction::TraceModuleStarts { module } => {
                if self.import_stack.contains(&module) {
                    self.panic(format!(
                        "there's an import cycle ({})",
                        self.import_stack
                            .iter()
                            .skip_while(|it| **it != module)
                            .chain([&module])
                            .map(|module| format!("{module}"))
                            .join(" → "),
                    ));
                }
                self.import_stack.push(module.clone());
                self.tracer.push(TraceEntry::ModuleStarted { module });
            }
            Instruction::TraceModuleEnds => {
                self.import_stack.pop().unwrap();
                let export_map = *self.data_stack.last().unwrap();
                self.heap.dup(export_map);
                self.tracer.push(TraceEntry::ModuleEnded { export_map })
            }
            Instruction::Error { id, errors } => {
                self.panic(format!(
                    "The VM crashed because there {} at {id}: {errors:?}",
                    if errors.len() == 1 {
                        "was an error"
                    } else {
                        "were errors"
                    }
                ));
            }
        }
    }

    pub fn panic(&mut self, reason: String) {
        self.status = Status::Panicked { reason };
    }

    pub fn run_synchronously_until_completion(mut self, db: &Database) -> TearDownResult {
        let use_provider = DbUseProvider { db };
        loop {
            self.run(&use_provider, 100000);
            match self.status() {
                Status::Running => info!("Code is still running."),
                _ => return self.tear_down(),
            }
        }
    }
}
