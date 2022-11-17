use crate::{
    compiler::error::CompilerError,
    database::Database,
    module::{Module, ModuleDb, Package},
};
use itertools::Itertools;
use lsp_types::{Diagnostic, DiagnosticSeverity, Position, Url};
use std::{ops::Range, sync::Arc};

impl CompilerError {
    pub fn into_diagnostic(self, db: &Database, module: Module) -> Diagnostic {
        Diagnostic {
            range: lsp_types::Range {
                start: db
                    .offset_to_lsp(module.clone(), self.span.start)
                    .to_position(),
                end: db.offset_to_lsp(module, self.span.end).to_position(),
            },
            severity: Some(DiagnosticSeverity::ERROR),
            code: None,
            code_description: None,
            source: Some("🍭 Candy".to_owned()),
            message: self.payload.to_string(),
            related_information: None,
            tags: None,
            data: None,
        }
    }
}

impl From<Module> for Option<Url> {
    fn from(module: Module) -> Option<Url> {
        match module.package {
            Package::User(_) | Package::External(_) => Some(
                Url::from_file_path(
                    module
                        .to_possible_paths()
                        .unwrap()
                        .into_iter()
                        .find_or_first(|path| path.exists())
                        .unwrap(),
                )
                .unwrap(),
            ),
            Package::Anonymous { url } => Some(Url::parse(&format!("untitled:{url}",)).unwrap()),
            Package::Tooling(_) => None,
        }
    }
}

// UTF-8 Byte Offset ↔ LSP Position/Range

impl Database {
    pub fn range_to_lsp(&self, module: Module, range: Range<usize>) -> lsp_types::Range {
        lsp_types::Range {
            start: self
                .offset_to_lsp(module.clone(), range.start)
                .to_position(),
            end: self.offset_to_lsp(module, range.end).to_position(),
        }
    }
}

#[salsa::query_group(LspPositionConversionStorage)]
pub trait LspPositionConversion: ModuleDb {
    // `lsp_types::Range` and `::Position` don't implement `Hash`, so they can't
    // be used as query keys directly.
    //
    // As its unlikely that a conversion is called multiple times with the same
    // key, we shouldn't loose much performance by not caching the individual
    // results.
    #[salsa::transparent]
    fn offset_from_lsp(&self, module: Module, line: u32, character: u32) -> usize;
    #[salsa::transparent]
    fn offset_to_lsp(&self, module: Module, position: usize) -> (u32, u32);

    fn line_start_utf8_byte_offsets(&self, module: Module) -> Arc<Vec<usize>>;
}

fn offset_from_lsp(
    db: &dyn LspPositionConversion,
    module: Module,
    line: u32,
    character: u32,
) -> usize {
    let text = db.get_module_content_as_string(module.clone()).unwrap();
    let line_start_offsets = db.line_start_utf8_byte_offsets(module);
    offset_from_lsp_raw(
        text.as_ref(),
        line_start_offsets.as_ref(),
        Position { line, character },
    )
}
pub fn offset_from_lsp_raw(text: &str, line_start_offsets: &[usize], position: Position) -> usize {
    let line_offset = line_start_offsets[position.line as usize];
    let line_length = if position.line as usize == line_start_offsets.len() - 1 {
        text.len() - line_offset
    } else {
        line_start_offsets[(position.line + 1) as usize] - line_offset
    };

    let line = &text[line_offset..line_offset + line_length];

    let words = line.encode_utf16().collect::<Vec<_>>();
    let char_offset = if position.character as usize >= words.len() {
        line_length
    } else {
        String::from_utf16(&words[0..position.character as usize])
            .unwrap()
            .len()
    };

    line_offset + char_offset
}

fn offset_to_lsp(db: &dyn LspPositionConversion, module: Module, mut offset: usize) -> (u32, u32) {
    let text = db.get_module_content_as_string(module.clone()).unwrap();
    if offset > text.len() {
        offset = text.len();
    }
    let line_start_offsets = db.line_start_utf8_byte_offsets(module);

    let line = line_start_offsets
        .binary_search(&offset)
        .unwrap_or_else(|i| i - 1);

    let line_start = line_start_offsets[line];
    let character_utf16_offset = text[line_start..offset.to_owned()].encode_utf16().count();
    (line as u32, character_utf16_offset as u32)
}

pub trait TupleToPosition {
    fn to_position(&self) -> Position;
}
impl TupleToPosition for (u32, u32) {
    fn to_position(&self) -> Position {
        Position {
            line: self.0,
            character: self.1,
        }
    }
}

fn line_start_utf8_byte_offsets(db: &dyn LspPositionConversion, module: Module) -> Arc<Vec<usize>> {
    let text = db.get_module_content_as_string(module).unwrap();
    Arc::new(line_start_utf8_byte_offsets_raw(&text))
}
pub fn line_start_utf8_byte_offsets_raw(text: &str) -> Vec<usize> {
    let mut offsets = vec![0];
    offsets.append(
        &mut text
            .bytes()
            .enumerate()
            .filter(|(_, it)| it == &b'\n')
            .map(|(index, _)| index + 1)
            .collect(),
    );
    offsets
}

pub trait JoinWithCommasAndAnd {
    fn join_with_commas_and_and(self) -> String;
}
impl JoinWithCommasAndAnd for Vec<String> {
    fn join_with_commas_and_and(mut self) -> String {
        match &self[..] {
            [] => panic!("Joining no parts."),
            [part] => part.to_string(),
            [first, second] => format!("{first} and {second}"),
            _ => {
                let last = self.pop().unwrap();
                format!("{}, and {last}", self.into_iter().join(", "))
            }
        }
    }
}
