mod storage;
mod time;
mod trace;
mod tracer;
mod web_server;

use std::sync::RwLock;

use actix_web::HttpServer;
use candy_frontend::{hir, module::Module, TracingConfig, TracingMode};
use candy_vm::{
    context::{PanickingUseProvider, RunForever, UseProvider},
    fiber::{ExecutionResult, FiberId},
    heap::{Closure, Struct},
    tracer::DummyTracer,
    vm::{Status, Vm},
};
use rustc_hash::FxHashMap;
use tracing::{debug, error, warn};

use crate::{storage::TraceStorage, trace::Trace, tracer::trace_call};

pub fn run<U: UseProvider>(use_provider: &U, module: Module, module_closure: Closure) {
    let tracing = TracingConfig {
        register_fuzzables: TracingMode::Off,
        calls: TracingMode::All,
        evaluated_expressions: TracingMode::All,
    };

    debug!("Running module.");
    let mut vm = Vm::default();
    vm.set_up_for_running_module_closure(module.clone(), module_closure);
    vm.run(use_provider, &mut RunForever, &mut DummyTracer::default());
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
    let mut storage = Arc::new(RwLock::new(TraceStorage::new(heap)));
    // std::thread::spawn(|| web_server::run(storage));

    let mut vm = Vm::default();
    let environment = storage.heap.create_struct(FxHashMap::default());
    let platform = storage.heap.create_hir_id(hir::Id::platform());
    let root = trace_call(&mut storage, platform, main, vec![environment], platform);

    debug!("Trace:\n{storage:?}");
    // web_server::run(storage);
}
