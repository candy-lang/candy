use std::{
    ops::{Deref, DerefMut, Range},
    sync::Arc,
};

use unicode_segmentation::UnicodeSegmentation;

use crate::module::{Module, ModuleDb};

/// The offset of a character in a string as the number of bytes preceding it in
/// UTF-8 encoding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Offset(pub usize);

impl From<usize> for Offset {
    fn from(offset: usize) -> Self {
        Offset(offset)
    }
}
impl Deref for Offset {
    type Target = usize;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl DerefMut for Offset {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Position {
    /// Zero-based line index (`\n`-separated)
    pub line: usize,
    /// Zero-based character index (counting grapheme clusters)
    pub character: usize,
}

#[salsa::query_group(PositionConversionStorage)]
pub trait PositionConversionDb: ModuleDb {
    // As its unlikely that a conversion is called multiple times with the same
    // key, we shouldn't loose much performance by not caching the individual
    // results.
    #[salsa::transparent]
    fn range_to_positions(&self, module: Module, range: Range<Offset>) -> Range<Position>;
    #[salsa::transparent]
    fn offset_to_position(&self, module: Module, position: Offset) -> Position;

    fn line_start_offsets(&self, module: Module) -> Arc<Vec<Offset>>;
}

fn range_to_positions(
    db: &dyn PositionConversionDb,
    module: Module,
    range: Range<Offset>,
) -> Range<Position> {
    let start = db.offset_to_position(module.clone(), range.start);
    let end = db.offset_to_position(module, range.end);
    start..end
}
fn offset_to_position(
    db: &dyn PositionConversionDb,
    module: Module,
    mut offset: Offset,
) -> Position {
    let Some(text) = db.get_module_content_as_string(module.clone()) else {
        assert_eq!(*offset, 0);
        return Position {
            line: 0,
            character: 0,
        };
    };
    if *offset > text.len() {
        *offset = text.len();
    }
    let line_start_offsets = db.line_start_offsets(module);

    let line = line_start_offsets
        .binary_search(&offset)
        .unwrap_or_else(|i| i - 1);

    let character = text[*line_start_offsets[line]..*offset]
        .graphemes(true)
        .count();
    Position { line, character }
}

fn line_start_offsets(db: &dyn PositionConversionDb, module: Module) -> Arc<Vec<Offset>> {
    let text = db.get_module_content_as_string(module).unwrap();
    Arc::new(line_start_offsets_raw(&text))
}
pub fn line_start_offsets_raw(text: &str) -> Vec<Offset> {
    let mut offsets = vec![Offset(0)];
    offsets.extend(
        text.bytes()
            .enumerate()
            .filter(|(_, it)| it == &b'\n')
            .map(|(index, _)| Offset(index + 1)),
    );
    offsets
}
