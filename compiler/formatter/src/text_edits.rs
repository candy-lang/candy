use candy_frontend::position::Offset;
use std::{borrow::Cow, ops::Range};

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct TextEdit {
    pub range: Range<Offset>,
    pub new_text: String,
}
impl TextEdit {
    fn is_insert(&self) -> bool {
        self.range.is_empty()
    }
}

/// All text edits ranges refer to positions in the document they are computed on. They therefore
/// move a document from state S1 to S2 without describing any intermediate state. Text edits ranges
/// must never overlap, that means no part of the original document must be manipulated by more than
/// one edit. However, it is possible that multiple edits have the same start position: multiple
/// inserts, or any number of inserts followed by a single remove or replace edit. If multiple
/// inserts have the same position, the order in the array defines the order in which the inserted
/// strings appear in the resulting text.
///
/// <https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/#textEditArray>
pub struct TextEdits {
    source: String,

    /// The edits are sorted by their start position.
    edits: Vec<TextEdit>,
}
impl TextEdits {
    pub fn new(source: String) -> Self {
        Self {
            source,
            edits: vec![],
        }
    }

    pub fn source(&self) -> &str {
        &self.source
    }
    pub fn has_edits(&self) -> bool {
        !self.edits.is_empty()
    }
    #[allow(clippy::map_unwrap_or)]
    pub fn has_edit_at(&self, offset: Offset) -> bool {
        self.edits
            .binary_search_by_key(&offset, |it| it.range.start)
            .map(|_| true) // An edit starts at this position.
            .unwrap_or_else(|index| {
                self.edits
                    .get(index)
                    // An edit contains this position.
                    .is_some_and(|it| it.range.contains(&offset))
            })
    }

    pub fn insert(&mut self, offset: Offset, text: impl Into<Cow<str>>) {
        self.change(offset..offset, text);
    }
    pub fn delete(&mut self, range: Range<Offset>) {
        self.change(range, "");
    }
    pub fn change(&mut self, range: Range<Offset>, new_text: impl Into<Cow<str>>) {
        let new_text = new_text.into();
        if self.source[*range.start..*range.end] == new_text {
            return;
        }

        let index = self
            .edits
            .binary_search_by_key(&range.start, |it| it.range.start);
        match index {
            Ok(index) => {
                let existing = &mut self.edits[index];
                assert!(
                    existing.is_insert() || range.is_empty(),
                    "At least one of [existing, new] must be an insert.",
                );

                if existing.range.is_empty() {
                    existing.range = range;
                }
                existing.new_text = format!("{}{}", existing.new_text, new_text);
            }
            Err(index) => {
                if index > 0 {
                    let previous = &self.edits[index - 1];
                    assert!(previous.range.end <= range.start);
                    if previous.range.end == range.start {
                        self.edits[index - 1] = TextEdit {
                            range: previous.range.start..range.end,
                            new_text: format!("{}{}", previous.new_text, new_text),
                        };
                        return;
                    }
                }
                if index < self.edits.len() {
                    let next = &self.edits[index];
                    assert!(range.end <= next.range.start);
                    if range.end == next.range.start {
                        self.edits[index] = TextEdit {
                            range: range.start..next.range.end,
                            new_text: format!("{}{}", new_text, next.new_text),
                        };
                        return;
                    }
                }
                self.edits.insert(
                    index,
                    TextEdit {
                        range,
                        new_text: new_text.into(),
                    },
                );
            }
        }
    }

    pub fn finish(self) -> Vec<TextEdit> {
        self.edits
    }
    pub fn apply(&self) -> String {
        let mut result = self.source.to_string();
        for edit in self.edits.iter().rev() {
            result.replace_range(*edit.range.start..*edit.range.end, &edit.new_text);
        }
        result
    }
}
