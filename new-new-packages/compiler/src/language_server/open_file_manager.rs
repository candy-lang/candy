use im::HashMap;
use lsp_types::{
    DidChangeTextDocumentParams, DidCloseTextDocumentParams, DidOpenTextDocumentParams, Url,
};

use crate::language_server::utils::RangeToUtf8ByteOffset;

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

    pub fn get(&self, uri: &Url) -> Option<&str> {
        self.open_files.get(uri).map(|it| it.as_str())
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
                            let range = range.to_utf8_byte_offset(text);
                            *text = format!(
                                "{}{}{}",
                                &text[..range.start],
                                &change.text,
                                &text[range.end..]
                            );
                        }
                        None => *text = change.text,
                    }
                }
                log::info!("New text: {:?}", text);
            })
            .or_insert_with(|| panic!("Received a change for a file that was not opened."));
    }

    pub async fn did_close(&mut self, params: DidCloseTextDocumentParams) {
        self.open_files
            .remove(&params.text_document.uri)
            .expect("File was closed without being opened.");
    }
}
