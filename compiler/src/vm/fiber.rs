use super::{
    channel::{Capacity, Packet},
    context::{ExecutionController, UseProvider},
    heap::{Builtin, ChannelId, Closure, Data, Heap, Pointer},
    tracer::InFiberTracer,
};
use crate::{
    compiler::lir::Instruction,
    module::Module,
    vm::{context::PanickingUseProvider, tracer::DummyInFiberTracer},
};
use itertools::Itertools;
use std::collections::HashMap;
use tracing::trace;

const TRACE: bool = false;

/// A fiber represents an execution thread of a program. It's a stack-based
/// machine that runs instructions from a LIR. Fibers are owned by a `Vm`.
#[derive(Clone)]
pub struct Fiber {
    pub status: Status,
    next_instruction: InstructionPointer,
    pub data_stack: Vec<Pointer>,
    pub call_stack: Vec<InstructionPointer>,
    pub import_stack: Vec<Module>,
    pub heap: Heap,
}

#[derive(Clone, Debug)]
pub enum Status {
    Running,
    CreatingChannel { capacity: Capacity },
    Sending { channel: ChannelId, packet: Packet },
    Receiving { channel: ChannelId },
    InParallelScope { body: Pointer },
    InTry { body: Pointer },
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

impl Fiber {
    fn new_with_heap(heap: Heap) -> Self {
        Self {
            status: Status::Done,
            next_instruction: InstructionPointer::null_pointer(),
            data_stack: vec![],
            call_stack: vec![],
            import_stack: vec![],
            heap,
        }
    }
    pub fn new_for_running_closure(heap: Heap, closure: Pointer, arguments: &[Pointer]) -> Self {
        assert!(
            !matches!(heap.get(closure).data, Data::Builtin(_)),
            "can only use with closures, not builtins"
        );

        let mut fiber = Self::new_with_heap(heap);

        fiber.data_stack.extend(arguments);
        fiber.data_stack.push(closure);

        fiber.status = Status::Running;
        fiber.run_instruction(
            &mut PanickingUseProvider,
            &mut DummyInFiberTracer,
            Instruction::Call {
                num_args: arguments.len(),
            },
        );
        fiber
    }
    pub fn new_for_running_module_closure(closure: Closure) -> Self {
        assert_eq!(closure.captured.len(), 0, "Called start_module_closure with a closure that is not a module closure (it captures stuff).");
        assert_eq!(closure.num_args, 0, "Called start_module_closure with a closure that is not a module closure (it has arguments).");
        let mut heap = Heap::default();
        let closure = heap.create_closure(closure);
        Self::new_for_running_closure(heap, closure, &[])
    }

    pub fn tear_down(mut self) -> Result<Packet, String> {
        match self.status {
            Status::Done => Ok(Packet {
                heap: self.heap,
                address: self.data_stack.pop().unwrap(),
            }),
            Status::Panicked { reason } => Err(reason),
            _ => panic!("Called `tear_down` on a fiber that's still running."),
        }
    }

    pub fn status(&self) -> Status {
        self.status.clone()
    }

    // If the status of this fiber is something else than `Status::Running`
    // after running, then the VM that manages this fiber is expected to perform
    // some action and to then call the corresponding `complete_*` method before
    // calling `run` again.

    pub fn complete_channel_create(&mut self, channel: ChannelId) {
        let send_port = self.heap.create_send_port(channel);
        let receive_port = self.heap.create_receive_port(channel);
        self.data_stack
            .push(self.heap.create_list(&[send_port, receive_port]));
        self.status = Status::Running;
    }
    pub fn complete_send(&mut self) {
        self.data_stack.push(self.heap.create_nothing());
        self.status = Status::Running;
    }
    pub fn complete_receive(&mut self, packet: Packet) {
        let address = packet
            .heap
            .clone_single_to_other_heap(&mut self.heap, packet.address);
        self.data_stack.push(address);
        self.status = Status::Running;
    }
    pub fn complete_parallel_scope(&mut self, result: Result<(), String>) {
        match result {
            Ok(()) => {
                self.data_stack.push(self.heap.create_nothing());
                self.status = Status::Running;
            }
            Err(reason) => self.panic(reason),
        }
    }
    pub fn complete_try(&mut self, result: Result<Packet, String>) {
        let result = match result {
            Ok(Packet {
                heap,
                address: return_value,
            }) => {
                let ok = self.heap.create_symbol("Ok".to_string());
                let return_value = heap.clone_single_to_other_heap(&mut self.heap, return_value);
                self.heap.create_list(&[ok, return_value])
            }
            Err(panic_reason) => {
                let err = self.heap.create_symbol("Err".to_string());
                let reason = self.heap.create_text(panic_reason);
                self.heap.create_list(&[err, reason])
            }
        };
        self.data_stack.push(result);
        self.status = Status::Running;
    }

    fn get_from_data_stack(&self, offset: usize) -> Pointer {
        self.data_stack[self.data_stack.len() - 1 - offset as usize]
    }
    pub fn panic(&mut self, reason: String) {
        self.status = Status::Panicked { reason };
    }

    pub fn run(
        &mut self,
        use_provider: &mut dyn UseProvider,
        execution_controller: &mut dyn ExecutionController,
        tracer: &mut dyn InFiberTracer,
    ) {
        assert!(
            matches!(self.status, Status::Running),
            "Called Fiber::run on a fiber that is not ready to run."
        );
        while matches!(self.status, Status::Running)
            && execution_controller.should_continue_running()
        {
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
            self.run_instruction(use_provider, tracer, instruction);
            execution_controller.instruction_executed();

            if self.next_instruction == InstructionPointer::null_pointer() {
                self.status = Status::Done;
            }
        }
    }
    pub fn run_instruction(
        &mut self,
        use_provider: &mut dyn UseProvider,
        tracer: &mut dyn InFiberTracer,
        instruction: Instruction,
    ) {
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

                let object = self.heap.get(closure_address);
                match object.data.clone() {
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
                        self.run_builtin_function(&builtin, &args);
                    }
                    _ => {
                        self.panic(format!(
                            "you can only call closures and builtins, but you tried to call {}",
                            object.format(&self.heap),
                        ));
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
                tracer.found_fuzzable_closure(&self.heap, id, closure);
            }
            Instruction::TraceValueEvaluated(id) => {
                let value = *self.data_stack.last().unwrap();
                self.heap.dup(value);
                tracer.value_evaluated(&self.heap, id, value);
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

                tracer.call_started(&self.heap, id, closure, args);
            }
            Instruction::TraceCallEnds => {
                let return_value = *self.data_stack.last().unwrap();
                self.heap.dup(return_value);
                tracer.call_ended(&self.heap, return_value);
            }
            Instruction::TraceNeedsStarts { id } => {
                let condition = self.data_stack[self.data_stack.len() - 2];
                let reason = self.data_stack[self.data_stack.len() - 1];
                self.heap.dup(condition);
                self.heap.dup(reason);
                tracer.needs_started(&self.heap, id, condition, reason);
            }
            Instruction::TraceNeedsEnds => {
                let nothing = *self.data_stack.last().unwrap();
                self.heap.dup(nothing);
                tracer.needs_ended();
            }
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
                tracer.module_started(module);
            }
            Instruction::TraceModuleEnds => {
                self.import_stack.pop().unwrap();
                let export_map = *self.data_stack.last().unwrap();
                self.heap.dup(export_map);
                tracer.module_ended(&self.heap, export_map);
            }
            Instruction::Error { id, errors } => {
                self.panic(format!(
                    "The fiber crashed because there {} at {id}: {errors:?}",
                    if errors.len() == 1 {
                        "was an error"
                    } else {
                        "were errors"
                    }
                ));
            }
        }
    }
}
