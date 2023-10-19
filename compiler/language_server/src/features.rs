use crate::database::Database;
use async_trait::async_trait;
use lsp_types::{
    self, FoldingRange, LocationLink, SemanticToken, TextDocumentContentChangeEvent, TextEdit, Url,
};
use rustc_hash::FxHashMap;
use std::collections::HashMap;
use tokio::sync::Mutex;

#[async_trait]
#[allow(clippy::diverging_sub_expression)]
pub trait LanguageFeatures: Send + Sync {
    #[must_use]
    fn language_id(&self) -> Option<String>;
    #[must_use]
    fn supported_url_schemes(&self) -> Vec<&'static str>;

    async fn initialize(&self) {}
    async fn shutdown(&self) {}

    fn supports_did_open(&self) -> bool {
        false
    }
    async fn did_open(&self, _db: &Mutex<Database>, _uri: Url, _content: Vec<u8>) {
        unimplemented!()
    }
    fn supports_did_change(&self) -> bool {
        false
    }
    async fn did_change(
        &self,
        _db: &Mutex<Database>,
        _uri: Url,
        _changes: Vec<TextDocumentContentChangeEvent>,
    ) {
        unimplemented!()
    }
    fn supports_did_close(&self) -> bool {
        false
    }
    async fn did_close(&self, _db: &Mutex<Database>, _uri: Url) {
        unimplemented!()
    }

    fn supports_folding_ranges(&self) -> bool {
        false
    }
    #[must_use]
    async fn folding_ranges(&self, _db: &Mutex<Database>, _uri: Url) -> Vec<FoldingRange> {
        unimplemented!()
    }

    fn supports_format(&self) -> bool {
        false
    }
    #[must_use]
    async fn format(&self, _db: &Mutex<Database>, _uri: Url) -> Vec<TextEdit> {
        unimplemented!()
    }

    fn supports_find_definition(&self) -> bool {
        false
    }
    #[must_use]
    async fn find_definition(
        &self,
        _db: &Mutex<Database>,
        _uri: Url,
        _position: lsp_types::Position,
    ) -> Option<LocationLink> {
        unimplemented!()
    }

    fn supports_references(&self) -> bool {
        false
    }
    /// Used for highlighting and finding references.
    #[must_use]
    async fn references(
        &self,
        _db: &Mutex<Database>,
        _uri: Url,
        _position: lsp_types::Position,
        _only_in_same_document: bool,
        _include_declaration: bool,
    ) -> FxHashMap<Url, Vec<Reference>> {
        unimplemented!()
    }

    fn supports_rename(&self) -> bool {
        false
    }
    #[must_use]
    async fn prepare_rename(
        &self,
        _db: &Mutex<Database>,
        _uri: Url,
        _position: lsp_types::Position,
    ) -> Option<lsp_types::Range> {
        unimplemented!()
    }
    #[must_use]
    async fn rename(
        &self,
        _db: &Mutex<Database>,
        _uri: Url,
        _position: lsp_types::Position,
        _new_name: String,
    ) -> Result<HashMap<Url, Vec<TextEdit>>, RenameError> {
        unimplemented!()
    }

    fn supports_semantic_tokens(&self) -> bool {
        false
    }
    #[must_use]
    async fn semantic_tokens(&self, _db: &Mutex<Database>, _uri: Url) -> Vec<SemanticToken> {
        unimplemented!()
    }
}

pub struct Reference {
    pub range: lsp_types::Range,
    pub is_write: bool,
}

pub enum RenameError {
    NewNameInvalid,
}
