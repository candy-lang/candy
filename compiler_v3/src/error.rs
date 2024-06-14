use crate::position::{Offset, RangeOfOffset, RangeOfPosition};
use std::{ops::Range, path::PathBuf};

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct CompilerError {
    pub path: PathBuf,
    pub span: Range<Offset>,
    pub message: String,
}
impl CompilerError {
    pub fn to_string_with_location(&self, source: &str) -> String {
        format!(
            "{}:{}: {}",
            self.path.display(),
            self.span.to_positions(source).format(),
            self.message
        )
    }
}
