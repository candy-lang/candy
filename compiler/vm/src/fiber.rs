use super::{
    channel::{Capacity, Packet},
    context::{ExecutionController, UseProvider},
    heap::{Builtin, Closure, Data, Heap, Text},
    lir::Instruction,
    tracer::FiberTracer,
};
use crate::{
    channel::ChannelId,
    heap::{HirId, InlineObject, Int, List, Pointer, ReceivePort, SendPort, Struct, Symbol},
};
use candy_frontend::{
    hir::{self, Id},
    id::CountableId,
    module::Module,
};
use itertools::Itertools;
use std::fmt::{self, Debug};
use tracing::trace;

const TRACE: bool = true;

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
pub struct Fiber {
    pub status: Status,
    next_instruction: Option<InstructionPointer>,
    pub data_stack: Vec<InlineObject>,
    pub call_stack: Vec<InstructionPointer>,
    pub import_stack: Vec<Module>,
    pub heap: Heap,
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
        body: Closure,
    },
    InTry {
        body: Closure,
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
    closure: Closure,

    /// Index of the next instruction to run.
    instruction: usize,
}
impl InstructionPointer {
    fn start_of_closure(closure: Closure) -> Self {
        Self {
            closure,
            instruction: 0,
        }
    }

    fn next(&self) -> Self {
        Self {
            closure: self.closure,
            instruction: self.instruction + 1,
        }
    }
}

pub enum ExecutionResult {
    Finished(Packet),
    Panicked { reason: String, responsible: Id },
}

impl Fiber {
    fn new_with_heap(heap: Heap) -> Self {
        Self {
            status: Status::Done,
            next_instruction: None,
            data_stack: vec![],
            call_stack: vec![],
            import_stack: vec![],
            heap,
        }
    }
    pub fn new_for_running_closure(
        heap: Heap,
        closure: Closure,
        arguments: &[InlineObject],
        responsible: hir::Id,
    ) -> Self {
        let mut fiber = Self::new_with_heap(heap);
        let responsible = HirId::create(&mut fiber.heap, responsible);
        fiber.status = Status::Running;
        fiber.call_closure(closure, arguments, responsible);
        fiber
    }
    pub fn new_for_running_module_closure(heap: Heap, module: Module, closure: Closure) -> Self {
        assert_eq!(
            closure.captured_len(),
            0,
            "Closure is not a module closure (it captures stuff).",
        );
        assert_eq!(
            closure.argument_count(),
            0,
            "Closure is not a module closure (it has arguments).",
        );
        let module_id = Id::new(module, vec![]);
        Self::new_for_running_closure(heap, closure, &[], module_id)
    }

    pub fn tear_down(mut self) -> ExecutionResult {
        match self.status {
            Status::Done => {
                let object = self.pop_from_data_stack();
                ExecutionResult::Finished(Packet {
                    heap: self.heap,
                    object,
                })
            }
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

        let fields = [
            ("SendPort", SendPort::create(&mut self.heap, channel)),
            ("ReceivePort", ReceivePort::create(&mut self.heap, channel)),
        ];
        let struct_ = Struct::create_with_symbol_keys(&mut self.heap, fields);
        self.push_to_data_stack(struct_);
        self.status = Status::Running;
    }
    pub fn complete_send(&mut self) {
        assert!(matches!(self.status, Status::Sending { .. }));

        let nothing = Symbol::create_nothing(&mut self.heap);
        self.push_to_data_stack(nothing);
        self.status = Status::Running;
    }
    pub fn complete_receive(&mut self, packet: Packet) {
        assert!(matches!(self.status, Status::Receiving { .. }));

        let object = packet.object.clone_to_heap(&mut self.heap);
        self.push_to_data_stack(object);
        self.status = Status::Running;
    }
    pub fn complete_parallel_scope(&mut self, result: Result<Packet, (String, Id)>) {
        assert!(matches!(self.status, Status::InParallelScope { .. }));

        match result {
            Ok(packet) => {
                let object = packet.object.clone_to_heap(&mut self.heap);
                self.push_to_data_stack(object);
                self.status = Status::Running;
            }
            Err((reason, responsible)) => self.panic(reason, responsible),
        }
    }
    pub fn complete_try(&mut self, result: &ExecutionResult) {
        assert!(matches!(self.status, Status::InTry { .. }));
        let result = match result {
            ExecutionResult::Finished(Packet { object, .. }) => {
                Ok(object.clone_to_heap(&mut self.heap))
            }
            ExecutionResult::Panicked { reason, .. } => {
                Err(Text::create(&mut self.heap, reason).into())
            }
        };
        let result = Struct::create_result(&mut self.heap, result);
        self.push_to_data_stack(result);
        self.status = Status::Running;
    }

    fn get_from_data_stack(&self, offset: usize) -> InlineObject {
        self.data_stack[self.data_stack.len() - 1 - offset]
    }
    #[allow(unused_parens)]
    pub fn panic(&mut self, reason: String, responsible: hir::Id) {
        assert!(!matches!(
            self.status,
            (Status::Done | Status::Panicked { .. }),
        ));
        // FIXME: Clear heap except objects referenced by the tracer
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
            let Some(next_instruction) = self.next_instruction else {
                self.status = Status::Done;
                break;
            };

            let instruction = next_instruction
                .closure
                .instructions()
                .get(next_instruction.instruction)
                .expect("Invalid instruction pointer")
                .to_owned(); // PERF: Can we avoid this clone?

            self.next_instruction = Some(next_instruction.next());
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
            trace!("Running instruction: {instruction:?}");
            let next_instruction = self.next_instruction.unwrap();
            trace!(
                "Instruction pointer: {}:{}",
                next_instruction.closure,
                next_instruction.instruction,
            );
            // trace!(
            //     "Data stack: {}",
            //     if self.data_stack.is_empty() {
            //         "<empty>".to_string()
            //     } else {
            //         self.data_stack
            //             .iter()
            //             .map(|it| format!("{it:?}"))
            //             .join(", ")
            //     },
            // );
            trace!(
                "Call stack: {}",
                if self.call_stack.is_empty() {
                    "<empty>".to_string()
                } else {
                    self.call_stack
                        .iter()
                        .map(|ip| format!("{}:{}", ip.closure, ip.instruction))
                        .join(", ")
                },
            );
            // trace!("Heap: {:?}", self.heap);
        }

        match instruction {
            Instruction::CreateInt(int) => {
                let int = Int::create_from_bigint(&mut self.heap, int);
                self.push_to_data_stack(int);
            }
            Instruction::CreateText(text) => {
                let text = Text::create(&mut self.heap, &text);
                self.push_to_data_stack(text);
            }
            Instruction::CreateSymbol(symbol) => {
                let symbol = Symbol::create(&mut self.heap, &symbol);
                self.push_to_data_stack(symbol);
            }
            Instruction::CreateList { num_items } => {
                let mut item_addresses = vec![];
                for _ in 0..num_items {
                    item_addresses.push(self.pop_from_data_stack());
                }
                let items = item_addresses.into_iter().rev().collect_vec();
                let list = List::create(&mut self.heap, &items);
                self.push_to_data_stack(list);
            }
            Instruction::CreateStruct { num_fields } => {
                // PERF: Avoid collecting keys and values into a `Vec` before creating the `HashMap`
                let mut key_value_addresses = vec![];
                for _ in 0..(2 * num_fields) {
                    key_value_addresses.push(self.pop_from_data_stack());
                }
                let entries = key_value_addresses.into_iter().rev().tuples().collect();
                let struct_ = Struct::create(&mut self.heap, &entries);
                self.push_to_data_stack(struct_);
            }
            Instruction::CreateHirId(id) => {
                let id = HirId::create(&mut self.heap, id);
                self.push_to_data_stack(id);
            }
            Instruction::CreateClosure {
                num_args,
                body,
                captured,
            } => {
                let captured = captured
                    .iter()
                    .map(|offset| {
                        let object = self.get_from_data_stack(*offset);
                        object.dup(&mut self.heap);
                        object
                    })
                    .collect_vec();
                let closure = Closure::create(&mut self.heap, &captured, num_args, body);
                self.push_to_data_stack(closure);
            }
            Instruction::CreateBuiltin(builtin) => {
                self.push_to_data_stack(Builtin::create(builtin));
            }
            Instruction::PushFromStack(offset) => {
                let address = self.get_from_data_stack(offset);
                address.dup(&mut self.heap);
                self.push_to_data_stack(address);
            }
            Instruction::PopMultipleBelowTop(n) => {
                let top = self.pop_from_data_stack();
                for _ in 0..n {
                    self.pop_from_data_stack().drop(&mut self.heap);
                }
                self.push_to_data_stack(top);
            }
            Instruction::Call { num_args } => {
                let responsible = self.pop_from_data_stack().unwrap_hir_id();
                let mut arguments = (0..num_args)
                    .map(|_| self.pop_from_data_stack())
                    .collect_vec();
                // PERF: Built the reverse list in place
                arguments.reverse();
                let callee = self.pop_from_data_stack();

                self.call(callee, &arguments, responsible);
            }
            Instruction::TailCall {
                num_locals_to_pop,
                num_args,
            } => {
                let responsible = self.pop_from_data_stack().unwrap_hir_id();
                let mut arguments = (0..num_args)
                    .map(|_| self.pop_from_data_stack())
                    .collect_vec();
                // PERF: Built the reverse list in place
                arguments.reverse();
                let callee = self.pop_from_data_stack();
                for _ in 0..num_locals_to_pop {
                    self.pop_from_data_stack().drop(&mut self.heap);
                }

                // Tail calling a function is basically just a normal call, but
                // pretending we are our caller.
                self.next_instruction = self.call_stack.pop();
                self.call(callee, &arguments, responsible);
            }
            Instruction::Return => {
                self.next_instruction.unwrap().closure.drop(&mut self.heap);
                self.next_instruction = self.call_stack.pop();
            }
            Instruction::UseModule { current_module } => {
                let responsible = self.pop_from_data_stack();
                let relative_path = self.pop_from_data_stack();

                match self.use_module(use_provider, current_module, relative_path) {
                    Ok(()) => {}
                    Err(reason) => {
                        let responsible = responsible.unwrap_hir_id();
                        self.panic(reason, responsible.get().to_owned());
                    }
                }
            }
            Instruction::Panic => {
                let responsible_for_panic = self.pop_from_data_stack();
                let reason = self.pop_from_data_stack();

                let Ok(reason) = Text::try_from(reason) else {
                    // Panic expressions only occur inside the needs function
                    // where we have validated the inputs before calling the
                    // instructions, or when lowering compiler errors from the
                    // HIR to the MIR.
                    panic!("We should never generate a LIR where the reason is not a text.");
                };
                let responsible = responsible_for_panic.unwrap_hir_id();

                self.panic(reason.get().to_owned(), responsible.get().to_owned());
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
                let responsible = self.pop_from_data_stack().try_into().unwrap();
                let mut args = vec![];
                for _ in 0..num_args {
                    args.push(self.pop_from_data_stack());
                }
                let callee = self.pop_from_data_stack();
                let call_site = self.pop_from_data_stack().try_into().unwrap();

                args.reverse();
                tracer.call_started(call_site, callee, args, responsible, &self.heap);
            }
            Instruction::TraceCallEnds => {
                let return_value = self.pop_from_data_stack();

                tracer.call_ended(return_value, &self.heap);
            }
            Instruction::TraceExpressionEvaluated => {
                let value = self.pop_from_data_stack();
                let expression = self.pop_from_data_stack().try_into().unwrap();

                tracer.value_evaluated(expression, value, &self.heap);
            }
            Instruction::TraceFoundFuzzableClosure => {
                let closure = self.pop_from_data_stack().try_into().expect("Instruction TraceFoundFuzzableClosure executed, but stack top is not a closure.");
                let definition = self.pop_from_data_stack().try_into().unwrap();

                tracer.found_fuzzable_closure(definition, closure, &self.heap);
            }
        }
    }

    pub fn call(&mut self, callee: InlineObject, arguments: &[InlineObject], responsible: HirId) {
        match callee.into() {
            Data::Closure(closure) => self.call_closure(closure, arguments, responsible),
            Data::Builtin(builtin) => {
                callee.drop(&mut self.heap);
                self.run_builtin_function(builtin.get(), arguments, responsible);
            }
            _ => {
                self.panic(
                    format!(
                        "You can only call closures and builtins, but you tried to call {callee}.",
                    ),
                    responsible.get().to_owned(),
                );
            }
        };
    }
    pub fn call_closure(
        &mut self,
        closure: Closure,
        arguments: &[InlineObject],
        responsible: HirId,
    ) {
        let expected_num_args = closure.argument_count();
        if arguments.len() != expected_num_args {
            self.panic(
                format!(
                    "A closure expected {expected_num_args} parameters, but you called it with {} arguments.",
                    arguments.len(),
                ),
                responsible.get().to_owned(),
            );
            return;
        }

        if let Some(next_instruction) = self.next_instruction {
            self.call_stack.push(next_instruction);
        }
        let captured = closure.captured();
        for captured in captured {
            captured.dup(&mut self.heap);
        }
        self.data_stack.extend_from_slice(captured);
        self.data_stack.extend_from_slice(arguments);
        self.push_to_data_stack(responsible);
        self.next_instruction = Some(InstructionPointer::start_of_closure(closure));
    }

    fn push_to_data_stack(&mut self, value: impl Into<InlineObject>) {
        self.data_stack.push(value.into());
    }
    fn pop_from_data_stack(&mut self) -> InlineObject {
        self.data_stack.pop().expect("Data stack is empty.")
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
