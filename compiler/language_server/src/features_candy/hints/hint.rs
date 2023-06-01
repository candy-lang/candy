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

    /// In editor decorations, VSCode trims multiple leading spaces to one.
    /// That's why we use a blank Braille Pattern at the beginning of the hints.
    pub fn ensure_leading_spaces_visible(&mut self) {
        self.text = format!("⠀{}", self.text);
    }
}

pub fn align_hints(hints: &mut [&mut Hint]) {
    assert!(!hints.is_empty());

    let max_indentation = hints.iter().map(|it| it.position.character).max().unwrap();
    for hint in hints {
        let hint = &mut **hint;
        let additional_indentation = max_indentation - hint.position.character;
        hint.text = format!(
            "{}{}",
            " ".repeat(additional_indentation as usize),
            hint.text
        );
    }
}
