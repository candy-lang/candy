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
    strict_provenance,
    try_blocks
)]
#![warn(clippy::nursery, clippy::pedantic)]
#![allow(
    clippy::large_enum_variant,
    clippy::match_same_arms,
    clippy::missing_errors_doc,
    clippy::missing_panics_doc,
    clippy::module_name_repetitions,
    clippy::similar_names,
    clippy::too_many_lines
)]

use crate::heap::{Struct, Tag};
pub use builtin_functions::CAN_USE_STDOUT;
use heap::{Function, Heap, InlineObject};
pub use instruction_pointer::InstructionPointer;
use tracing::debug;
pub use utils::PopulateInMemoryProviderFromFileSystem;
pub use vm::{Panic, StateAfterRun, StateAfterRunForever, Vm, VmFinished};

mod builtin_functions;
pub mod byte_code;
pub mod environment;
mod handle_id;
pub mod heap;
mod instruction_pointer;
mod instructions;
pub mod mir_to_byte_code;
pub mod tracer;
mod utils;
mod vm;

impl InlineObject {
    pub fn into_main_function(self, heap: &Heap) -> Result<Function, &'static str> {
        let exported_definitions: Struct = self.try_into().unwrap();
        debug!("The module exports these definitions: {exported_definitions}",);

        exported_definitions
            .get(Tag::create(heap.default_symbols().main))
            .ok_or("The module doesn't export a main function.")
            .and_then(|main| {
                main.try_into()
                    .map_err(|_| "The exported main value is not a function.")
            })
    }
}
