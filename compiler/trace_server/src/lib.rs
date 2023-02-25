mod trace;
mod tracer;

use candy_frontend::{hir, module::Module, TracingConfig, TracingMode};
use candy_vm::{
    context::{PanickingUseProvider, RunForever, UseProvider},
    fiber::{ExecutionResult, FiberId},
    heap::{Closure, Struct},
    tracer::logical::LogicalTracer,
    vm::{Status, Vm},
};
use rustc_hash::FxHashMap;
use tracing::{debug, error, warn};

use crate::{trace::Trace, tracer::trace_call};

pub fn run<U: UseProvider>(use_provider: &U, module: Module, module_closure: Closure) {
    let tracing = TracingConfig {
        register_fuzzables: TracingMode::Off,
        calls: TracingMode::All,
        evaluated_expressions: TracingMode::All,
    };

    debug!("Running module.");
    let mut tracer = LogicalTracer::default();
    let mut vm = Vm::default();
    vm.set_up_for_running_module_closure(module.clone(), module_closure);
    vm.run(use_provider, &mut RunForever, &mut tracer);
    if let Status::WaitingForOperations = vm.status() {
        error!("The module waits on channel operations. Perhaps, the code tried to read from a channel without sending a packet into it.");
        // TODO: Show stack traces of all fibers?
        return;
    }
    let result = vm.tear_down();

    let (mut heap, exported_definitions): (_, Struct) = match result {
        ExecutionResult::Finished(return_value) => {
            debug!("The module exports these definitions: {return_value:?}",);
            let exported = return_value
                .heap
                .get(return_value.address)
                .data
                .clone()
                .try_into()
                .unwrap();
            (return_value.heap, exported)
        }
        ExecutionResult::Panicked {
            reason,
            responsible,
        } => {
            error!("The module panicked: {reason}");
            error!("{responsible} is responsible.");
            return;
        }
    };

    let main = heap.create_symbol("Main".to_string());
    let main = match exported_definitions.get(&heap, main) {
        Some(main) => main,
        None => {
            error!("The module doesn't contain a main function.");
            return;
        }
    };

    debug!("Running main function.");
    let mut vm = Vm::default();
    let environment = heap.create_struct(FxHashMap::default());
    let platform = heap.create_hir_id(hir::Id::platform());
    let root = trace_call(&mut heap, platform, main, vec![environment], platform);
    let trace = Trace { heap, root };

    debug!("Trace:\n{trace:?}");
}
