use candy_frontend::hir::Id;
use candy_vm::{
    self,
    channel::Packet,
    execution_controller::{CountingExecutionController, ExecutionController},
    fiber::{EndedReason, Fiber, FiberId, InstructionPointer, Panic, VmEnded},
    heap::{Function, Heap, HirId, InlineObject, InlineObjectSliceCloneToHeap},
    lir::Lir,
    tracer::{FiberTracer, Tracer},
    vm::{self, Vm},
};

use super::input::Input;
use crate::{hir_coverage::HirCoverage, lir_coverage::LirCoverage};
use rustc_hash::FxHashMap;
use std::{borrow::Borrow, rc::Rc, sync::RwLock};
use tracing::info;

const MAX_INSTRUCTIONS: usize = 1000000;

pub struct Runner<L: Borrow<Lir>> {
    pub vm: Option<Vm<L, HirCoverageTracer>>, // Is consumed when the runner is finished.
    pub input: Input,
    pub tracer: HirCoverageTracer,
    pub num_instructions: usize,
    pub lir_coverage: LirCoverage,
    pub hir_coverage: Rc<RwLock<HirCoverage>>,
    pub result: Option<RunResult>,
}

pub enum RunResult {
    /// Executing the function with the input took more than `MAX_INSTRUCTIONS`.
    Timeout,

    /// The execution finished successfully with a value.
    Done(Packet),

    /// The execution panicked and the caller of the function (aka the fuzzer)
    /// is at fault.
    NeedsUnfulfilled { reason: String },

    /// The execution panicked with an internal panic. This indicates an error
    /// in the code that should be shown to the user.
    Panicked(Panic),
}
impl RunResult {
    pub fn to_string(&self, call: &str) -> String {
        match self {
            RunResult::Timeout => format!("{call} timed out."),
            RunResult::Done(return_value) => format!("{call} returned {}.", return_value.object),
            RunResult::NeedsUnfulfilled { reason } => {
                format!("{call} panicked and it's our fault: {reason}")
            }
            RunResult::Panicked(panic) => {
                format!("{call} panicked internally: {}", panic.reason)
            }
        }
    }
}

impl<L: Borrow<Lir>> Runner<L> {
    pub fn new(lir: L, function: Function, input: Input) -> Self {
        let (mut heap, constant_mapping) = lir.borrow().constant_heap.clone();
        let num_instructions = lir.borrow().instructions.len();

        let mut mapping = FxHashMap::default();
        let function = function
            .clone_to_heap_with_mapping(&mut heap, &mut mapping)
            .try_into()
            .unwrap();
        let arguments = input
            .arguments
            .clone_to_heap_with_mapping(&mut heap, &mut mapping);
        let responsible = HirId::create(&mut heap, Id::fuzzer());

        let hir_coverage = Rc::new(RwLock::new(HirCoverage::none()));
        let mut tracer = HirCoverageTracer::new(hir_coverage.clone());
        let vm = Vm::for_function(
            lir,
            heap,
            constant_mapping,
            function,
            &arguments,
            responsible,
            &mut tracer,
        );

        Runner {
            vm: Some(vm),
            input,
            tracer,
            num_instructions: 0,
            lir_coverage: LirCoverage::none(num_instructions),
            hir_coverage,
            result: None,
        }
    }

    pub fn run(&mut self, execution_controller: &mut impl ExecutionController<HirCoverageTracer>) {
        assert!(self.vm.is_some());
        assert!(self.result.is_none());

        let mut coverage_tracker = LirCoverageTrackingExecutionController {
            coverage: &mut self.lir_coverage,
        };
        let mut instruction_counter = CountingExecutionController::default();
        let mut execution_controller = (
            execution_controller,
            &mut coverage_tracker,
            &mut instruction_counter,
        );

        self.vm
            .as_mut()
            .unwrap()
            .run(&mut execution_controller, &mut self.tracer);

        self.num_instructions += instruction_counter.num_instructions;

        self.result = match self.vm.as_ref().unwrap().status() {
            vm::Status::CanRun => {
                if self.num_instructions > MAX_INSTRUCTIONS {
                    Some(RunResult::Timeout)
                } else {
                    None
                }
            }
            // Because the fuzzer never sends channels as inputs, the function
            // waits on some internal concurrency operations that will never be
            // completed. This most likely indicates an error in the code, but
            // it's of course valid to have a function that never returns. Thus,
            // this should be treated just like a regular timeout.
            vm::Status::WaitingForOperations => Some(RunResult::Timeout),
            vm::Status::Done => {
                let VmEnded { heap, reason, .. } =
                    self.vm.take().unwrap().tear_down(&mut self.tracer);
                let EndedReason::Finished(return_value) = reason else {
                    unreachable!();
                };
                Some(RunResult::Done(Packet {
                    heap,
                    object: return_value,
                }))
            }
            vm::Status::Panicked(panic) => Some(if panic.responsible == Id::fuzzer() {
                RunResult::NeedsUnfulfilled {
                    reason: panic.reason,
                }
            } else {
                self.vm.take().unwrap().tear_down(&mut self.tracer);
                RunResult::Panicked(panic)
            }),
        };
    }
}

pub struct LirCoverageTrackingExecutionController<'a> {
    coverage: &'a mut LirCoverage,
}
impl<'a, T: FiberTracer> ExecutionController<T> for LirCoverageTrackingExecutionController<'a> {
    fn should_continue_running(&self) -> bool {
        true
    }

    fn instruction_executed(
        &mut self,
        _fiber_id: FiberId,
        _fiber: &Fiber<T>,
        ip: InstructionPointer,
    ) {
        self.coverage.add(ip);
    }
}

pub struct HirCoverageTracer(Rc<RwLock<HirCoverage>>);
impl HirCoverageTracer {
    fn new(coverage: Rc<RwLock<HirCoverage>>) -> Self {
        Self(coverage)
    }
}
impl Tracer for HirCoverageTracer {
    type ForFiber = HirCoverageTracer;

    fn root_fiber_created(&mut self) -> Self::ForFiber {
        HirCoverageTracer(self.0.clone())
    }
}
impl FiberTracer for HirCoverageTracer {
    fn child_fiber_created(&mut self, _child: FiberId) -> Self {
        HirCoverageTracer(self.0.clone())
    }

    fn dup_all_stored_objects(&self, _heap: &mut Heap) {}

    fn value_evaluated(&mut self, _heap: &mut Heap, expression: HirId, _value: InlineObject) {
        info!("Value evaluated: {expression}");
        self.0.write().unwrap().add(expression.get().clone())
    }
}
