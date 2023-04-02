use crate::{
    storage::TraceStorage,
    time::{Time, Timer},
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
use std::{
    cmp::max,
    fmt,
    sync::{Arc, RwLock},
};
use tracing::{debug, error};

pub fn trace_call(
    storage: Arc<RwLock<TraceStorage>>,
    call_site: Pointer,
    callee: Pointer,
    arguments: Vec<Pointer>,
    responsible: Pointer,
) -> TraceId {
    let mut vm_heap = Heap::default();
    let storage = storage.write().unwrap();
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

pub struct LogicalTracer {
    pub timer: Timer,
    pub storage: Arc<RwLock<TraceStorage>>,
    pub stack: Vec<TraceId>,
}

impl LogicalTracer {
    fn new(storage: Arc<RwLock<TraceStorage>>) -> Self {
        Self {
            timer: Timer::start(),
            storage,
            stack: vec![],
        }
    }

    fn end_current_and_merge(
        stack: &mut Vec<TraceId>,
        storage: &mut TraceStorage,
        now: Time,
        result: CallResult,
    ) {
        let current = stack.pop().unwrap();
        let Trace::CallSpan { end, .. } = storage.get_mut(current) else { unreachable!() };
        *end = Some(CallEnd { when: now, result });

        let before = *stack.last().unwrap();
        let Trace::CallSpan { children, .. } = storage.get_mut(before) else { unreachable!() };
        children.as_mut().unwrap().push(current);
    }
    fn end_all(
        stack: &mut Vec<TraceId>,
        storage: &mut TraceStorage,
        now: Time,
        result: CallResult,
    ) {
        while stack.len() > 1 {
            Self::end_current_and_merge(stack, storage, now, result);
        }
    }
}
impl Tracer for LogicalTracer {
    fn add(&mut self, event: VmEvent) {
        match event {
            VmEvent::FiberCreated { fiber } => {}
            VmEvent::FiberDone { fiber } => {}
            VmEvent::FiberPanicked {
                fiber,
                panicked_child,
            } => {
                let storage = self.storage.write().unwrap();
                if fiber == FiberId::root() {
                    self.end_all(
                        &mut self.stack,
                        storage,
                        self.timer.now(),
                        CallResult::Panicked,
                    );
                }
            }
            VmEvent::FiberCanceled { fiber } => {
                let storage = self.storage.write().unwrap();
                if fiber == FiberId::root() {
                    self.end_all(
                        &mut self.stack,
                        storage,
                        self.timer.now(),
                        CallResult::Canceled,
                    );
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
                    let storage = self.storage.write().unwrap();
                    let call_site = storage.import_from_heap(heap, call_site);
                    let callee = storage.import_from_heap(heap, callee);
                    let arguments = arguments
                        .into_iter()
                        .map(|arg| storage.import_from_heap(heap, arg))
                        .collect_vec();
                    debug!(
                        "{}{} {}",
                        "  ".repeat(self.stack.len()),
                        callee.format(&storage.heap),
                        arguments
                            .iter()
                            .map(|argument| argument.format(&storage.heap))
                            .join(" "),
                    );
                    let id = storage.create(Trace::CallSpan {
                        call_site,
                        callee,
                        arguments,
                        children: Some(vec![]),
                        start: self.timer.now(),
                        end: None,
                    });
                    self.stack.push(id);
                    storage.maybe_deduplicate();
                }
                FiberEvent::CallEnded { return_value, heap } => {
                    let storage = self.storage.write().unwrap();
                    let return_value = storage.import_from_heap(heap, return_value);
                    self.end_current_and_merge(CallResult::Returned(return_value));
                    storage.maybe_deduplicate();
                }
            },
        }
    }
}
