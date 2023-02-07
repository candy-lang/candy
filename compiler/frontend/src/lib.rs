#![feature(box_patterns)]
#![feature(entry_insert)]
#![feature(let_chains)]

pub mod ast;
pub mod ast_to_hir;
pub mod builtin_functions;
pub mod comment;
pub mod cst;
pub mod cst_to_ast;
pub mod error;
pub mod hir;
pub mod hir_to_mir;
pub mod id;
pub mod mir;
pub mod mir_optimize;
pub mod module;
pub mod position;
pub mod rcst;
pub mod rcst_to_cst;
pub mod string_to_rcst;
pub mod tracing;
mod utils;

pub use self::tracing::{TracingConfig, TracingMode};
