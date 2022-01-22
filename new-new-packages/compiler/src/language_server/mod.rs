use lsp_types::{
    DidChangeTextDocumentParams, DidCloseTextDocumentParams, DidOpenTextDocumentParams,
    DocumentFilter, InitializeParams, InitializeResult, InitializedParams, MessageType,
    Registration, ServerCapabilities, ServerInfo, TextDocumentChangeRegistrationOptions,
    TextDocumentRegistrationOptions,
};
use lspower::{jsonrpc, Client, LanguageServer};
use tokio::sync::Mutex;

use self::open_file_manager::OpenFileManager;

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
        let capabilities = ServerCapabilities {
            // semantic_tokens_provider: Some(SemanticTokensServerCapabilities {}),
            ..ServerCapabilities::default()
        };
        Ok(InitializeResult {
            capabilities,
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
        self.client
            .register_capability(vec![
                Registration {
                    id: "0".to_owned(),
                    method: "textDocument/didOpen".to_owned(),
                    register_options: Some(
                        serde_json::to_value(TextDocumentRegistrationOptions {
                            document_selector: Some(vec![candy_files.clone()]),
                        })
                        .unwrap(),
                    ),
                },
                Registration {
                    id: "1".to_owned(),
                    method: "textDocument/didOpen".to_owned(),
                    register_options: Some(
                        serde_json::to_value(TextDocumentRegistrationOptions {
                            document_selector: Some(vec![candy_files.clone()]),
                        })
                        .unwrap(),
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
        self.open_file_manager.lock().await.did_open(params).await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        self.open_file_manager.lock().await.did_change(params).await;
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        self.open_file_manager.lock().await.did_close(params).await;
    }
}
