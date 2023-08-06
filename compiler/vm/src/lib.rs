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
use heap::{Function, InlineObject, SymbolTable};
pub use instruction_pointer::InstructionPointer;
use tracing::debug;
pub use utils::PopulateInMemoryProviderFromFileSystem;
pub use vm::{
    Panic, StateAfterRun, StateAfterRunForever, StateAfterRunWithoutHandles, Vm, VmPanicked,
    VmReturned,
};

mod builtin_functions;
mod handle;
pub mod heap;
mod instruction_pointer;
mod instructions;
pub mod lir;
pub mod mir_to_lir;
pub mod tracer;
mod utils;
mod vm;

impl InlineObject {
    pub fn into_main_function(self, symbol_table: &SymbolTable) -> Result<Function, &'static str> {
        let exported_definitions: Struct = self.try_into().unwrap();
        debug!(
            "The module exports these definitions: {}",
            DisplayWithSymbolTable::to_string(&exported_definitions, symbol_table),
        );

        exported_definitions
            .get(Tag::create(SymbolId::MAIN))
            .ok_or("The module doesn't export a main function.")
            .and_then(|main| {
                main.try_into()
                    .map_err(|_| "The exported main value is not a function.")
            })
    }
}
