use im::HashMap;
use lsp_types::{
    DidChangeTextDocumentParams, DidCloseTextDocumentParams, DidOpenTextDocumentParams, Position,
    Url,
};

#[derive(Debug)]
pub struct OpenFileManager {
    pub open_files: HashMap<Url, String>,
}
impl OpenFileManager {
    pub fn new() -> Self {
        Self {
            open_files: Default::default(),
        }
    }
    pub async fn did_open(&mut self, params: DidOpenTextDocumentParams) {
        match params.text_document.language_id.as_str() {
            "candy" => {
                let current_value = self
                    .open_files
                    .insert(params.text_document.uri, params.text_document.text);
                assert!(current_value.is_none());
            }
            _ => return,
        }
    }

    pub async fn did_change(&mut self, params: DidChangeTextDocumentParams) {
        let DidChangeTextDocumentParams {
            content_changes,
            text_document,
        } = params;
        self.open_files
            .entry(text_document.uri)
            .and_modify(move |text| {
                log::info!("received {} changes", content_changes.len());
                for change in content_changes {
                    match change.range {
                        Some(range) => {
                            let start = Self::position_to_utf8_byte_offset(text, &range.start);
                            let end = Self::position_to_utf8_byte_offset(text, &range.end);
                            *text = format!("{}{}{}", &text[..start], &change.text, &text[end..]);
                        }
                        None => *text = change.text,
                    }
                }
                log::info!("New text: {:?}", text);
            })
            .or_insert_with(|| panic!("Received a change for a file that was not opened."));
    }

    fn position_to_utf8_byte_offset(text: &str, position: &Position) -> usize {
        let mut line_index = 0;
        let mut line_offset = 0;
        while line_index < position.line {
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
        let char_offset = if position.character as usize >= words.len() {
            line_length_bytes
        } else {
            String::from_utf16(&words[0..position.character as usize])
                .unwrap()
                .len()
        };

        line_offset + char_offset
    }

    pub async fn did_close(&mut self, params: DidCloseTextDocumentParams) {
        self.open_files
            .remove(&params.text_document.uri)
            .expect("File was closed without being opened.");
    }
}
