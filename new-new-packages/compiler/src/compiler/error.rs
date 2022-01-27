use std::ops::Range;

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct CompilerError {
    pub span: Range<usize>,
    pub message: String,
}
