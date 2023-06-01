use lsp_types::Position;
use serde::{Deserialize, Serialize};
use std::ops::Range;

#[derive(PartialEq, Eq, Debug, Clone, Serialize, Deserialize)]
pub struct Hint {
    pub kind: HintKind,
    pub text: Option<String>,
    pub range: Range<Position>,
}
#[derive(PartialEq, Eq, Debug, Clone, Serialize, Deserialize, PartialOrd, Ord, Copy)]
#[serde(rename_all = "camelCase")]
pub enum HintKind {
    Value,
    Fuzz,
    FuzzCallSite,
    Panic,
}

impl Hint {
    pub fn like_comment(kind: HintKind, comment: String, end_of_line: Position) -> Self {
        Self {
            kind,
            text: Some(format!("  # {}", comment.replace('\n', r#"\n"#))),
            range: end_of_line..end_of_line,
        }
    }

    /// In editor decorations, VSCode trims multiple leading spaces to one.
    /// That's why we use a blank Braille Pattern at the beginning of the hints.
    pub fn ensure_leading_spaces_visible(&mut self) {
        if let Some(text) = &mut self.text {
            *text = format!("⠀{text}");
        }
    }
}

pub fn align_hints(hints: &mut [&mut Hint]) {
    assert!(!hints.is_empty());
    assert!(hints.iter().all(|hint| hint.text.is_some()));

    let max_indentation = hints
        .iter()
        .map(|it| it.range.start.character)
        .max()
        .unwrap();
    for hint in hints {
        let hint = &mut **hint;
        let additional_indentation = max_indentation - hint.range.start.character;
        hint.text = Some(format!(
            "{}{}",
            " ".repeat(additional_indentation as usize),
            hint.text.as_ref().unwrap()
        ));
    }
}
