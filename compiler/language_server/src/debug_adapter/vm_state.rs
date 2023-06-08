use super::{tracer::DebugTracer, utils::FiberIdThreadIdConversion};
use candy_frontend::id::CountableId;
use candy_vm::{
    fiber::FiberId,
    lir::Lir,
    vm::{FiberTree, Vm},
};
use dap::types::Thread;
use std::rc::Rc;

pub struct VmState {
    pub vm: Vm<Rc<Lir>, DebugTracer>,
    pub tracer: DebugTracer,
}

impl VmState {
    pub fn threads(&self) -> Vec<Thread> {
        self.vm
            .fibers()
            .iter()
            .map(|(id, fiber)| Thread {
                // FIXME: Use data from tracer?
                id: id.to_thread_id(),
                // TODO: indicate hierarchy
                name: format!(
                    "Fiber {}{}{}",
                    id.to_usize(),
                    if *id == FiberId::root() {
                        " (root)"
                    } else {
                        ""
                    },
                    match fiber {
                        FiberTree::Single(_) => "",
                        FiberTree::Parallel(_) => " (in `parallel`)",
                        FiberTree::Try(_) => " (in `try`)",
                    },
                ),
            })
            .collect()
    }
}
