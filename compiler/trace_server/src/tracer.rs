use crate::trace::{CallSpan, End, Trace};
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
    tracer_heap: &mut Heap,
    call_site: Pointer,
    callee: Pointer,
    arguments: Vec<Pointer>,
    responsible: Pointer,
) -> CallSpan {
    let mut vm_heap = Heap::default();
    let call_site = tracer_heap.clone_single_to_other_heap(&mut vm_heap, call_site);
    let callee = tracer_heap.clone_single_to_other_heap(&mut vm_heap, callee);
    let arguments = tracer_heap.clone_multiple_to_other_heap_with_existing_mapping(
        &mut vm_heap,
        &arguments,
        &mut FxHashMap::default(),
    );
    let responsible = tracer_heap.clone_single_to_other_heap(&mut vm_heap, responsible);

    let mut tracer = LogicalTracer::new(tracer_heap);
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

pub struct LogicalTracer<'heap> {
    pub heap: &'heap mut Heap,
    pub when_to_deduplicate: usize,
    pub stack: Vec<CallSpan>,
}

impl<'heap> LogicalTracer<'heap> {
    fn new(heap: &'heap mut Heap) -> Self {
        let heap_size = heap.number_of_objects();
        Self {
            heap,
            when_to_deduplicate: max(3 * heap_size, 100),
            stack: vec![],
        }
    }

    fn import_from_heap(&mut self, address: Pointer, heap: &Heap) -> Pointer {
        heap.clone_single_to_other_heap(&mut self.heap, address)
    }
    fn end_current_and_merge(&mut self, end: End) {
        let mut current = self.stack.pop().unwrap();
        current.end = end;
        self.stack
            .last_mut()
            .unwrap()
            .children
            .as_mut()
            .unwrap()
            .push(current);
    }
    fn end_all(&mut self, returns: End) {
        while self.stack.len() > 1 {
            self.end_current_and_merge(returns);
        }
    }
}
impl<'heap> Tracer for LogicalTracer<'heap> {
    fn add(&mut self, event: VmEvent) {
        match event {
            VmEvent::FiberCreated { fiber } => {}
            VmEvent::FiberDone { fiber } => {}
            VmEvent::FiberPanicked {
                fiber,
                panicked_child,
            } => {
                if fiber == FiberId::root() {
                    self.end_all(End::Panicked);
                }
            }
            VmEvent::FiberCanceled { fiber } => {
                if fiber == FiberId::root() {
                    self.end_all(End::Canceled);
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
                    let call_site = self.import_from_heap(call_site, heap);
                    let callee = self.import_from_heap(callee, heap);
                    let arguments = arguments
                        .into_iter()
                        .map(|arg| self.import_from_heap(arg, heap))
                        .collect_vec();
                    debug!(
                        "{}{} {}",
                        "  ".repeat(self.stack.len()),
                        callee.format(self.heap),
                        arguments
                            .iter()
                            .map(|argument| argument.format(self.heap))
                            .join(" "),
                    );
                    self.stack.push(CallSpan {
                        call_site,
                        callee,
                        arguments,
                        children: Some(vec![]),
                        end: End::NotYet,
                    });
                }
                FiberEvent::CallEnded { return_value, heap } => {
                    let return_value = self.import_from_heap(return_value, heap);
                    self.end_current_and_merge(End::Returns(return_value));
                }
            },
        }

        if self.heap.number_of_objects() > self.when_to_deduplicate {
            let pointer_map = self.heap.deduplicate();
            for entry in self.stack.iter_mut() {
                entry.change_pointers(&pointer_map);
            }
            self.when_to_deduplicate = (self.when_to_deduplicate as f64 * 1.1) as usize;
        }
    }
}
