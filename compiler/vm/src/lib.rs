#![feature(
    allocator_api,
    anonymous_lifetime_in_impl_trait,
    let_chains,
    slice_ptr_get,
    strict_provenance
)]

use crate::heap::{Struct, Symbol};
use candy_frontend::{hir, module::Module};
use channel::Packet;
use context::{RunForever, UseProvider};
use fiber::ExecutionResult;
use heap::{Closure, Heap};
use lir::Lir;
use tracer::Tracer;
use tracing::{debug, error};
use vm::{Status, Vm};

mod builtin_functions;
pub mod channel;
pub mod context;
pub mod fiber;
pub mod heap;
pub mod lir;
pub mod mir_to_lir;
pub mod tracer;
mod use_module;
mod utils;
pub mod vm;

pub fn run_lir(
    module: Module,
    lir: Lir,
    use_provider: &impl UseProvider,
    tracer: &mut impl Tracer,
) -> ExecutionResult {
    let mut heap = Heap::default();
    let closure = Closure::create_from_module_lir(&mut heap, lir);

    let mut vm = Vm::default();
    vm.set_up_for_running_module_closure(heap, module, closure);

    vm.run(use_provider, &mut RunForever, tracer);
    if let Status::WaitingForOperations = vm.status() {
        error!("The module waits on channel operations. Perhaps, the code tried to read from a channel without sending a packet into it.");
        // TODO: Show stack traces of all fibers?
    }
    vm.tear_down()
}

impl Packet {
    pub fn into_main_function(mut self) -> Result<(Heap, Closure), &'static str> {
        let exported_definitions: Struct = self.object.try_into().unwrap();
        debug!("The module exports these definitions: {exported_definitions}");

        let main = Symbol::create(&mut self.heap, "Main");
        exported_definitions
            .get(main)
            .ok_or("The module doesn't export a main function.")
            .and_then(|main| {
                main.try_into()
                    .map_err(|_| "The exported main object is not a function.")
            })
            .map(|main| (self.heap, main))
    }
}

impl ExecutionResult {
    pub fn into_main_function(self) -> Result<(Heap, Closure), String> {
        match self {
            ExecutionResult::Finished(packet) => {
                packet.into_main_function().map_err(|it| it.to_string())
            }
            ExecutionResult::Panicked {
                reason,
                responsible,
            } => Err(format!("The module panicked at {responsible}: {reason}")),
        }
    }
}

pub fn run_main(
    heap: Heap,
    main: Closure,
    environment: Struct,
    use_provider: &impl UseProvider,
    tracer: &mut impl Tracer,
) -> ExecutionResult {
    let mut vm = Vm::default();
    vm.set_up_for_running_closure(heap, main, &[environment.into()], hir::Id::platform());
    vm.run(use_provider, &mut RunForever, tracer);
    vm.tear_down()
}
