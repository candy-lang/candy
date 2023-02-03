use std::{ops::Range, sync::Arc};

use unicode_segmentation::UnicodeSegmentation;

use crate::module::{Module, ModuleDb};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Position {
    pub line: usize,
    pub character: usize,
}

// Offset: Number of bytes in UTF-8 encoding
// Position: Zero-based line (`\n`-separated) and character (grapheme cluster)
//   index

#[salsa::query_group(PositionConversionStorage)]
pub trait PositionConversionDb: ModuleDb {
    // As its unlikely that a conversion is called multiple times with the same
    // key, we shouldn't loose much performance by not caching the individual
    // results.
    #[salsa::transparent]
    fn range_to_positions(&self, module: Module, range: Range<usize>) -> Range<Position>;
    #[salsa::transparent]
    fn offset_to_position(&self, module: Module, position: usize) -> Position;

    fn line_start_offsets(&self, module: Module) -> Arc<Vec<usize>>;
}

fn range_to_positions(
    db: &dyn PositionConversionDb,
    module: Module,
    range: Range<usize>,
) -> Range<Position> {
    let start = db.offset_to_position(module.clone(), range.start);
    let end = db.offset_to_position(module, range.end);
    start..end
}
fn offset_to_position(
    db: &dyn PositionConversionDb,
    module: Module,
    mut offset: usize,
) -> Position {
    let text = db.get_module_content_as_string(module.clone()).unwrap();
    if offset > text.len() {
        offset = text.len();
    }
    let line_start_offsets = db.line_start_offsets(module);

    let line = line_start_offsets
        .binary_search(&offset)
        .unwrap_or_else(|i| i - 1);

    let character = text[line_start_offsets[line]..offset]
        .graphemes(true)
        .count();
    Position { line, character }
}

fn line_start_offsets(db: &dyn PositionConversionDb, module: Module) -> Arc<Vec<usize>> {
    let text = db.get_module_content_as_string(module).unwrap();
    Arc::new(line_start_offsets_raw(&text))
}
pub fn line_start_offsets_raw(text: &str) -> Vec<usize> {
    let mut offsets = vec![0];
    offsets.extend(
        text.bytes()
            .enumerate()
            .filter(|(_, it)| it == &b'\n')
            .map(|(index, _)| index + 1),
    );
    offsets
}
