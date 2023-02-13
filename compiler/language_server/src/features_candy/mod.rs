use std::{future::Future, pin::Pin, thread};

use async_trait::async_trait;
use candy_frontend::{
    ast_to_hir::AstToHir,
    hir::CollectErrors,
    module::{Module, ModuleDb, MutableModuleProviderOwner},
    position::Offset,
};
use itertools::Itertools;
use lsp_types::{
    Diagnostic, DocumentHighlight, FoldingRange, LocationLink, SemanticToken,
    TextDocumentContentChangeEvent,
};
use tokio::sync::{mpsc::Sender, Mutex};
use tracing::debug;

use crate::{
    database::Database,
    features::LanguageFeatures,
    utils::{error_into_diagnostic, lsp_range_to_range_raw},
};

use self::{
    find_definition::find_definition, folding_ranges::folding_ranges, hints::Hint,
    references::references, semantic_tokens::semantic_tokens,
};

pub mod find_definition;
pub mod folding_ranges;
pub mod hints;
pub mod references;
pub mod semantic_tokens;

pub struct CandyFeatures {
    diagnostics_sender: Sender<(Module, Vec<Diagnostic>)>,
    hints_events_sender: Sender<hints::Event>,
}
impl CandyFeatures {
    pub fn new(
        diagnostics_sender: Sender<(Module, Vec<Diagnostic>)>,
        hints_sender: Sender<(Module, Vec<Hint>)>,
    ) -> Self {
        let (hints_events_sender, hints_events_receiver) = tokio::sync::mpsc::channel(1024);
        // let (hints_sender, mut hints_receiver) = tokio::sync::mpsc::channel(1024);
        thread::spawn(|| {
            hints::run_server(hints_events_receiver, hints_sender);
        });
        Self {
            diagnostics_sender,
            hints_events_sender,
        }
    }

    async fn analyze_modules<M: AsRef<[Module]>>(&self, db: &Mutex<Database>, modules: M) {
        let modules = modules.as_ref();
        debug!(
            "Analyzing {} {}",
            if modules.len() == 1 {
                "module"
            } else {
                "modules"
            },
            modules.iter().join(", "),
        );

        for module in modules {
            let diagnostics = {
                let db = db.lock().await;
                let (hir, _mapping) = db.hir(module.clone()).unwrap();

                let mut errors = vec![];
                hir.collect_errors(&mut errors);
                errors
                    .into_iter()
                    .map(|it| error_into_diagnostic(&*db, module.clone(), it))
                    .collect()
            };
            self.diagnostics_sender
                .send((module.to_owned(), diagnostics))
                .await
                .expect("Diagnostics channel closed");
        }
    }

    async fn send_to_hints_server(&self, event: hints::Event) {
        match self.hints_events_sender.send(event).await {
            Ok(_) => {}
            Err(_) => panic!("Couldn't send message to hints server."),
        }
    }
}

#[async_trait]
impl LanguageFeatures for CandyFeatures {
    fn language_id(&self) -> Option<String> {
        Some("candy".to_string())
    }
    fn supported_url_schemes(&self) -> Vec<String> {
        vec!["file".to_string(), "untitled".to_string()]
    }

    async fn initialize(&self) {}
    async fn shutdown(&self) {
        self.send_to_hints_server(hints::Event::Shutdown).await;
    }

    fn supports_did_open(&self) -> bool {
        true
    }
    async fn did_open(&self, db: &Mutex<Database>, module: Module, content: Vec<u8>) {
        {
            let mut db = db.lock().await;
            db.did_open_module(&module, content.clone());
        }
        self.analyze_modules(db, [module.clone()]).await;
        self.send_to_hints_server(hints::Event::UpdateModule(module, content))
            .await;
    }
    fn supports_did_change(&self) -> bool {
        true
    }
    async fn did_change(
        &self,
        db: &Mutex<Database>,
        module: Module,
        changes: Vec<TextDocumentContentChangeEvent>,
    ) {
        let (content, open_modules) = {
            let mut db = db.lock().await;
            let content = apply_text_changes(&db, module.clone(), changes).into_bytes();
            db.did_change_module(&module, content.clone());
            (content, db.get_open_modules())
        };
        self.analyze_modules(db, open_modules).await;
        self.send_to_hints_server(hints::Event::UpdateModule(module, content))
            .await;
    }
    fn supports_did_close(&self) -> bool {
        true
    }
    async fn did_close(&self, db: &Mutex<Database>, module: Module) {
        {
            let mut db = db.lock().await;
            db.did_close_module(&module);
        }
        self.send_to_hints_server(hints::Event::CloseModule(module))
            .await;
    }

    fn supports_folding_ranges(&self) -> bool {
        true
    }
    fn folding_ranges(&self, db: &Database, module: Module) -> Vec<FoldingRange> {
        folding_ranges(db, module)
    }

    fn supports_find_definition(&self) -> bool {
        true
    }
    fn find_definition(
        &self,
        db: &Database,
        module: Module,
        offset: Offset,
    ) -> Option<LocationLink> {
        find_definition(db, module, offset)
    }

    fn supports_references(&self) -> bool {
        true
    }
    fn references(
        &self,
        db: &Database,
        module: Module,
        offset: Offset,
        include_declaration: bool,
    ) -> Option<Vec<DocumentHighlight>> {
        references(db, module, offset, include_declaration)
    }

    fn supports_semantic_tokens(&self) -> bool {
        true
    }
    fn semantic_tokens<'life0, 'life1, 'async_trait>(
        &'life0 self,
        db: &'life1 Database,
        module: Module,
    ) -> Pin<Box<dyn Future<Output = Vec<SemanticToken>> + Send + 'async_trait>>
    where
        'life0: 'async_trait,
        'life1: 'async_trait,
        Self: 'async_trait,
    {
        // [Database] is not [Send], so we can't use it in the async block.
        // Since we don't need it after any async break, we avoid the `async`
        // keyword for this function and use the database synchronously.
        let tokens = semantic_tokens(db, module);
        Box::pin(async move { tokens })
    }
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
        .to_owned();
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
