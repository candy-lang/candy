use std::fmt::{self, Debug};

use crate::channel::ChannelId;

use super::{
    channel::{Capacity, Packet},
    context::{ExecutionController, UseProvider},
    heap::{Builtin, Closure, Data, Heap, Pointer, Text},
    lir::Instruction,
    tracer::FiberTracer,
};
use candy_frontend::{
    hir::{self, Id},
    id::CountableId,
    module::Module,
};
use itertools::Itertools;
use rustc_hash::FxHashMap;
use tracing::trace;

const TRACE: bool = false;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct FiberId(usize);
impl CountableId for FiberId {
    fn from_usize(id: usize) -> Self {
        Self(id)
    }
    fn to_usize(&self) -> usize {
        self.0
    }
}
impl Debug for FiberId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "fiber_{:x}", self.0)
    }
}

/// A fiber represents an execution thread of a program. It's a stack-based
/// machine that runs instructions from a LIR. Fibers are owned by a `Vm`.
#[derive(Clone)]
pub struct Fiber<T: FiberTracer> {
    pub status: Status,
    next_instruction: InstructionPointer,
    pub data_stack: Vec<Pointer>,
    pub call_stack: Vec<InstructionPointer>,
    pub import_stack: Vec<Module>,
    pub heap: Heap,
    pub tracer: T,
}

#[derive(Clone, Debug)]
pub enum Status {
    Running,
    CreatingChannel {
        capacity: Capacity,
    },
    Sending {
        channel: ChannelId,
        packet: Packet,
    },
    Receiving {
        channel: ChannelId,
    },
    InParallelScope {
        body: Pointer,
    },
    InTry {
        body: Pointer,
    },
    Done,
    Panicked {
        reason: String,
        responsible: hir::Id,
    },
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

pub enum ExecutionResult {
    Finished(Packet),
    Panicked { reason: String, responsible: Id },
}

impl Fiber {
    fn new_with_heap(heap: Heap, tracer: Box<dyn FiberTracer>) -> Self {
        Self {
            status: Status::Done,
            next_instruction: InstructionPointer::null_pointer(),
            data_stack: vec![],
            call_stack: vec![],
            import_stack: vec![],
            heap,
            tracer,
        }
    }
    pub fn new_for_running_closure(
        heap: Heap,
        closure: Pointer,
        arguments: Vec<Pointer>,
        responsible: Pointer,
        tracer: Box<dyn FiberTracer>,
    ) -> Self {
        assert!(matches!(heap.get(closure).data, Data::Closure(_)));

        let mut fiber = Self::new_with_heap(heap, tracer);
        fiber.status = Status::Running;
        fiber.call(closure, arguments, responsible);

        fiber
    }
    pub fn new_for_running_module_closure(module: Module, closure: Closure) -> Self {
        assert_eq!(
            closure.captured.len(),
            0,
            "Closure is not a module closure (it captures stuff)."
        );
        assert_eq!(
            closure.num_args, 0,
            "Closure is not a module closure (it has arguments)."
        );
        let mut heap = Heap::default();
        let closure = heap.create_closure(closure);
        let module_id = heap.create_hir_id(Id::new(module, vec![]));
        Self::new_for_running_closure(heap, closure, vec![], module_id, tracer)
    }

    pub fn into_execution_result(mut self) -> ExecutionResult {
        match self.status {
            Status::Done => ExecutionResult::Finished(Packet {
                heap: self.heap,
                address: self.data_stack.pop().unwrap(),
            }),
            Status::Panicked {
                reason,
                responsible,
            } => ExecutionResult::Panicked {
                reason,
                responsible,
            },
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
        assert!(matches!(self.status, Status::CreatingChannel { .. }));

        let send_port_symbol = self.heap.create_symbol("SendPort".to_string());
        let receive_port_symbol = self.heap.create_symbol("ReceivePort".to_string());
        let send_port = self.heap.create_send_port(channel);
        let receive_port = self.heap.create_receive_port(channel);
        self.data_stack
            .push(self.heap.create_struct(FxHashMap::from_iter([
                (send_port_symbol, send_port),
                (receive_port_symbol, receive_port),
            ])));
        self.status = Status::Running;
    }
    pub fn complete_send(&mut self) {
        assert!(matches!(self.status, Status::Sending { .. }));

        self.data_stack.push(self.heap.create_nothing());
        self.status = Status::Running;
    }
    pub fn complete_receive(&mut self, packet: Packet) {
        assert!(matches!(self.status, Status::Receiving { .. }));

        let address = packet.clone_to_other_heap(&mut self.heap);
        self.data_stack.push(address);
        self.status = Status::Running;
    }
    pub fn complete_parallel_scope(&mut self, result: Result<Packet, (String, Id)>) {
        assert!(matches!(self.status, Status::InParallelScope { .. }));

        match result {
            Ok(packet) => {
                let value = packet
                    .heap
                    .clone_single_to_other_heap(&mut self.heap, packet.address);
                self.data_stack.push(value);
                self.status = Status::Running;
            }
            Err((reason, responsible)) => self.panic(reason, responsible),
        }
    }
    pub fn complete_try(&mut self, result: ExecutionResult) {
        assert!(matches!(self.status, Status::InTry { .. }));
        let result = match result {
            ExecutionResult::Finished(Packet {
                heap,
                address: return_value,
            }) => Ok(heap.clone_single_to_other_heap(&mut self.heap, return_value)),
            ExecutionResult::Panicked { reason, .. } => Err(self.heap.create_text(reason)),
        };
        self.data_stack.push(self.heap.create_result(result));
        self.status = Status::Running;
    }

    fn get_from_data_stack(&self, offset: usize) -> Pointer {
        self.data_stack[self.data_stack.len() - 1 - offset]
    }
    pub fn panic(&mut self, reason: String, responsible: hir::Id) {
        assert!(!matches!(
            self.status,
            Status::Done | Status::Panicked { .. }
        ));
        self.status = Status::Panicked {
            reason,
            responsible,
        };
    }

    pub fn run(
        &mut self,
        use_provider: &dyn UseProvider,
        execution_controller: &mut dyn ExecutionController,
        tracer: &mut FiberTracer,
    ) {
        assert!(
            matches!(self.status, Status::Running),
            "Called Fiber::run on a fiber that is not ready to run."
        );
        while matches!(self.status, Status::Running)
            && execution_controller.should_continue_running()
        {
            if self.next_instruction == InstructionPointer::null_pointer() {
                self.status = Status::Done;
                break;
            }

            let current_closure = self.heap.get(self.next_instruction.closure);
            let current_body = if let Data::Closure(Closure { body, .. }) = &current_closure.data {
                body
            } else {
                panic!("The instruction pointer points to a non-closure.");
            };
            let instruction = current_body
                .get(self.next_instruction.instruction)
                .expect("invalid instruction pointer")
                .clone();

            self.next_instruction.instruction += 1;
            self.run_instruction(use_provider, tracer, instruction);
            execution_controller.instruction_executed();
        }
    }
    pub fn run_instruction(
        &mut self,
        use_provider: &dyn UseProvider,
        tracer: &mut FiberTracer,
        instruction: Instruction,
    ) {
        if TRACE {
            trace!(
                "Instruction pointer: {}:{}",
                self.next_instruction.closure,
                self.next_instruction.instruction,
            );
            trace!(
                "Data stack: {}",
                self.data_stack
                    .iter()
                    .map(|it| it.format_debug(&self.heap))
                    .join(", "),
            );
            trace!(
                "Call stack: {}",
                self.call_stack
                    .iter()
                    .map(|ip| format!("{}:{}", ip.closure, ip.instruction))
                    .join(", "),
            );
            trace!("Heap: {:?}", self.heap);
            trace!("Running instruction: {instruction:?}");
        }

        match instruction {
            Instruction::CreateInt(int) => {
                let address = self.heap.create_int(int);
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
            Instruction::CreateList { num_items } => {
                let mut item_addresses = vec![];
                for _ in 0..num_items {
                    item_addresses.push(self.data_stack.pop().unwrap());
                }
                let items = item_addresses.into_iter().rev().collect_vec();
                let address = self.heap.create_list(items);
                self.data_stack.push(address);
            }
            Instruction::CreateStruct { num_fields } => {
                let mut key_value_addresses = vec![];
                for _ in 0..(2 * num_fields) {
                    key_value_addresses.push(self.data_stack.pop().unwrap());
                }
                let mut entries = FxHashMap::default();
                for mut key_and_value in &key_value_addresses.into_iter().rev().chunks(2) {
                    let key = key_and_value.next().unwrap();
                    let value = key_and_value.next().unwrap();
                    assert_eq!(key_and_value.next(), None);
                    entries.insert(key, value);
                }
                let address = self.heap.create_struct(entries);
                self.data_stack.push(address);
            }
            Instruction::CreateHirId(id) => {
                let address = self.heap.create_hir_id(id);
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
                let responsible = self.data_stack.pop().unwrap();
                let mut arguments = vec![];
                for _ in 0..num_args {
                    arguments.push(self.data_stack.pop().unwrap());
                }
                arguments.reverse();
                let callee = self.data_stack.pop().unwrap();

                self.call(callee, arguments, responsible);
            }
            Instruction::TailCall {
                num_locals_to_pop,
                num_args,
            } => {
                let responsible = self.data_stack.pop().unwrap();
                let mut arguments = vec![];
                for _ in 0..num_args {
                    arguments.push(self.data_stack.pop().unwrap());
                }
                arguments.reverse();
                let callee = self.data_stack.pop().unwrap();
                for _ in 0..num_locals_to_pop {
                    let address = self.data_stack.pop().unwrap();
                    self.heap.drop(address);
                }

                // Tail calling a function is basically just a normal call, but
                // pretending we are our caller.
                let caller = self.call_stack.pop().unwrap();
                self.next_instruction = caller;
                self.call(callee, arguments, responsible);
            }
            Instruction::Return => {
                self.heap.drop(self.next_instruction.closure);
                let caller = self.call_stack.pop().unwrap();
                self.next_instruction = caller;
            }
            Instruction::UseModule { current_module } => {
                let responsible = self.data_stack.pop().unwrap();
                let relative_path = self.data_stack.pop().unwrap();

                match self.use_module(use_provider, current_module, relative_path) {
                    Ok(()) => {}
                    Err(reason) => {
                        let responsible = self.heap.get_hir_id(responsible);
                        self.panic(reason, responsible);
                    }
                }
            }
            Instruction::Panic => {
                let responsible_for_panic = self.data_stack.pop().unwrap();
                let reason = self.data_stack.pop().unwrap();

                let reason: Result<Text, _> = self.heap.get(reason).data.clone().try_into();
                let Ok(reason) = reason else {
                    // Panic expressions only occur inside the needs function
                    // where we have validated the inputs before calling the
                    // instructions, or when lowering compiler errors from the
                    // HIR to the MIR.
                    panic!("We should never generate a LIR where the reason is not a text.");
                };
                let responsible = self.heap.get_hir_id(responsible_for_panic);

                self.panic(reason.value, responsible);
            }
            Instruction::ModuleStarts { module } => {
                if self.import_stack.contains(&module) {
                    self.panic(
                        "Import cycle.".to_string(),
                        hir::Id::new(module.clone(), vec![]),
                    );
                }
                self.import_stack.push(module);
            }
            Instruction::ModuleEnds => {
                self.import_stack.pop().unwrap();
            }
            Instruction::TraceCallStarts { num_args } => {
                let responsible = self.data_stack.pop().unwrap();
                let mut args = vec![];
                for _ in 0..num_args {
                    args.push(self.data_stack.pop().unwrap());
                }
                let callee_address = self.data_stack.pop().unwrap();
                let call_site = self.data_stack.pop().unwrap();

                args.reverse();
                tracer.call_started(call_site, callee_address, args, responsible, &mut self.heap);
            }
            Instruction::TraceCallEnds => {
                let return_value = self.data_stack.pop().unwrap();

                tracer.call_ended(return_value, &mut self.heap);
            }
            Instruction::TraceExpressionEvaluated => {
                let value = self.data_stack.pop().unwrap();
                let expression = self.data_stack.pop().unwrap();

                tracer.value_evaluated(expression, value, &mut self.heap);
            }
            Instruction::TraceFoundFuzzableClosure => {
                let closure = self.data_stack.pop().unwrap();
                let definition = self.data_stack.pop().unwrap();

                assert!(
                    matches!(self.heap.get(closure).data, Data::Closure(_)),
                    "Instruction TraceFoundFuzzableClosure executed, but stack top is not a closure.",
                );

                tracer.found_fuzzable_closure(definition, closure, &mut self.heap);
            }
        }
    }

    pub fn call(&mut self, callee: Pointer, mut arguments: Vec<Pointer>, responsible: Pointer) {
        match &self.heap.get(callee).data {
            Data::Closure(Closure {
                captured,
                num_args: expected_num_args,
                ..
            }) => {
                if arguments.len() != *expected_num_args {
                    self.panic(
                        format!(
                            "A closure expected {expected_num_args} parameters, but you called it with {} arguments.",
                            arguments.len(),
                        ),
                        self.heap.get_hir_id(responsible),
                    );
                    return;
                }

                self.call_stack.push(self.next_instruction);
                let mut captured = captured.clone();
                for captured in &captured {
                    self.heap.dup(*captured);
                }
                self.data_stack.append(&mut captured);
                self.data_stack.append(&mut arguments);
                self.data_stack.push(responsible);
                self.next_instruction = InstructionPointer::start_of_closure(callee);
            }
            Data::Builtin(Builtin { function: builtin }) => {
                let builtin = *builtin;
                self.heap.drop(callee);
                self.run_builtin_function(&builtin, &arguments, responsible);
            }
            _ => {
                self.panic(
                    format!(
                        "You can only call closures and builtins, but you tried to call {}.",
                        callee.format(&self.heap),
                    ),
                    self.heap.get_hir_id(responsible),
                );
            }
        };
    }
}

trait NthLast {
    fn nth_last(&mut self, index: usize) -> Pointer;
}
impl NthLast for Vec<Pointer> {
    fn nth_last(&mut self, index: usize) -> Pointer {
        self[self.len() - 1 - index]
    }
}
