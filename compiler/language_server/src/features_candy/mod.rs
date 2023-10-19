use self::{
    find_definition::find_definition,
    folding_ranges::folding_ranges,
    references::{reference_query_for_offset, references, ReferenceQuery},
    semantic_tokens::semantic_tokens,
};
use crate::{
    database::Database,
    features::{LanguageFeatures, Reference, RenameError},
    server::AnalyzerClient,
    utils::{lsp_range_to_range_raw, module_from_url, LspPositionConversion},
};
use async_trait::async_trait;
use candy_formatter::Formatter;
use candy_frontend::{
    module::{Module, ModuleDb, ModuleKind, MutableModuleProviderOwner, PackagesPath},
    rcst_to_cst::RcstToCst,
};
use lsp_types::{
    self, notification::Notification, FoldingRange, LocationLink, SemanticToken,
    TextDocumentContentChangeEvent, TextEdit, Url,
};
use regex::Regex;
use rustc_hash::FxHashMap;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, thread};
use tokio::sync::{mpsc::Sender, Mutex};

pub mod analyzer;
pub mod find_definition;
pub mod folding_ranges;
pub mod references;
pub mod semantic_tokens;

#[derive(Serialize, Deserialize)]
pub struct ServerStatusNotification {
    pub text: String,
}
impl Notification for ServerStatusNotification {
    const METHOD: &'static str = "candy/publishServerStatus";

    type Params = Self;
}

#[derive(Debug)]
pub struct CandyFeatures {
    hints_events_sender: Sender<analyzer::Message>,
}
impl CandyFeatures {
    #[must_use]
    pub fn new(packages_path: PackagesPath, client: AnalyzerClient) -> Self {
        let (hints_events_sender, hints_events_receiver) = tokio::sync::mpsc::channel(1024);
        thread::spawn(move || {
            analyzer::run_server(packages_path, hints_events_receiver, client);
        });
        Self {
            hints_events_sender,
        }
    }

    async fn send_to_analyzer(&self, event: analyzer::Message) {
        match self.hints_events_sender.send(event).await {
            Ok(_) => {}
            Err(error) => panic!("Couldn't send message to hints server: {error:?}."),
        }
    }
}

#[async_trait]
impl LanguageFeatures for CandyFeatures {
    fn language_id(&self) -> Option<String> {
        Some("candy".to_string())
    }
    fn supported_url_schemes(&self) -> Vec<&'static str> {
        vec!["file", "untitled"]
    }

    async fn initialize(&self) {}
    async fn shutdown(&self) {
        self.send_to_analyzer(analyzer::Message::Shutdown).await;
    }

    fn supports_did_open(&self) -> bool {
        true
    }
    async fn did_open(&self, db: &Mutex<Database>, uri: Url, content: Vec<u8>) {
        let module = {
            let mut db = db.lock().await;
            let module = decode_module(&uri, &db.packages_path);
            db.did_open_module(&module, content.clone());
            module
        };
        self.send_to_analyzer(analyzer::Message::UpdateModule(module, content))
            .await;
    }
    fn supports_did_change(&self) -> bool {
        true
    }
    async fn did_change(
        &self,
        db: &Mutex<Database>,
        uri: Url,
        changes: Vec<TextDocumentContentChangeEvent>,
    ) {
        let (module, content) = {
            let mut db = db.lock().await;
            let module = decode_module(&uri, &db.packages_path);
            let content = apply_text_changes(&db, module.clone(), changes).into_bytes();
            db.did_change_module(&module, content.clone());
            (module, content)
        };
        self.send_to_analyzer(analyzer::Message::UpdateModule(module, content))
            .await;
    }
    fn supports_did_close(&self) -> bool {
        true
    }
    async fn did_close(&self, db: &Mutex<Database>, uri: Url) {
        let module = {
            let mut db = db.lock().await;
            let module = decode_module(&uri, &db.packages_path);
            db.did_close_module(&module);
            module
        };
        self.send_to_analyzer(analyzer::Message::CloseModule(module))
            .await;
    }

    fn supports_folding_ranges(&self) -> bool {
        true
    }
    async fn folding_ranges(&self, db: &Mutex<Database>, uri: Url) -> Vec<FoldingRange> {
        let db = db.lock().await;
        let module = decode_module(&uri, &db.packages_path);
        folding_ranges(&*db, module)
    }

    fn supports_format(&self) -> bool {
        true
    }
    async fn format(&self, db: &Mutex<Database>, uri: Url) -> Vec<TextEdit> {
        let db = db.lock().await;
        let module = decode_module(&uri, &db.packages_path);
        let Ok(cst) = db.cst(module.clone()) else {
            return vec![];
        };

        cst.format_to_edits()
            .finish()
            .into_iter()
            .map(|it| TextEdit {
                range: db.range_to_lsp_range(module.clone(), it.range),
                new_text: it.new_text,
            })
            .collect()
    }

    fn supports_find_definition(&self) -> bool {
        true
    }
    async fn find_definition(
        &self,
        db: &Mutex<Database>,
        uri: Url,
        position: lsp_types::Position,
    ) -> Option<LocationLink> {
        let db = db.lock().await;
        let module = decode_module(&uri, &db.packages_path);
        let offset = db.lsp_position_to_offset(module.clone(), position);
        find_definition(&db, module, offset)
    }

    fn supports_references(&self) -> bool {
        true
    }
    async fn references(
        &self,
        db: &Mutex<Database>,
        uri: Url,
        position: lsp_types::Position,
        _only_in_same_document: bool,
        include_declaration: bool,
    ) -> FxHashMap<Url, Vec<Reference>> {
        let db = db.lock().await;
        let module = decode_module(&uri, &db.packages_path);
        let offset = db.lsp_position_to_offset(module.clone(), position);

        let mut all_references = FxHashMap::default();
        let references = references(&*db, module, offset, include_declaration);
        // TODO: Look for references in all modules
        if !references.is_empty() {
            all_references.insert(uri, references);
        }
        all_references
    }

    fn supports_rename(&self) -> bool {
        true
    }
    async fn prepare_rename(
        &self,
        db: &Mutex<Database>,
        uri: Url,
        position: lsp_types::Position,
    ) -> Option<lsp_types::Range> {
        let db = db.lock().await;
        let module = decode_module(&uri, &db.packages_path);
        let offset = db.lsp_position_to_offset(module.clone(), position);

        match reference_query_for_offset(&*db, module.clone(), offset) {
            Some((ReferenceQuery::Id(_), range)) => Some(db.range_to_lsp_range(module, range)),
            Some((
                ReferenceQuery::Symbol(_, _) | ReferenceQuery::Int(_, _) | ReferenceQuery::Needs(_),
                _,
            ))
            | None => None,
        }
    }
    async fn rename(
        &self,
        db: &Mutex<Database>,
        uri: Url,
        position: lsp_types::Position,
        new_name: String,
    ) -> Result<HashMap<Url, Vec<TextEdit>>, RenameError> {
        {
            let db = db.lock().await;
            let module = decode_module(&uri, &db.packages_path);
            let offset = db.lsp_position_to_offset(module.clone(), position);

            let regex =
                match reference_query_for_offset(&*db, module, offset).map(|(query, _)| query) {
                    Some(ReferenceQuery::Id(_)) => Regex::new(r"^[a-z][A-Za-z0-9_]*$").unwrap(),
                    Some(
                        ReferenceQuery::Symbol(_, _)
                        | ReferenceQuery::Int(_, _)
                        | ReferenceQuery::Needs(_),
                    )
                    | None => {
                        panic!("Renaming is not supported at this position.")
                    }
                };
            if !regex.is_match(&new_name) {
                return Err(RenameError::NewNameInvalid);
            }
        }

        let references = self.references(db, uri, position, false, true).await;
        assert!(!references.is_empty());
        let changes = references
            .into_iter()
            .map(|(url, references)| {
                let changes = references
                    .into_iter()
                    .map(|it| TextEdit {
                        range: it.range,
                        new_text: new_name.clone(),
                    })
                    .collect();
                (url, changes)
            })
            .collect();
        Ok(changes)
    }

    fn supports_semantic_tokens(&self) -> bool {
        true
    }
    async fn semantic_tokens(&self, db: &Mutex<Database>, uri: Url) -> Vec<SemanticToken> {
        let db = db.lock().await;
        let module = decode_module(&uri, &db.packages_path);
        semantic_tokens(&*db, module)
    }
}

fn decode_module(uri: &Url, packages_path: &PackagesPath) -> Module {
    module_from_url(uri, ModuleKind::Code, packages_path).unwrap()
}
fn apply_text_changes(
    db: &Database,
    module: Module,
    changes: Vec<TextDocumentContentChangeEvent>,
) -> String {
    let mut text = db
        .get_module_content_as_string(module)
        .unwrap()
        .as_ref()
        .clone();
    for change in changes {
        match change.range {
            Some(range) => {
                let range = lsp_range_to_range_raw(&text, range);
                text = format!(
                    "{}{}{}",
                    &text[..*range.start],
                    &change.text,
                    &text[*range.end..],
                );
            }
            None => text = change.text,
        }
    }
    text
}
