use lsp_types::Position;
use serde::{Deserialize, Serialize};

#[derive(PartialEq, Eq, Debug, Clone, Serialize, Deserialize)]
pub struct Hint {
    pub kind: HintKind,
    pub text: String,
    pub position: Position,
}
#[derive(PartialEq, Eq, Debug, Clone, Serialize, Deserialize, PartialOrd, Ord, Copy)]
#[serde(rename_all = "camelCase")]
pub enum HintKind {
    Value,
    Panic,
}

impl Hint {
    pub fn like_comment(kind: HintKind, comment: String, end_of_line: Position) -> Self {
        Self {
            kind,
            text: format!("  # {}", comment.replace('\n', r#"\n"#)),
            position: end_of_line,
        }
    }
}
