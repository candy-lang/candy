pub mod definition;
pub mod folding_range;
pub mod hints;
pub mod references;
pub mod semantic_tokens;
pub mod utils;

use self::{
    definition::find_definition,
    folding_range::FoldingRangeDb,
    hints::HintsNotification,
    references::{find_document_highlights, find_references},
    semantic_tokens::SemanticTokenDb,
    utils::{line_start_utf8_byte_offsets_raw, offset_from_lsp_raw},
};
use crate::{
    compiler::{ast_to_hir::AstToHir, hir::CollectErrors},
    module::{Module, ModuleDb},
    Database,
};
use itertools::Itertools;
use lsp_types::{
    DidChangeTextDocumentParams, DidCloseTextDocumentParams, DidOpenTextDocumentParams,
    DocumentFilter, DocumentHighlight, DocumentHighlightParams, FoldingRange, FoldingRangeParams,
    GotoDefinitionParams, GotoDefinitionResponse, InitializeParams, InitializeResult,
    InitializedParams, Location, MessageType, ReferenceParams, Registration, SemanticTokens,
    SemanticTokensFullOptions, SemanticTokensOptions, SemanticTokensParams,
    SemanticTokensRegistrationOptions, SemanticTokensResult, SemanticTokensServerCapabilities,
    ServerCapabilities, ServerInfo, StaticRegistrationOptions,
    TextDocumentChangeRegistrationOptions, TextDocumentContentChangeEvent,
    TextDocumentRegistrationOptions, Url, WorkDoneProgressOptions,
};
use std::{path::PathBuf, sync::Arc};
use tokio::sync::{mpsc::Sender, Mutex};
use tower_lsp::{jsonrpc, Client, LanguageServer};

pub struct CandyLanguageServer {
    pub client: Client,
    pub db: Mutex<Database>,
    pub hints_server_sink: Arc<Mutex<Option<Sender<hints::Event>>>>,
    project_directory: Option<PathBuf>,
}
impl CandyLanguageServer {
    pub fn from_client(client: Client) -> Self {
        Self {
            client,
            db: Default::default(),
            hints_server_sink: Default::default(),
            project_directory: None,
        }
    }

    async fn send_to_hints_server(&self, event: hints::Event) {
        let event_sink = self.hints_server_sink.lock().await;
        match event_sink.as_ref().unwrap().send(event).await {
            Ok(_) => {}
            Err(_) => panic!("Couldn't send message to hints server."),
        }
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for CandyLanguageServer {
    async fn initialize(&self, params: InitializeParams) -> jsonrpc::Result<InitializeResult> {
        log::info!("LSP: initialize");
        self.client
            .log_message(MessageType::INFO, "Initializing!")
            .await;

        let first_workspace_folder = params
            .workspace_folders
            .unwrap()
            .first()
            .unwrap()
            .uri
            .clone();
        self.project_directory = match first_workspace_folder.scheme() {
            "file" => Some(first_workspace_folder.to_file_path().unwrap()),
            _ => panic!("Workspace folder must be a file URI."),
        };

        let (events_sender, events_receiver) = tokio::sync::mpsc::channel(16);
        let (hints_sender, mut hints_receiver) = tokio::sync::mpsc::channel(8);
        tokio::spawn(hints::run_server(events_receiver, hints_sender));
        *self.hints_server_sink.lock().await = Some(events_sender);
        let client = self.client.clone();
        let hint_reporter = async move || {
            while let Some((module, hints)) = hints_receiver.recv().await {
                log::debug!("Reporting hints for {module}: {hints:?}");
                client
                    .send_notification::<HintsNotification>(HintsNotification {
                        uri: Url::from(module).to_string(),
                        hints,
                    })
                    .await;
            }
        };
        tokio::spawn(hint_reporter());

        Ok(InitializeResult {
            // We only support dynamic registration for now.
            capabilities: ServerCapabilities::default(),
            server_info: Some(ServerInfo {
                name: "🍭 Candy Language Server".to_owned(),
                version: None,
            }),
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        log::info!("LSP: initialized");
        let candy_files = vec![
            DocumentFilter {
                language: Some("candy".to_owned()),
                scheme: Some("file".to_owned()),
                pattern: None,
            },
            DocumentFilter {
                language: Some("candy".to_owned()),
                scheme: Some("untitled".to_owned()),
                pattern: None,
            },
        ];
        let text_document_registration_options = TextDocumentRegistrationOptions {
            document_selector: Some(candy_files.clone()),
        };
        self.client
            .register_capability(vec![
                Registration {
                    id: "0".to_owned(),
                    method: "textDocument/didOpen".to_owned(),
                    register_options: Some(
                        serde_json::to_value(text_document_registration_options.clone()).unwrap(),
                    ),
                },
                Registration {
                    id: "1".to_owned(),
                    method: "textDocument/didOpen".to_owned(),
                    register_options: Some(
                        serde_json::to_value(text_document_registration_options.clone()).unwrap(),
                    ),
                },
                Registration {
                    id: "2".to_owned(),
                    method: "textDocument/didChange".to_owned(),
                    register_options: Some(
                        serde_json::to_value(TextDocumentChangeRegistrationOptions {
                            document_selector: Some(candy_files),
                            sync_kind: 2, // incremental
                        })
                        .unwrap(),
                    ),
                },
                Registration {
                    id: "3".to_owned(),
                    method: "textDocument/definition".to_owned(),
                    register_options: Some(
                        serde_json::to_value(text_document_registration_options.clone()).unwrap(),
                    ),
                },
                Registration {
                    id: "4".to_owned(),
                    method: "textDocument/references".to_owned(),
                    register_options: Some(
                        serde_json::to_value(text_document_registration_options.clone()).unwrap(),
                    ),
                },
                Registration {
                    id: "5".to_owned(),
                    method: "textDocument/documentHighlight".to_owned(),
                    register_options: Some(
                        serde_json::to_value(text_document_registration_options.clone()).unwrap(),
                    ),
                },
                Registration {
                    id: "6".to_owned(),
                    method: "textDocument/foldingRange".to_owned(),
                    register_options: Some(
                        serde_json::to_value(text_document_registration_options.clone()).unwrap(),
                    ),
                },
                Registration {
                    id: "7".to_owned(),
                    method: "textDocument/semanticTokens".to_owned(),
                    register_options: Some(
                        serde_json::to_value(
                            SemanticTokensServerCapabilities::SemanticTokensRegistrationOptions(
                                SemanticTokensRegistrationOptions {
                                    text_document_registration_options,
                                    semantic_tokens_options: SemanticTokensOptions {
                                        work_done_progress_options: WorkDoneProgressOptions {
                                            work_done_progress: None,
                                        },
                                        legend: semantic_tokens::LEGEND.clone(),
                                        range: Some(false),
                                        full: Some(SemanticTokensFullOptions::Bool(true)),
                                    },
                                    static_registration_options: StaticRegistrationOptions {
                                        id: None,
                                    },
                                },
                            ),
                        )
                        .unwrap(),
                    ),
                },
            ])
            .await
            .expect("Dynamic capability registration failed.");
        self.client
            .log_message(MessageType::INFO, "server initialized!")
            .await;
    }

    async fn shutdown(&self) -> jsonrpc::Result<()> {
        self.send_to_hints_server(hints::Event::Shutdown).await;
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let module = Module::from_package_root_and_file(
            self.project_directory,
            params.text_document.uri.into(),
        )
        .unwrap();
        let content = params.text_document.text.into_bytes();
        {
            let mut db = self.db.lock().await;
            db.did_open_module(&module, content.clone());
        }
        self.analyze_modules(vec![module.clone()]).await;
        self.send_to_hints_server(hints::Event::UpdateModule(module, content))
            .await;
    }
    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let module: Module = params.text_document.uri.into();
        let mut open_modules = Vec::<Module>::new();
        let content = {
            let mut db = self.db.lock().await;
            let text = apply_text_changes(&db, module.clone(), params.content_changes);
            db.did_change_module(&module, text.clone().into_bytes());
            open_modules.extend(db.open_modules.keys().cloned());
            text.into_bytes()
        };
        self.analyze_modules(open_modules).await;
        self.send_to_hints_server(hints::Event::UpdateModule(module, content))
            .await;
    }
    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        let module = params.text_document.uri.into();
        let mut db = self.db.lock().await;
        db.did_close_module(&module);
        self.send_to_hints_server(hints::Event::CloseModule(module))
            .await;
    }

    async fn goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> jsonrpc::Result<Option<GotoDefinitionResponse>> {
        let db = self.db.lock().await;
        Ok(find_definition(&db, params))
    }

    async fn references(&self, params: ReferenceParams) -> jsonrpc::Result<Option<Vec<Location>>> {
        let db = self.db.lock().await;
        Ok(find_references(&db, params))
    }
    async fn document_highlight(
        &self,
        params: DocumentHighlightParams,
    ) -> jsonrpc::Result<Option<Vec<DocumentHighlight>>> {
        let db = self.db.lock().await;
        Ok(find_document_highlights(&db, params))
    }

    async fn folding_range(
        &self,
        params: FoldingRangeParams,
    ) -> jsonrpc::Result<Option<Vec<FoldingRange>>> {
        let db = self.db.lock().await;
        let ranges = db.folding_ranges(params.text_document.uri.into());
        Ok(Some(ranges))
    }

    async fn semantic_tokens_full(
        &self,
        params: SemanticTokensParams,
    ) -> jsonrpc::Result<Option<SemanticTokensResult>> {
        let db = self.db.lock().await;
        let tokens = db.semantic_tokens(params.text_document.uri.into());
        Ok(Some(SemanticTokensResult::Tokens(SemanticTokens {
            result_id: None,
            data: tokens,
        })))
    }
}

impl CandyLanguageServer {
    async fn analyze_modules(&self, modules: Vec<Module>) {
        log::debug!(
            "Analyzing {} {}",
            if modules.len() == 1 {
                "module"
            } else {
                "modules"
            },
            modules.iter().join(", ")
        );
        let db = self.db.lock().await;
        log::debug!("Locked.");

        for module in modules {
            let (hir, _mapping) = db.hir(module.clone()).unwrap();

            let diagnostics = {
                let mut errors = vec![];
                hir.collect_errors(&mut errors);
                errors
                    .into_iter()
                    .map(|it| it.to_diagnostic(&db, module.clone()))
                    .collect()
            };
            self.client
                .publish_diagnostics(module.clone().into(), diagnostics, None)
                .await;
        }
    }
}

fn apply_text_changes(
    db: &Database,
    module: Module,
    changes: Vec<TextDocumentContentChangeEvent>,
) -> String {
    let mut text = db
        .get_module_content_as_string(module.clone())
        .unwrap()
        .as_ref()
        .to_owned();
    for change in changes {
        match change.range {
            Some(range) => {
                let line_start_offsets = line_start_utf8_byte_offsets_raw(&text);
                let start = offset_from_lsp_raw(&text, &line_start_offsets[..], range.start);
                let end = offset_from_lsp_raw(&text, &line_start_offsets[..], range.end);
                text = format!("{}{}{}", &text[..start], &change.text, &text[end..]);
            }
            None => text = change.text,
        }
    }
    text
}
