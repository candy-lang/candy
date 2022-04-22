use std::ops::Range;

use super::{ast::AstError, hir::HirError, rcst::RcstError};
use crate::input::Input;

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct CompilerError {
    pub input: Input,
    pub span: Range<usize>,
    pub payload: CompilerErrorPayload,
}

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub enum CompilerErrorPayload {
    Rcst(RcstError),
    Ast(AstError),
    Hir(HirError),
}
