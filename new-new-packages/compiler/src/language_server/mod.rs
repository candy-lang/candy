use im::HashMap;
use lsp_types::{
    DidChangeTextDocumentParams, DidCloseTextDocumentParams, DidOpenTextDocumentParams,
    DocumentFilter, InitializeParams, InitializeResult, InitializedParams, MessageType,
    Registration, ServerCapabilities, ServerInfo, TextDocumentChangeRegistrationOptions,
    TextDocumentRegistrationOptions, Url,
};
use lspower::{jsonrpc, Client, LanguageServer};
use tokio::sync::Mutex;

#[derive(Debug)]
pub struct CandyLanguageServer {
    pub client: Client,
    pub open_files: Mutex<HashMap<Url, String>>,
}
impl CandyLanguageServer {
    pub fn from_client(client: Client) -> Self {
        Self {
            client,
            open_files: Mutex::new(HashMap::new()),
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
        match params.text_document.language_id.as_str() {
            "candy" => {
                let mut open_files = self.open_files.lock().await;
                let current_value =
                    open_files.insert(params.text_document.uri, params.text_document.text);
                assert!(current_value.is_none());
            }
            _ => return,
        }
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let mut open_files = self.open_files.lock().await;
        let DidChangeTextDocumentParams {
            content_changes,
            text_document,
        } = params;
        open_files
            .entry(text_document.uri)
            .and_modify(move |text| {
                for change in content_changes {
                    match change.range {
                        Some(range) => {
                            log::info!("received did_change with range: {:?}", range);
                        }
                        None => *text = change.text,
                    }
                }
            })
            .or_insert_with(|| panic!("Received a change for a file that was not open."));
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        let mut open_files = self.open_files.lock().await;
        open_files
            .remove(&params.text_document.uri)
            .expect("File was closed without being opened.");
    }
}
