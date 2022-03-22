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
use lspower::{jsonrpc, Client, LanguageServer};
use tokio::sync::Mutex;

use crate::{
    compiler::{ast_to_hir::AstToHir, cst_to_ast::CstToAst, string_to_cst::StringToCst},
    database::project_directory,
    input::{Input, InputDb},
    language_server::hints::HintsDb,
    Database,
};

use self::{
    definition::find_definition,
    folding_range::FoldingRangeDb,
    hints::HintsNotification,
    references::{find_document_highlights, find_references},
    semantic_tokens::SemanticTokenDb,
    utils::LspPositionConversion,
};

pub mod definition;
pub mod folding_range;
pub mod hints;
pub mod references;
pub mod semantic_tokens;
pub mod utils;

pub struct CandyLanguageServer {
    pub client: Client,
    pub db: Mutex<Database>,
}
impl CandyLanguageServer {
    pub fn from_client(client: Client) -> Self {
        Self {
            client,
            db: Mutex::new(Database::default()),
        }
    }
}

#[lspower::async_trait]
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
        *project_directory.lock().unwrap() = match first_workspace_folder.into() {
            Input::File(path) => Some(path),
            _ => panic!("Workspace folder must be a file URI."),
        };

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
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let input = params.text_document.uri.into();
        {
            let mut db = self.db.lock().await;
            db.did_open_input(&input, params.text_document.text);
        }
        self.analyze_file(input).await;
    }
    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let input: Input = params.text_document.uri.into();
        {
            let mut db = self.db.lock().await;
            let text = apply_text_changes(&db, input.clone(), params.content_changes);
            db.did_change_input(&input, text);
        }
        self.analyze_file(input).await;
    }
    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        let input = params.text_document.uri.into();
        let mut db = self.db.lock().await;
        db.did_close_input(&input);
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
    async fn analyze_file(&self, input: Input) {
        log::debug!("Analyzing file. Locking guard.");
        let db = self.db.lock().await;
        log::debug!("Locked.");

        let (_, cst_errors) = db.cst_raw(input.clone()).unwrap();
        let (_, _, ast_errors) = db.ast_raw(input.clone()).unwrap();
        let (_, _, hir_errors) = db.hir_raw(input.clone()).unwrap();

        log::debug!("Mapping errors to diagnostics.");
        let diagnostics = cst_errors
            .into_iter()
            .chain(ast_errors.into_iter())
            .chain(hir_errors.into_iter())
            .map(|it| it.to_diagnostic(&db, input.clone()))
            .collect();
        log::debug!("Publishing diagnostics.");
        self.client
            .publish_diagnostics(input.clone().into(), diagnostics, None)
            .await;
        log::debug!("Published.");
        let hints = db.hints(input.clone());
        self.client
            .send_custom_notification::<HintsNotification>(HintsNotification {
                uri: Url::from(input).to_string(),
                hints,
            })
            .await;
    }
}

fn apply_text_changes(
    db: &Database,
    input: Input,
    changes: Vec<TextDocumentContentChangeEvent>,
) -> String {
    let mut text = db.get_input(input.clone()).unwrap().as_ref().to_owned();
    for change in changes {
        match change.range {
            Some(range) => {
                let start =
                    db.offset_from_lsp(input.clone(), range.start.line, range.start.character);
                let end = db.offset_from_lsp(input.clone(), range.end.line, range.end.character);
                text = format!("{}{}{}", &text[..start], &change.text, &text[end..]);
            }
            None => text = change.text,
        }
    }
    text
}
