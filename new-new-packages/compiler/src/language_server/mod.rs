use lsp_types::{
    DidChangeTextDocumentParams, DidCloseTextDocumentParams, DidOpenTextDocumentParams,
    DocumentFilter, FoldingRange, FoldingRangeParams, InitializeParams, InitializeResult,
    InitializedParams, MessageType, Registration, SemanticTokens, SemanticTokensFullOptions,
    SemanticTokensOptions, SemanticTokensParams, SemanticTokensRegistrationOptions,
    SemanticTokensResult, SemanticTokensServerCapabilities, ServerCapabilities, ServerInfo,
    StaticRegistrationOptions, TextDocumentChangeRegistrationOptions,
    TextDocumentContentChangeEvent, TextDocumentRegistrationOptions, WorkDoneProgressOptions,
};
use lspower::{jsonrpc, Client, LanguageServer};
use tokio::sync::Mutex;

use crate::{
    compiler::{ast_to_hir::AstToHir, cst_to_ast::CstToAst, string_to_cst::StringToCst},
    input::{Input, InputReference},
    Database,
};

use self::{
    folding_range::FoldingRangeDb,
    hints::{HintsDb, HintsNotification},
    semantic_tokens::SemanticTokenDb,
    utils::LspPositionConversion,
};

pub mod folding_range;
pub mod hints;
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
    async fn initialize(&self, _: InitializeParams) -> jsonrpc::Result<InitializeResult> {
        log::info!("LSP: initialize");
        self.client
            .log_message(MessageType::INFO, "Initializing!")
            .await;
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
        let candy_files = DocumentFilter {
            language: Some("candy".to_owned()),
            scheme: Some("file".to_owned()),
            pattern: None,
        };
        let text_document_registration_options = TextDocumentRegistrationOptions {
            document_selector: Some(vec![candy_files.clone()]),
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
                            document_selector: Some(vec![candy_files]),
                            sync_kind: 2, // incremental
                        })
                        .unwrap(),
                    ),
                },
                Registration {
                    id: "3".to_owned(),
                    method: "textDocument/foldingRange".to_owned(),
                    register_options: Some(
                        serde_json::to_value(text_document_registration_options.clone()).unwrap(),
                    ),
                },
                Registration {
                    id: "4".to_owned(),
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
        let input_reference = params.text_document.uri.into();
        self.db
            .lock()
            .await
            .did_open_input(&input_reference, params.text_document.text);
        self.analyze_file(input_reference).await;
    }
    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let input_reference: InputReference = params.text_document.uri.into();
        let mut db = self.db.lock().await;
        let text = apply_text_changes(&db, input_reference.clone(), params.content_changes);
        db.did_change_input(&input_reference, text);
        self.analyze_file(input_reference).await;
    }
    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        let input_reference = params.text_document.uri.into();
        self.db.lock().await.did_close_input(&input_reference);
    }

    async fn folding_range(
        &self,
        params: FoldingRangeParams,
    ) -> jsonrpc::Result<Option<Vec<FoldingRange>>> {
        let ranges = self
            .db
            .lock()
            .await
            .folding_ranges(params.text_document.uri.into());
        Ok(Some(ranges))
    }

    async fn semantic_tokens_full(
        &self,
        params: SemanticTokensParams,
    ) -> jsonrpc::Result<Option<SemanticTokensResult>> {
        let tokens = self
            .db
            .lock()
            .await
            .semantic_tokens(params.text_document.uri.into());
        Ok(Some(SemanticTokensResult::Tokens(SemanticTokens {
            result_id: None,
            data: tokens,
        })))
    }
}

impl CandyLanguageServer {
    async fn analyze_file(&self, input_reference: InputReference) {
        let db = self.db.lock().await;

        let (_, cst_errors) = db.cst_raw(input_reference.clone()).unwrap();
        let (_, _, ast_errors) = db.ast_raw(input_reference.clone()).unwrap();
        let (_, _, hir_errors) = db.hir_raw(input_reference.clone()).unwrap();

        let diagnostics = cst_errors
            .into_iter()
            .chain(ast_errors.into_iter())
            .chain(hir_errors.into_iter())
            .map(|it| it.to_diagnostic(&db, input_reference.clone()))
            .collect();
        self.client
            .publish_diagnostics(input_reference.clone().into(), diagnostics, None)
            .await;
        let hints = db.hints(input_reference.clone());
        self.client
            .send_custom_notification::<HintsNotification>(HintsNotification {
                uri: match input_reference {
                    InputReference::File(path) => {
                        path.into_os_string().to_string_lossy().to_string()
                    }
                    InputReference::Untitled(string) => string,
                },
                hints,
            })
            .await;
    }
}

fn apply_text_changes(
    db: &Database,
    input_reference: InputReference,
    changes: Vec<TextDocumentContentChangeEvent>,
) -> String {
    let mut text = db
        .get_input(input_reference.clone())
        .unwrap()
        .as_ref()
        .to_owned();
    for change in changes {
        match change.range {
            Some(range) => {
                let start = db.position_to_utf8_byte_offset(
                    range.start.line,
                    range.start.character,
                    input_reference.clone(),
                );
                let end = db.position_to_utf8_byte_offset(
                    range.end.line,
                    range.end.character,
                    input_reference.clone(),
                );
                text = format!("{}{}{}", &text[..start], &change.text, &text[end..]);
            }
            None => text = change.text,
        }
    }
    text
}
