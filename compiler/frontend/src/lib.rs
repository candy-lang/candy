#![feature(box_patterns, entry_insert, io_error_more, let_chains)]
#![warn(clippy::nursery, clippy::pedantic)]
#![allow(
    clippy::cognitive_complexity,
    clippy::match_same_arms,
    clippy::missing_errors_doc,
    clippy::missing_panics_doc,
    clippy::module_name_repetitions,
    clippy::similar_names,
    clippy::too_many_lines
)]

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
pub mod mir;
pub mod mir_optimize;
pub mod module;
pub mod position;
pub mod rcst;
pub mod rcst_to_cst;
pub mod rich_ir;
pub mod string_to_rcst;
pub mod tracing;
pub mod utils;

pub use self::tracing::{TracingConfig, TracingMode};
