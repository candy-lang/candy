use std::ops::Range;

use lsp_types::Position;

pub trait RangeToUtf8ByteOffset {
    fn to_utf8_byte_offset(&self, text: &str) -> Range<usize>;
}
impl RangeToUtf8ByteOffset for lsp_types::Range {
    fn to_utf8_byte_offset(&self, text: &str) -> Range<usize> {
        let start = self.start.to_utf8_byte_offset(text);
        let end = self.end.to_utf8_byte_offset(text);
        start..end
    }
}

pub trait PositionToUtf8ByteOffset {
    fn to_utf8_byte_offset(&self, text: &str) -> usize;
}
impl PositionToUtf8ByteOffset for Position {
    fn to_utf8_byte_offset(&self, text: &str) -> usize {
        let mut line_index = 0;
        let mut line_offset = 0;
        while line_index < self.line {
            match text.bytes().nth(line_offset).unwrap() {
                b'\n' => {
                    line_index += 1;
                    line_offset += 1;
                }
                _ => {
                    line_offset += 1;
                }
            }
        }

        let mut line_length_bytes = 0;
        loop {
            match text.bytes().nth(line_offset + line_length_bytes) {
                Some(b'\r' | b'\n') | None => break,
                Some(_) => line_length_bytes += 1,
            }
        }

        let line = &text[line_offset..line_offset + line_length_bytes];

        let words = line.encode_utf16().collect::<Vec<_>>();
        let char_offset = if self.character as usize >= words.len() {
            line_length_bytes
        } else {
            String::from_utf16(&words[0..self.character as usize])
                .unwrap()
                .len()
        };

        line_offset + char_offset
    }
}
