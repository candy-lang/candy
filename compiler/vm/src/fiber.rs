use super::{
    channel::{Capacity, Packet},
    execution_controller::ExecutionController,
    heap::{Data, Function, Heap, Text},
    lir::Instruction,
};
use crate::{
    channel::ChannelId,
    heap::{
        DisplayWithSymbolTable, HirId, InlineObject, List, Pointer, ReceivePort, SendPort, Struct,
        SymbolId, SymbolTable, Tag,
    },
    tracer::{FiberTracer, TracedFiberEnded, TracedFiberEndedReason},
    Lir,
};
use candy_frontend::{
    hir::{self, Id},
    id::CountableId,
    impl_countable_id,
    module::Module,
};
use derive_more::{Deref, From};
use itertools::Itertools;
use std::{
    fmt::{self, Debug},
    iter::Step,
};
use strum::EnumIs;
use tracing::trace;

const TRACE: bool = false;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct FiberId(usize);
impl_countable_id!(FiberId);
impl Debug for FiberId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "fiber_{:x}", self.0)
    }
}

/// A fiber represents an execution thread of a program. It's a stack-based
/// machine that runs instructions from a LIR. Fibers are owned by a `Vm`.
pub struct Fiber<T: FiberTracer> {
    pub status: Status,
    next_instruction: Option<InstructionPointer>,
    pub data_stack: Vec<InlineObject>,
    pub call_stack: Vec<InstructionPointer>,
    pub heap: Heap,
    pub tracer: T,
}

#[derive(Clone, Debug, EnumIs)]
pub enum Status {
    Running,
    CreatingChannel { capacity: Capacity },
    Sending { channel: ChannelId, packet: Packet },
    Receiving { channel: ChannelId },
    InParallelScope { body: Function },
    InTry { body: Function },
    Done,
    Panicked(Panic),
}

#[derive(Clone, Copy, Deref, Eq, From, Hash, Ord, PartialEq, PartialOrd)]
pub struct InstructionPointer(usize);
impl InstructionPointer {
    pub fn null_pointer() -> Self {
        Self(0)
    }
    fn next(&self) -> Self {
        Self(self.0 + 1)
    }
}
impl Step for InstructionPointer {
    fn steps_between(start: &Self, end: &Self) -> Option<usize> {
        Some(**end - **start)
    }

    fn forward_checked(start: Self, count: usize) -> Option<Self> {
        (*start).checked_add(count).map(Self)
    }

    fn backward_checked(start: Self, count: usize) -> Option<Self> {
        (*start).checked_sub(count).map(Self)
    }
}
impl Debug for InstructionPointer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ip-{}", self.0)
    }
}

pub struct VmEnded {
    pub heap: Heap,
    pub reason: EndedReason,
}
pub struct FiberEnded<T: FiberTracer> {
    pub heap: Heap,
    pub tracer: T,
    pub reason: EndedReason,
}
#[derive(Clone, Debug)]
pub enum EndedReason {
    Finished(InlineObject),
    Panicked(Panic),
}
#[derive(Clone, Debug)]
pub struct Panic {
    pub reason: String,
    pub responsible: Id,
    pub panicked_child: Option<FiberId>,
}
impl Panic {
    pub fn new(reason: String, responsible: Id) -> Self {
        Self {
            reason,
            responsible,
            panicked_child: None,
        }
    }
    pub fn new_without_responsible(reason: String) -> Self {
        Self::new(reason, Id::complicated_responsibility())
    }
}

impl<T: FiberTracer> Fiber<T> {
    fn new_with_heap(heap: Heap, tracer: T) -> Self {
        Self {
            status: Status::Done,
            next_instruction: None,
            data_stack: vec![],
            call_stack: vec![],
            heap,
            tracer,
        }
    }
    pub fn for_function(
        heap: Heap,
        function: Function,
        arguments: &[InlineObject],
        responsible: HirId,
        tracer: T,
    ) -> Self {
        let mut fiber = Self::new_with_heap(heap, tracer);

        let platform_id = HirId::create(&mut fiber.heap, true, hir::Id::platform());
        fiber.tracer.call_started(
            &mut fiber.heap,
            platform_id,
            function.into(),
            arguments.to_vec(),
            platform_id,
        );
        platform_id.drop(&mut fiber.heap);

        fiber.status = Status::Running;
        fiber.call_function(function, arguments, responsible);

        fiber
    }
    pub fn for_module_function(
        mut heap: Heap,
        module: Module,
        function: Function,
        tracer: T,
    ) -> Self {
        assert_eq!(
            function.captured_len(),
            0,
            "Function is not a module function (it captures stuff).",
        );
        assert_eq!(
            function.argument_count(),
            0,
            "Function is not a module function (it has arguments).",
        );
        let responsible = HirId::create(&mut heap, true, Id::new(module, vec![]));
        Self::for_function(heap, function, &[], responsible, tracer)
    }

    pub fn tear_down(mut self) -> FiberEnded<T> {
        let reason = match self.status {
            Status::Done => EndedReason::Finished(self.pop_from_data_stack()),
            Status::Panicked(panic) => EndedReason::Panicked(panic),
            _ => panic!("Called `tear_down` on a fiber that's still running."),
        };
        FiberEnded {
            heap: self.heap,
            tracer: self.tracer,
            reason,
        }
    }
    pub fn adopt_finished_child(&mut self, child_id: FiberId, ended: FiberEnded<T>) {
        self.heap.adopt(ended.heap);
        let reason = match ended.reason {
            EndedReason::Finished(return_value) => TracedFiberEndedReason::Finished(return_value),
            EndedReason::Panicked(panic) => TracedFiberEndedReason::Panicked(panic),
        };
        self.tracer.child_fiber_ended(TracedFiberEnded {
            id: child_id,
            heap: &mut self.heap,
            tracer: ended.tracer,
            reason,
        });
    }

    pub fn status(&self) -> Status {
        self.status.clone()
    }
    pub fn call_stack(&self) -> &[InstructionPointer] {
        &self.call_stack
    }

    // If the status of this fiber is something else than `Status::Running`
    // after running, then the VM that manages this fiber is expected to perform
    // some action and to then call the corresponding `complete_*` method before
    // calling `run` again.

    pub fn complete_channel_create(&mut self, channel: ChannelId) {
        assert!(self.status.is_creating_channel());

        let fields = [
            (
                SymbolId::SEND_PORT,
                SendPort::create(&mut self.heap, channel),
            ),
            (
                SymbolId::RECEIVE_PORT,
                ReceivePort::create(&mut self.heap, channel),
            ),
        ];
        let struct_ = Struct::create_with_symbol_keys(&mut self.heap, true, fields);
        self.push_to_data_stack(struct_);
        self.status = Status::Running;
    }
    pub fn complete_send(&mut self) {
        assert!(self.status.is_sending());

        self.push_to_data_stack(Tag::create_nothing());
        self.status = Status::Running;
    }
    pub fn complete_receive(&mut self, packet: Packet) {
        assert!(self.status.is_receiving());

        let object = packet.object.clone_to_heap(&mut self.heap);
        self.push_to_data_stack(object);
        self.status = Status::Running;
    }
    pub fn complete_parallel_scope(&mut self, result: Result<InlineObject, Panic>) {
        assert!(self.status.is_in_parallel_scope());

        match result {
            Ok(object) => {
                self.push_to_data_stack(object);
                self.status = Status::Running;
            }
            Err(panic) => self.panic(panic),
        }
    }
    pub fn complete_try(&mut self, ended_reason: &EndedReason) {
        assert!(self.status.is_in_try());

        let result = match ended_reason {
            EndedReason::Finished(return_value) => Ok(*return_value),
            EndedReason::Panicked(panic) => {
                Err(Text::create(&mut self.heap, true, &panic.reason).into())
            }
        };
        let result = Tag::create_result(&mut self.heap, true, result);
        self.push_to_data_stack(result);
        self.status = Status::Running;
    }

    fn get_from_data_stack(&self, offset: usize) -> InlineObject {
        self.data_stack[self.data_stack.len() - 1 - offset]
    }
    #[allow(unused_parens)]
    pub fn panic(&mut self, panic: Panic) {
        assert!(!matches!(
            self.status,
            (Status::Done | Status::Panicked { .. }),
        ));

        self.heap.reset_reference_counts();
        self.tracer.dup_all_stored_objects(&mut self.heap);
        self.heap.drop_all_unreferenced();

        self.status = Status::Panicked(panic);
    }

    pub fn run(
        &mut self,
        lir: &Lir,
        execution_controller: &mut dyn ExecutionController<T>,
        id: FiberId,
    ) {
        assert!(
            self.status.is_running(),
            "Called Fiber::run on a fiber that is not ready to run.",
        );
        while self.status.is_running() && execution_controller.should_continue_running() {
            let Some(current_instruction) = self.next_instruction else {
                self.status = Status::Done;
                self.tracer
                    .call_ended(&mut self.heap, *self.data_stack.last().unwrap());
                break;
            };

            let instruction = lir
                .instructions
                .get(*current_instruction)
                .expect("invalid instruction pointer");
            self.next_instruction = Some(current_instruction.next());

            self.run_instruction(&lir.symbol_table, instruction);
            execution_controller.instruction_executed(id, self, current_instruction);
        }
    }
    pub fn run_instruction(&mut self, symbol_table: &SymbolTable, instruction: &Instruction) {
        if TRACE {
            trace!("Running instruction: {instruction:?}");
            let current_instruction = self.next_instruction.unwrap();
            trace!("Instruction pointer: {:?}", current_instruction);
            trace!(
                "Data stack: {}",
                if self.data_stack.is_empty() {
                    "<empty>".to_string()
                } else {
                    self.data_stack
                        .iter()
                        .map(|it| format!("{it:?}"))
                        .join(", ")
                },
            );
            trace!(
                "Call stack: {}",
                if self.call_stack.is_empty() {
                    "<empty>".to_string()
                } else {
                    self.call_stack
                        .iter()
                        .map(|ip| format!("{ip:?}"))
                        .join(", ")
                },
            );
            trace!("Heap: {:?}", self.heap);
        }

        match instruction {
            Instruction::CreateTag { symbol_id } => {
                let value = self.pop_from_data_stack();
                let tag = Tag::create_with_value(&mut self.heap, true, *symbol_id, value);
                self.push_to_data_stack(tag);
            }
            Instruction::CreateList { num_items } => {
                let mut item_addresses = vec![];
                for _ in 0..*num_items {
                    item_addresses.push(self.pop_from_data_stack());
                }
                let items = item_addresses.into_iter().rev().collect_vec();
                let list = List::create(&mut self.heap, true, &items);
                self.push_to_data_stack(list);
            }
            Instruction::CreateStruct { num_fields } => {
                // PERF: Avoid collecting keys and values into a `Vec` before creating the `HashMap`
                let mut key_value_addresses = vec![];
                for _ in 0..(2 * num_fields) {
                    key_value_addresses.push(self.pop_from_data_stack());
                }
                let entries = key_value_addresses.into_iter().rev().tuples().collect();
                let struct_ = Struct::create(&mut self.heap, true, &entries);
                self.push_to_data_stack(struct_);
            }
            Instruction::CreateFunction {
                captured,
                num_args,
                body,
            } => {
                let captured = captured
                    .iter()
                    .map(|offset| {
                        let object = self.get_from_data_stack(*offset);
                        object.dup(&mut self.heap);
                        object
                    })
                    .collect_vec();
                let function = Function::create(&mut self.heap, true, &captured, *num_args, *body);
                self.push_to_data_stack(function);
            }
            Instruction::PushConstant(constant) => {
                self.push_to_data_stack(*constant);
            }
            Instruction::PushFromStack(offset) => {
                let address = self.get_from_data_stack(*offset);
                address.dup(&mut self.heap);
                self.push_to_data_stack(address);
            }
            Instruction::PopMultipleBelowTop(n) => {
                let top = self.pop_from_data_stack();
                for _ in 0..*n {
                    self.pop_from_data_stack().drop(&mut self.heap);
                }
                self.push_to_data_stack(top);
            }
            Instruction::Call { num_args } => {
                let responsible = self.pop_from_data_stack().try_into().unwrap();
                let mut arguments = (0..*num_args)
                    .map(|_| self.pop_from_data_stack())
                    .collect_vec();
                // PERF: Build the reverse list in place.
                arguments.reverse();
                let callee = self.pop_from_data_stack();

                self.call(symbol_table, callee, &arguments, responsible);
            }
            Instruction::TailCall {
                num_locals_to_pop,
                num_args,
            } => {
                let responsible = self.pop_from_data_stack().try_into().unwrap();
                let mut arguments = (0..*num_args)
                    .map(|_| self.pop_from_data_stack())
                    .collect_vec();
                // PERF: Built the reverse list in place
                arguments.reverse();
                let callee = self.pop_from_data_stack();
                for _ in 0..*num_locals_to_pop {
                    self.pop_from_data_stack().drop(&mut self.heap);
                }

                // Tail calling a function is basically just a normal call, but
                // pretending we are our caller.
                self.next_instruction = self.call_stack.pop();
                self.call(symbol_table, callee, &arguments, responsible);
            }
            Instruction::Return => {
                self.next_instruction = self.call_stack.pop();
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
                let responsible: HirId = responsible_for_panic.try_into().unwrap();

                self.panic(Panic::new(
                    reason.get().to_owned(),
                    responsible.get().to_owned(),
                ));
            }
            Instruction::TraceCallStarts { num_args } => {
                let responsible = self.pop_from_data_stack().try_into().unwrap();
                let mut args = vec![];
                for _ in 0..*num_args {
                    args.push(self.pop_from_data_stack());
                }
                let callee = self.pop_from_data_stack();
                let call_site = self.pop_from_data_stack().try_into().unwrap();

                args.reverse();
                self.tracer
                    .call_started(&mut self.heap, call_site, callee, args, responsible);
            }
            Instruction::TraceCallEnds => {
                let return_value = self.pop_from_data_stack();

                self.tracer.call_ended(&mut self.heap, return_value);
            }
            Instruction::TraceExpressionEvaluated => {
                let value = self.pop_from_data_stack();
                let expression = self.pop_from_data_stack().try_into().unwrap();

                self.tracer
                    .value_evaluated(&mut self.heap, expression, value);
            }
            Instruction::TraceFoundFuzzableFunction => {
                let function = self.pop_from_data_stack().try_into().expect("Instruction TraceFoundFuzzableFunction executed, but stack top is not a function.");
                let definition = self.pop_from_data_stack().try_into().unwrap();

                self.tracer
                    .found_fuzzable_function(&mut self.heap, definition, function);
            }
        }
    }

    pub fn call(
        &mut self,
        symbol_table: &SymbolTable,
        callee: InlineObject,
        arguments: &[InlineObject],
        responsible: HirId,
    ) {
        match callee.into() {
            Data::Function(function) => self.call_function(function, arguments, responsible),
            Data::Builtin(builtin) => {
                callee.drop(&mut self.heap);
                self.run_builtin_function(symbol_table, builtin.get(), arguments, responsible);
            }
            Data::Tag(tag) => {
                if tag.has_value() {
                    self.panic(Panic::new(
                        "A tag's value cannot be overwritten by calling it. Use `tag.withValue` instead.".to_string(),
                        responsible.get().to_owned(),
                    ));
                    return;
                }

                if let [value] = arguments {
                    let tag = Tag::create_with_value(&mut self.heap, true, tag.symbol_id(), *value);
                    self.push_to_data_stack(tag);
                    value.dup(&mut self.heap);
                } else {
                    self.panic(Panic::new(
                        format!(
                            "A tag can only hold exactly one value, but you called it with {} arguments.",
                            arguments.len(),
                        ),
                        responsible.get().to_owned(),
                    ));
                }
            }
            _ => {
                self.panic(Panic::new(
                    format!(
                        "You can only call functions, builtins and tags, but you tried to call {}.",
                        DisplayWithSymbolTable::to_string(&callee, symbol_table),
                    ),
                    responsible.get().to_owned(),
                ));
            }
        };
    }
    pub fn call_function(
        &mut self,
        function: Function,
        arguments: &[InlineObject],
        responsible: HirId,
    ) {
        let expected_num_args = function.argument_count();
        if arguments.len() != expected_num_args {
            self.panic(Panic::new(
                format!(
                    "A function expected {expected_num_args} parameters, but you called it with {} arguments.",
                    arguments.len(),
                ),
                responsible.get().to_owned(),)
            );
            return;
        }

        if let Some(next_instruction) = self.next_instruction {
            self.call_stack.push(next_instruction);
        }
        let captured = function.captured();
        for captured in captured {
            captured.dup(&mut self.heap);
        }
        self.data_stack.extend_from_slice(captured);
        self.data_stack.extend_from_slice(arguments);
        self.push_to_data_stack(responsible);
        self.next_instruction = Some(function.body());
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
