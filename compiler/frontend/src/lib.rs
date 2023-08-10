#![feature(
    anonymous_lifetime_in_impl_trait,
    box_patterns,
    entry_insert,
    hasher_prefixfree_extras,
    io_error_more,
    let_chains,
    try_blocks
)]

pub use self::tracing::{TracingConfig, TracingMode};

pub mod ast;
pub mod ast_to_hir;
pub mod builtin_functions;
pub mod comment;
pub mod cst;
pub mod cst_to_ast;
pub mod error;
pub mod format;
pub mod hir;
pub mod hir_to_mir;
pub mod id;
pub mod lir;
pub mod mir;
pub mod mir_optimize;
pub mod mir_to_lir;
pub mod module;
pub mod position;
pub mod rcst;
pub mod rcst_to_cst;
pub mod rich_ir;
pub mod string_to_rcst;
pub mod tracing;
pub mod utils;
