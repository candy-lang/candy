use super::{ast::AstError, hir::HirError, rcst::RcstError};
use crate::module::Module;
use std::ops::Range;

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct CompilerError {
    pub module: Module,
    pub span: Range<usize>,
    pub payload: CompilerErrorPayload,
}

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub enum CompilerErrorPayload {
    InvalidUtf8,
    Rcst(RcstError),
    Ast(AstError),
    Hir(HirError),
}
