use lsp_types::{
    Diagnostic, DiagnosticSeverity, DidChangeTextDocumentParams, DidCloseTextDocumentParams,
    DidOpenTextDocumentParams, DocumentFilter, FoldingRange, FoldingRangeParams, InitializeParams,
    InitializeResult, InitializedParams, MessageType, Registration, ServerCapabilities, ServerInfo,
    TextDocumentChangeRegistrationOptions, TextDocumentRegistrationOptions, Url,
};
use lspower::{jsonrpc, Client, LanguageServer};
use tokio::{fs, sync::Mutex};

use crate::compiler::{
    ast_to_hir::CompileVecAstsToHir, cst_to_ast::LowerCstToAst, string_to_cst::StringToCst,
};

use self::{
    folding_range::compute_folding_ranges, open_file_manager::OpenFileManager, utils::RangeToLsp,
};

mod folding_range;
mod open_file_manager;
mod utils;

#[derive(Debug)]
pub struct CandyLanguageServer {
    pub client: Client,
    pub open_file_manager: Mutex<OpenFileManager>,
}
impl CandyLanguageServer {
    pub fn from_client(client: Client) -> Self {
        Self {
            client,
            open_file_manager: Mutex::new(OpenFileManager::new()),
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
                        serde_json::to_value(text_document_registration_options).unwrap(),
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
        self.open_file_manager
            .lock()
            .await
            .did_open(params.clone())
            .await;
        self.analyze_file(params.text_document.uri).await;
    }
    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        self.open_file_manager
            .lock()
            .await
            .did_change(params.clone())
            .await;
        self.analyze_file(params.text_document.uri).await;
    }
    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        self.open_file_manager.lock().await.did_close(params).await;
    }

    async fn folding_range(
        &self,
        params: FoldingRangeParams,
    ) -> jsonrpc::Result<Option<Vec<FoldingRange>>> {
        let source = self.get_file_content(params.text_document.uri).await?;
        Ok(Some(compute_folding_ranges(&source)))
    }
}

impl CandyLanguageServer {
    async fn analyze_file(&self, uri: Url) {
        let source = match self.get_file_content(uri.clone()).await {
            Ok(source) => source,
            Err(error) => {
                log::error!("{:?}", error);
                return;
            }
        };
        let cst = source.parse_cst();
        let (ast, ast_cst_id_mapping, ast_errors) = cst.clone().into_ast();
        let (_, _, hir_errors) = ast.compile_to_hir(cst, ast_cst_id_mapping);

        let diagnostics = ast_errors
            .into_iter()
            .chain(hir_errors.into_iter())
            .map(|it| Diagnostic {
                range: it.span.to_lsp(&source),
                severity: Some(DiagnosticSeverity::ERROR),
                code: None,
                code_description: None,
                source: Some("🍭 Candy".to_owned()),
                message: it.message,
                related_information: None,
                tags: None,
                data: None,
            })
            .collect();
        self.client
            .publish_diagnostics(uri, diagnostics, None)
            .await;
    }
    async fn get_file_content(&self, uri: Url) -> jsonrpc::Result<String> {
        match self.open_file_manager.lock().await.get(&uri) {
            Some(text) => Ok(text.to_owned()),
            None => {
                let file_path = uri.to_file_path().unwrap();
                fs::read_to_string(&file_path)
                    .await
                    .map_err(|it| jsonrpc::Error {
                        code: jsonrpc::ErrorCode::InternalError,
                        message: it.to_string(),
                        data: None,
                    })
            }
        }
    }
}
