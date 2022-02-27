use std::ops::Range;

use lsp_types::{Diagnostic, DiagnosticSeverity, Position, Url};

use crate::{
    compiler::error::CompilerError,
    database::Database,
    input::{Input, InputDb},
};

impl CompilerError {
    pub fn to_diagnostic(self, db: &Database, input: Input) -> Diagnostic {
        Diagnostic {
            range: lsp_types::Range {
                start: db
                    .offset_to_lsp(input.clone(), self.span.start)
                    .to_position(),
                end: db.offset_to_lsp(input, self.span.end).to_position(),
            },
            severity: Some(DiagnosticSeverity::ERROR),
            code: None,
            code_description: None,
            source: Some("🍭 Candy".to_owned()),
            message: self.message,
            related_information: None,
            tags: None,
            data: None,
        }
    }
}

impl From<Url> for Input {
    fn from(uri: Url) -> Input {
        match uri.scheme() {
            "file" => Input::File(uri.to_file_path().unwrap()),
            "untitled" => Input::Untitled(uri.to_string()["untitled:".len()..].to_owned()),
            _ => panic!("Unsupported URI scheme: {}", uri.scheme()),
        }
    }
}

impl From<Input> for Url {
    fn from(input: Input) -> Url {
        match input {
            Input::File(path) => Url::from_file_path(path).unwrap(),
            Input::Untitled(id) => Url::parse(&format!("untitled:{}", id)).unwrap(),
        }
    }
}

// UTF-8 Byte Offset ↔ LSP Position/Range

impl Database {
    pub fn range_to_lsp(&self, input: Input, range: Range<usize>) -> lsp_types::Range {
        lsp_types::Range {
            start: self.offset_to_lsp(input.clone(), range.start).to_position(),
            end: self.offset_to_lsp(input, range.end).to_position(),
        }
    }
}

#[salsa::query_group(LspPositionConversionStorage)]
pub trait LspPositionConversion: InputDb {
    // `lsp_types::Range` and `::Position` don't implement `Hash`, so they can't
    // be used as query keys directly.
    //
    // As its unlikely that a conversion is called multiple times with the same
    // key, we shouldn't loose much performance by not caching the individual
    // results.
    #[salsa::transparent]
    fn offset_from_lsp(&self, input: Input, line: u32, character: u32) -> usize;
    #[salsa::transparent]
    fn offset_to_lsp(&self, input: Input, position: usize) -> (u32, u32);

    fn line_start_utf8_byte_offsets(&self, input: Input) -> Vec<usize>;
}

fn offset_from_lsp(
    db: &dyn LspPositionConversion,
    input: Input,
    line: u32,
    character: u32,
) -> usize {
    let text = db.get_input(input.clone()).unwrap();
    let line_start_offsets = db.line_start_utf8_byte_offsets(input);

    let line_offset = line_start_offsets[line as usize];
    let line_length = if line as usize == line_start_offsets.len() - 1 {
        text.len()
    } else {
        line_start_offsets[(line + 1) as usize] - line_offset
    };

    let line = &text[line_offset..line_offset + line_length];

    let words = line.encode_utf16().collect::<Vec<_>>();
    let char_offset = if character as usize >= words.len() {
        line_length
    } else {
        String::from_utf16(&words[0..character as usize])
            .unwrap()
            .len()
    };

    line_offset + char_offset
}

fn offset_to_lsp(db: &dyn LspPositionConversion, input: Input, offset: usize) -> (u32, u32) {
    let text = db.get_input(input.clone()).unwrap();
    let line_start_offsets = db.line_start_utf8_byte_offsets(input);

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

fn line_start_utf8_byte_offsets(db: &dyn LspPositionConversion, input: Input) -> Vec<usize> {
    let text = db.get_input(input).unwrap();
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
