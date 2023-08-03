#![feature(
    allocator_api,
    anonymous_lifetime_in_impl_trait,
    core_intrinsics,
    fmt_internals,
    iterator_try_collect,
    let_chains,
    nonzero_ops,
    slice_ptr_get,
    step_trait,
    strict_provenance
)]

use crate::heap::{DisplayWithSymbolTable, Struct, SymbolId, Tag};
use execution_controller::RunForever;
use fiber::{EndedReason, VmEnded};
use heap::{Function, Heap, InlineObject, SymbolTable};
use lir::Lir;
use std::borrow::Borrow;
use tracer::Tracer;
use tracing::{debug, error};
use vm::{Status, Vm};

mod builtin_functions;
pub mod channel;
pub mod execution_controller;
pub mod fiber;
pub mod heap;
pub mod lir;
pub mod mir_to_lir;
pub mod tracer;
mod utils;
pub mod vm;

impl<L: Borrow<Lir>, T: Tracer> Vm<L, T> {
    pub fn run_until_completion(mut self, tracer: &mut T) -> VmEnded {
        self.run(&mut RunForever, tracer);
        if let Status::WaitingForOperations = self.status() {
            error!("The module waits on channel operations. Perhaps, the code tried to read from a channel without sending a packet into it.");
            // TODO: Show stack traces of all fibers?
        }
        self.tear_down(tracer)
    }
}

impl VmEnded {
    pub fn into_main_function(
        self,
        symbol_table: &SymbolTable,
    ) -> Result<(Heap, Function), String> {
        match self.reason {
            EndedReason::Finished(return_value) => {
                match return_value_into_main_function(symbol_table, return_value) {
                    Ok(main) => Ok((self.heap, main)),
                    Err(err) => Err(err.to_string()),
                }
            }
            EndedReason::Panicked(panic) => Err(format!(
                "The module panicked at {}: {}",
                panic.responsible, panic.reason,
            )),
        }
    }
}

pub fn return_value_into_main_function(
    symbol_table: &SymbolTable,
    return_value: InlineObject,
) -> Result<Function, &'static str> {
    let exported_definitions: Struct = return_value.try_into().unwrap();
    debug!(
        "The module exports these definitions: {}",
        DisplayWithSymbolTable::to_string(&exported_definitions, symbol_table),
    );

    exported_definitions
        .get(Tag::create(SymbolId::MAIN))
        .ok_or("The module doesn't export a main function.")
        .and_then(|main| {
            main.try_into()
                .map_err(|_| "The exported main object is not a function.")
        })
}
