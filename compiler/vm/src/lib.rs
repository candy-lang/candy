#![feature(
    allocator_api,
    anonymous_lifetime_in_impl_trait,
    let_chains,
    slice_ptr_get,
    strict_provenance
)]

use crate::heap::{Struct, Tag};
use channel::Packet;
use context::RunForever;
use fiber::ExecutionResult;
use heap::{Function, Heap, HeapObject};
use lir::Lir;
use rustc_hash::FxHashMap;
use std::borrow::Borrow;
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
mod utils;
pub mod vm;

impl<L: Borrow<Lir>> Vm<L> {
    pub fn run_until_completion(mut self, tracer: &mut impl Tracer) -> ExecutionResult {
        self.run(&mut RunForever, tracer);
        if let Status::WaitingForOperations = self.status() {
            error!("The module waits on channel operations. Perhaps, the code tried to read from a channel without sending a packet into it.");
            // TODO: Show stack traces of all fibers?
        }
        self.tear_down()
    }
}

impl Packet {
    pub fn into_main_function(mut self) -> Result<(Heap, Function), &'static str> {
        let exported_definitions: Struct = self.object.try_into().unwrap();
        debug!("The module exports these definitions: {exported_definitions}");

        let main = Tag::create_from_str(&mut self.heap, "Main", None);
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
    pub fn into_main_function(
        self,
    ) -> Result<(Heap, Function, FxHashMap<HeapObject, HeapObject>), String> {
        match self {
            ExecutionResult::Finished {
                packet,
                constant_mapping,
            } => match packet.into_main_function() {
                Ok((heap, function)) => Ok((heap, function, constant_mapping)),
                Err(err) => Err(err.to_string()),
            },
            ExecutionResult::Panicked {
                reason,
                responsible,
            } => Err(format!("The module panicked at {responsible}: {reason}")),
        }
    }
}
