use std::path::Path;

use async_trait::async_trait;
use lsp_types::{
    self, DocumentHighlight, FoldingRange, LocationLink, SemanticToken,
    TextDocumentContentChangeEvent, Url,
};
use tokio::sync::Mutex;

use crate::database::Database;

#[async_trait]
pub trait LanguageFeatures: Send + Sync {
    fn language_id(&self) -> Option<String>;
    fn supported_url_schemes(&self) -> Vec<&'static str>;

    async fn initialize(&self) {}
    async fn shutdown(&self) {}

    fn supports_did_open(&self) -> bool {
        false
    }
    async fn did_open(
        &self,
        _db: &Mutex<Database>,
        _project_directory: &Path,
        _uri: Url,
        _content: Vec<u8>,
    ) {
        unimplemented!()
    }
    fn supports_did_change(&self) -> bool {
        false
    }
    async fn did_change(
        &self,
        _db: &Mutex<Database>,

        _project_directory: &Path,
        _uri: Url,
        _changes: Vec<TextDocumentContentChangeEvent>,
    ) {
        unimplemented!()
    }
    fn supports_did_close(&self) -> bool {
        false
    }
    async fn did_close(&self, _db: &Mutex<Database>, _project_directory: &Path, _uri: Url) {
        unimplemented!()
    }

    fn supports_folding_ranges(&self) -> bool {
        false
    }
    async fn folding_ranges(
        &self,
        _db: &Mutex<Database>,
        _project_directory: &Path,
        _uri: Url,
    ) -> Vec<FoldingRange> {
        unimplemented!()
    }

    fn supports_find_definition(&self) -> bool {
        false
    }
    async fn find_definition(
        &self,
        _db: &Mutex<Database>,
        _project_directory: &Path,
        _uri: Url,
        _position: lsp_types::Position,
    ) -> Option<LocationLink> {
        unimplemented!()
    }

    fn supports_references(&self) -> bool {
        false
    }
    /// Used for highlighting and finding references.
    async fn references(
        &self,
        _db: &Mutex<Database>,
        _project_directory: &Path,
        _uri: Url,
        _position: lsp_types::Position,
        _include_declaration: bool,
    ) -> Option<Vec<DocumentHighlight>> {
        unimplemented!()
    }

    fn supports_semantic_tokens(&self) -> bool {
        false
    }
    async fn semantic_tokens(
        &self,
        _db: &Mutex<Database>,
        _project_directory: &Path,
        _uri: Url,
    ) -> Vec<SemanticToken> {
        unimplemented!()
    }
}
