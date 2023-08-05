use super::tracer::DebugTracer;
use candy_vm::{lir::Lir, Vm};
use std::rc::Rc;

type DebugVm = Vm<Rc<Lir>, DebugTracer>;
