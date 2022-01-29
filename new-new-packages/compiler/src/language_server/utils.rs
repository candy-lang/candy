use lsp_types::{Diagnostic, DiagnosticSeverity, Position, Url};

use crate::{
    compiler::error::CompilerError,
    database::Database,
    input::{Input, InputReference},
};

impl CompilerError {
    pub fn to_diagnostic(self, db: &Database, input_reference: InputReference) -> Diagnostic {
        Diagnostic {
            range: lsp_types::Range {
                start: db
                    .utf8_byte_offset_to_lsp(self.span.start, input_reference.clone())
                    .to_position(),
                end: db
                    .utf8_byte_offset_to_lsp(self.span.end, input_reference)
                    .to_position(),
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

impl From<Url> for InputReference {
    fn from(uri: Url) -> InputReference {
        match uri.scheme() {
            "file" => InputReference::File(uri.to_file_path().unwrap()),
            "untitled" => InputReference::Untitled(uri.to_string()["untitled:".len()..].to_owned()),
            _ => panic!("Unsupported URI scheme: {}", uri.scheme()),
        }
    }
}

impl From<InputReference> for Url {
    fn from(input_reference: InputReference) -> Url {
        match input_reference {
            InputReference::File(path) => Url::from_file_path(path).unwrap(),
            InputReference::Untitled(id) => Url::parse(&format!("untitled:{}", id)).unwrap(),
        }
    }
}

// UTF-8 Byte Offset ↔ LSP Position/Range

#[salsa::query_group(LspPositionConversionStorage)]
pub trait LspPositionConversion: Input {
    // `lsp_types::Range` and `::Position` don't implement `Hash`, so they can't be
    // used as query keys directly.
    fn position_to_utf8_byte_offset(
        &self,
        line: u32,
        character: u32,
        input_reference: InputReference,
    ) -> usize;
    fn utf8_byte_offset_to_lsp(
        &self,
        position: usize,
        input_reference: InputReference,
    ) -> (u32, u32);

    fn line_start_utf8_byte_offsets(&self, input_reference: InputReference) -> Vec<usize>;
}

fn position_to_utf8_byte_offset(
    db: &dyn LspPositionConversion,
    line: u32,
    character: u32,
    input_reference: InputReference,
) -> usize {
    let text = db.get_input(input_reference.clone()).unwrap();
    let line_start_offsets = db.line_start_utf8_byte_offsets(input_reference);

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

fn utf8_byte_offset_to_lsp(
    db: &dyn LspPositionConversion,
    offset: usize,
    input_reference: InputReference,
) -> (u32, u32) {
    let text = db.get_input(input_reference.clone()).unwrap();
    let line_start_offsets = db.line_start_utf8_byte_offsets(input_reference);

    let line = line_start_offsets
        .binary_search(&offset)
        .unwrap_or_else(|i| i);

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

fn line_start_utf8_byte_offsets(
    db: &dyn LspPositionConversion,
    input_reference: InputReference,
) -> Vec<usize> {
    let text = db.get_input(input_reference).unwrap();
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
