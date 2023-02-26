use crate::{
    storage::TraceStorage,
    time::Timer,
    trace::{CallEnd, CallResult, Trace, TraceId},
};
use candy_frontend::hir;
use candy_frontend::id::CountableId;
use candy_vm::{
    context::{PanickingUseProvider, RunForever},
    fiber::{ExecutionResult, FiberId},
    heap::{Heap, Pointer},
    tracer::{FiberEvent, Tracer, VmEvent},
    vm::{Status, Vm},
};
use itertools::Itertools;
use rustc_hash::FxHashMap;
use std::{cmp::max, fmt};
use tracing::{debug, error};

pub fn trace_call(
    storage: &mut TraceStorage,
    call_site: Pointer,
    callee: Pointer,
    arguments: Vec<Pointer>,
    responsible: Pointer,
) -> TraceId {
    let mut vm_heap = Heap::default();
    let call_site = storage
        .heap
        .clone_single_to_other_heap(&mut vm_heap, call_site);
    let callee = storage
        .heap
        .clone_single_to_other_heap(&mut vm_heap, callee);
    let arguments = storage
        .heap
        .clone_multiple_to_other_heap_with_existing_mapping(
            &mut vm_heap,
            &arguments,
            &mut FxHashMap::default(),
        );
    let responsible = storage
        .heap
        .clone_single_to_other_heap(&mut vm_heap, responsible);

    let mut tracer = LogicalTracer::new(storage);
    tracer.for_fiber(FiberId::root()).call_started(
        call_site,
        callee,
        arguments.clone(),
        responsible,
        &vm_heap,
    );

    let mut vm = Vm::default();
    vm.set_up_for_running_closure(vm_heap, callee, arguments, responsible);
    loop {
        match vm.status() {
            Status::CanRun => {
                vm.run(&PanickingUseProvider, &mut RunForever, &mut tracer);
            }
            Status::WaitingForOperations => {}
            _ => break,
        }
        vm.free_unreferenced_channels();
    }
    match vm.tear_down() {
        ExecutionResult::Finished(return_value) => {
            // tracer
            //     .for_fiber(FiberId::root())
            //     .call_ended(return_value.address, &return_value.heap);
            debug!("The function returned: {return_value:?}");
        }
        ExecutionResult::Panicked {
            reason,
            responsible,
        } => {
            error!("The function panicked: {reason}");
            error!("{responsible} is responsible.");
        }
    }

    assert_eq!(tracer.stack.len(), 1);
    tracer.stack.pop().unwrap()
}

pub struct LogicalTracer<'s> {
    pub timer: Timer,
    pub storage: &'s mut TraceStorage,
    pub stack: Vec<TraceId>,
}

impl<'s> LogicalTracer<'s> {
    fn new(storage: &'s mut TraceStorage) -> Self {
        Self {
            timer: Timer::start(),
            storage,
            stack: vec![],
        }
    }

    fn end_current_and_merge(&mut self, result: CallResult) {
        let current = self.stack.pop().unwrap();
        let Trace::CallSpan { end, .. } = self.storage.get_mut(current) else { unreachable!() };
        *end = Some(CallEnd {
            when: self.timer.get_time(),
            result,
        });

        let before = *self.stack.last().unwrap();
        let Trace::CallSpan { children, .. } = self.storage.get_mut(before) else { unreachable!() };
        children.as_mut().unwrap().push(current);
    }
    fn end_all(&mut self, result: CallResult) {
        while self.stack.len() > 1 {
            self.end_current_and_merge(result);
        }
    }
}
impl<'s> Tracer for LogicalTracer<'s> {
    fn add(&mut self, event: VmEvent) {
        match event {
            VmEvent::FiberCreated { fiber } => {}
            VmEvent::FiberDone { fiber } => {}
            VmEvent::FiberPanicked {
                fiber,
                panicked_child,
            } => {
                if fiber == FiberId::root() {
                    self.end_all(CallResult::Panicked);
                }
            }
            VmEvent::FiberCanceled { fiber } => {
                if fiber == FiberId::root() {
                    self.end_all(CallResult::Canceled);
                }
            }
            VmEvent::FiberExecutionStarted { .. } => {}
            VmEvent::FiberExecutionEnded { .. } => {}
            VmEvent::ChannelCreated { .. } => {}
            VmEvent::InFiber { fiber, event } => match event {
                FiberEvent::ValueEvaluated { .. } => {}
                FiberEvent::FoundFuzzableClosure { .. } => {}
                FiberEvent::CallStarted {
                    call_site,
                    callee,
                    arguments,
                    // TODO: Save who's responsible.
                    responsible: _,
                    heap,
                } => {
                    let call_site = self.storage.import_from_heap(heap, call_site);
                    let callee = self.storage.import_from_heap(heap, callee);
                    let arguments = arguments
                        .into_iter()
                        .map(|arg| self.storage.import_from_heap(heap, arg))
                        .collect_vec();
                    debug!(
                        "{}{} {}",
                        "  ".repeat(self.stack.len()),
                        callee.format(&self.storage.heap),
                        arguments
                            .iter()
                            .map(|argument| argument.format(&self.storage.heap))
                            .join(" "),
                    );
                    let id = self.storage.create(Trace::CallSpan {
                        call_site,
                        callee,
                        arguments,
                        children: Some(vec![]),
                        start: self.timer.get_time(),
                        end: None,
                    });
                    self.stack.push(id);
                }
                FiberEvent::CallEnded { return_value, heap } => {
                    let return_value = self.storage.import_from_heap(heap, return_value);
                    self.end_current_and_merge(CallResult::Returned(return_value));
                }
            },
        }

        self.storage.maybe_deduplicate();
    }
}
