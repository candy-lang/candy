use async_trait::async_trait;
use candy_frontend::module::{Module, ModuleKind};
use lsp_types::{
    DidChangeTextDocumentParams, DidCloseTextDocumentParams, DidOpenTextDocumentParams,
    DocumentFilter, DocumentHighlight, DocumentHighlightParams, FoldingRange, FoldingRangeParams,
    GotoDefinitionParams, GotoDefinitionResponse, InitializeParams, InitializeResult,
    InitializedParams, Location, MessageType, Position, ReferenceParams, Registration,
    SemanticTokens, SemanticTokensFullOptions, SemanticTokensOptions, SemanticTokensParams,
    SemanticTokensRegistrationOptions, SemanticTokensResult, SemanticTokensServerCapabilities,
    ServerCapabilities, ServerInfo, StaticRegistrationOptions,
    TextDocumentChangeRegistrationOptions, TextDocumentRegistrationOptions, Url,
    WorkDoneProgressOptions,
};
use rustc_hash::FxHashMap;
use serde::Serialize;
use std::{mem, path::PathBuf};
use strum::IntoEnumIterator;
use tokio::sync::{Mutex, RwLock, RwLockReadGuard, RwLockWriteGuard};
use tower_lsp::{jsonrpc, Client, ClientSocket, LanguageServer, LspService};
use tracing::{debug, span, Level};

use crate::{
    database::Database,
    features::LanguageFeatures,
    features_candy::{hints::HintsNotification, CandyFeatures},
    features_ir::{Ir, IrFeatures},
    semantic_tokens,
    utils::{module_from_package_root_and_url, module_to_url, LspPositionConversion},
};

pub struct Server {
    pub client: Client,
    pub db: Mutex<Database>,
    pub state: RwLock<ServerState>,
}
pub enum ServerState {
    Initial {
        features: ServerFeatures,
    },
    Running {
        project_directory: PathBuf,
        features: ServerFeatures,
    },
    Shutdown,
}
impl ServerState {
    pub fn require_features(&self) -> &ServerFeatures {
        match self {
            ServerState::Initial { features } => features,
            ServerState::Running { features, .. } => features,
            ServerState::Shutdown => panic!("Server is shut down"),
        }
    }
    pub fn require_features_mut(&mut self) -> &mut ServerFeatures {
        match self {
            ServerState::Initial { features } => features,
            ServerState::Running { features, .. } => features,
            ServerState::Shutdown => panic!("Server is shut down"),
        }
    }
}

pub struct ServerFeatures {
    pub candy: CandyFeatures,
    pub ir: FxHashMap<Ir, IrFeatures>,
}
impl ServerFeatures {
    fn selectors_where<F>(&self, mut filter: F) -> Vec<DocumentFilter>
    where
        F: FnMut(&dyn LanguageFeatures) -> bool,
    {
        let mut selectors = vec![];

        let mut add_selectors_for = move |selectors: &mut Vec<DocumentFilter>, features| {
            if !filter(features) {
                return;
            }

            let language_id = features.language_id();
            let schemes = features.supported_url_schemes();
            assert!(!schemes.is_empty());

            selectors.extend(schemes.into_iter().map(|scheme| DocumentFilter {
                language: language_id.clone(),
                scheme: Some(scheme),
                pattern: None,
            }))
        };
        add_selectors_for(&mut selectors, &self.candy);
        for features in self.ir.values() {
            add_selectors_for(&mut selectors, features);
        }
        selectors
    }
    fn registration_options_where<F>(&self, filter: F) -> TextDocumentRegistrationOptions
    where
        F: FnMut(&dyn LanguageFeatures) -> bool,
    {
        TextDocumentRegistrationOptions {
            document_selector: Some(self.selectors_where(filter)),
        }
    }
}

impl Server {
    pub fn create() -> (LspService<Self>, ClientSocket) {
        let (diagnostics_sender, mut diagnostics_receiver) = tokio::sync::mpsc::channel(8);
        let (hints_sender, mut hints_receiver) = tokio::sync::mpsc::channel(1024);

        let (service, client) = LspService::build(|client| {
            let candy_features = CandyFeatures::new(diagnostics_sender, hints_sender);
            let ir_features = Ir::iter().map(|ir| (ir, IrFeatures::new(ir))).collect();

            // let (mut diagnostics_receiver, mut hints_receiver) = {
            //     let mut receivers = self.diagnostics_and_hints_receiver.lock().await;
            //     mem::take(&mut *receivers).unwrap()
            // };
            let client_for_closure = client.clone();
            let diagnostics_reporter = async move || {
                while let Some((module, diagnostics)) = diagnostics_receiver.recv().await {
                    client_for_closure
                        .publish_diagnostics(module_to_url(&module).unwrap(), diagnostics, None)
                        .await;
                }
            };
            tokio::spawn(diagnostics_reporter());
            let client_for_closure = client.clone();
            let hint_reporter = async move || {
                while let Some((module, hints)) = hints_receiver.recv().await {
                    client_for_closure
                        .send_notification::<HintsNotification>(HintsNotification {
                            uri: module_to_url(&module).unwrap(),
                            hints,
                        })
                        .await;
                }
            };
            tokio::spawn(hint_reporter());

            Self {
                client,
                db: Default::default(),
                state: RwLock::new(ServerState::Initial {
                    features: ServerFeatures {
                        candy: candy_features,
                        ir: ir_features,
                    },
                }),
            }
        })
        .custom_method("candy/viewRcst", Server::candy_view_rcst)
        .custom_method("candy/viewAst", Server::candy_view_ast)
        .finish();

        (service, client)
    }

    pub async fn require_features(&self) -> RwLockReadGuard<ServerFeatures> {
        RwLockReadGuard::map(self.state.read().await, ServerState::require_features)
    }

    pub async fn code_module_from_url(&self, url: Url) -> Module {
        let ServerState::Running { ref project_directory, .. } = *self.state.read().await else {
            panic!("Server not running");
        };
        module_from_package_root_and_url(project_directory.to_owned(), url, ModuleKind::Code)
    }
    pub async fn ir_and_module_from_url(&self, url: Url) -> (Option<Ir>, Module) {
        let ir = match url.scheme() {
            "candy-rcst" => Some(Ir::Rcst),
            "candy-ast" => Some(Ir::Ast),
            _ => None,
        };

        let original_url = if ir.is_some() {
            let original_scheme = url.query().unwrap().strip_prefix("scheme%3D").unwrap();
            let original_scheme = urlencoding::decode(original_scheme).unwrap();
            Url::parse(&format!("{}://{}", original_scheme, url.path())).unwrap()
        } else {
            url
        };
        let module = self.code_module_from_url(original_url).await;

        (ir, module)
    }
    pub async fn features_and_module_from_url(
        &self,
        url: Url,
    ) -> (RwLockReadGuard<dyn LanguageFeatures>, Module) {
        let (ir, module) = self.ir_and_module_from_url(url).await;
        let features = RwLockReadGuard::map(self.state.read().await, |state| {
            let features = state.require_features();
            ir.map(|ir| features.ir.get(&ir).unwrap() as &dyn LanguageFeatures)
                .unwrap_or_else(|| &features.candy)
        });
        (features, module)
    }
}

#[async_trait]
impl LanguageServer for Server {
    async fn initialize(&self, params: InitializeParams) -> jsonrpc::Result<InitializeResult> {
        span!(Level::DEBUG, "LSP: initialize");
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
        let project_directory = match first_workspace_folder.scheme() {
            "file" => first_workspace_folder.to_file_path().unwrap(),
            _ => panic!("Workspace folder must be a file URI."),
        };

        {
            let state = self.state.read().await;
            let features = state.require_features();
            features.candy.initialize().await;
            for feature in features.ir.values() {
                feature.initialize().await;
            }
        }

        {
            RwLockWriteGuard::map(self.state.write().await, |state| {
                let owned_state = mem::replace(state, ServerState::Shutdown);
                let ServerState::Initial { features } = owned_state else { panic!("Already initialized"); };
                *state = ServerState::Running {
                    project_directory,
                    features,
                };
                state
            });
        }

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
        debug!("LSP: initialized");

        fn registration(method: &'static str, options: impl Serialize) -> Registration {
            Registration {
                id: method.to_string(),
                method: method.to_string(),
                register_options: Some(serde_json::to_value(options).unwrap()),
            }
        }
        let state = self.state.read().await;
        let features = state.require_features();

        self.client
            .register_capability(vec![
                registration(
                    "textDocument/didOpen",
                    features.registration_options_where(|it| it.supports_did_open()),
                ),
                registration(
                    "textDocument/didChange",
                    TextDocumentChangeRegistrationOptions {
                        document_selector: Some(
                            features.selectors_where(|it| it.supports_did_change()),
                        ),
                        sync_kind: 2, // incremental
                    },
                ),
                registration(
                    "textDocument/didClose",
                    features.registration_options_where(|it| it.supports_did_close()),
                ),
                registration(
                    "textDocument/definition",
                    features.registration_options_where(|it| it.supports_find_definition()),
                ),
                registration(
                    "textDocument/references",
                    features.registration_options_where(|it| it.supports_references()),
                ),
                registration(
                    "textDocument/documentHighlight",
                    features.registration_options_where(|it| it.supports_references()),
                ),
                registration(
                    "textDocument/foldingRange",
                    features.registration_options_where(|it| it.supports_folding_ranges()),
                ),
                registration(
                    "textDocument/semanticTokens",
                    SemanticTokensServerCapabilities::SemanticTokensRegistrationOptions(
                        SemanticTokensRegistrationOptions {
                            text_document_registration_options: features
                                .registration_options_where(|it| it.supports_semantic_tokens()),
                            semantic_tokens_options: SemanticTokensOptions {
                                work_done_progress_options: WorkDoneProgressOptions {
                                    work_done_progress: None,
                                },
                                legend: semantic_tokens::LEGEND.clone(),
                                // TODO
                                range: Some(false),
                                full: Some(SemanticTokensFullOptions::Bool(true)),
                            },
                            static_registration_options: StaticRegistrationOptions { id: None },
                        },
                    ),
                ),
            ])
            .await
            .expect("Dynamic capability registration failed.");
        self.client
            .log_message(MessageType::INFO, "server initialized!")
            .await;
    }

    async fn shutdown(&self) -> jsonrpc::Result<()> {
        let mut owned_state = {
            let mut state = self.state.write().await;
            mem::replace(&mut *state, ServerState::Shutdown)
        };
        let features = owned_state.require_features_mut();
        features.candy.shutdown().await;
        for (_, feature) in features.ir.drain() {
            feature.shutdown().await;
        }
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let (features, module) = self
            .features_and_module_from_url(params.text_document.uri)
            .await;
        assert!(features.supports_did_open());
        let content = params.text_document.text.into_bytes();
        features.did_open(&self.db, module, content).await;
    }
    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let (features, module) = self
            .features_and_module_from_url(params.text_document.uri)
            .await;
        assert!(features.supports_did_change());
        features
            .did_change(&self.db, module, params.content_changes)
            .await;
    }
    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        let (features, module) = self
            .features_and_module_from_url(params.text_document.uri)
            .await;
        assert!(features.supports_did_close());
        features.did_close(&self.db, module).await;
    }

    async fn goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> jsonrpc::Result<Option<GotoDefinitionResponse>> {
        let (features, module) = self
            .features_and_module_from_url(params.text_document_position_params.text_document.uri)
            .await;
        assert!(features.supports_find_definition());
        let db = self.db.lock().await;
        let offset = db.lsp_position_to_offset(
            module.clone(),
            params.text_document_position_params.position,
        );
        let response = features
            .find_definition(&db, module, offset)
            .map(|link| GotoDefinitionResponse::Link(vec![link]));
        Ok(response)
    }

    async fn references(&self, params: ReferenceParams) -> jsonrpc::Result<Option<Vec<Location>>> {
        let uri = params.text_document_position.text_document.uri;
        let highlights = self
            .references_raw(
                uri.clone(),
                params.text_document_position.position,
                params.context.include_declaration,
            )
            .await;
        let response = highlights.map(|highlights| {
            highlights
                .into_iter()
                .map(|highlight| Location {
                    uri: uri.clone(),
                    range: highlight.range,
                })
                .collect()
        });
        Ok(response)
    }
    async fn document_highlight(
        &self,
        params: DocumentHighlightParams,
    ) -> jsonrpc::Result<Option<Vec<DocumentHighlight>>> {
        let response = self
            .references_raw(
                params.text_document_position_params.text_document.uri,
                params.text_document_position_params.position,
                true,
            )
            .await;
        Ok(response)
    }

    async fn folding_range(
        &self,
        params: FoldingRangeParams,
    ) -> jsonrpc::Result<Option<Vec<FoldingRange>>> {
        let (features, module) = self
            .features_and_module_from_url(params.text_document.uri)
            .await;
        assert!(features.supports_folding_ranges());
        let db = self.db.lock().await;
        Ok(Some(features.folding_ranges(&db, module)))
    }

    async fn semantic_tokens_full(
        &self,
        params: SemanticTokensParams,
    ) -> jsonrpc::Result<Option<SemanticTokensResult>> {
        let (features, module) = self
            .features_and_module_from_url(params.text_document.uri)
            .await;
        let db = self.db.lock().await;
        let tokens = features.semantic_tokens(&db, module);
        let tokens = tokens.await;
        Ok(Some(SemanticTokensResult::Tokens(SemanticTokens {
            result_id: None,
            data: tokens,
        })))
    }
}
impl Server {
    async fn references_raw(
        &self,
        uri: Url,
        position: Position,
        include_declaration: bool,
    ) -> Option<Vec<DocumentHighlight>> {
        let (features, module) = self.features_and_module_from_url(uri).await;
        assert!(features.supports_references());
        let db = self.db.lock().await;
        let offset = db.lsp_position_to_offset(module.clone(), position);
        features.references(&db, module, offset, include_declaration)
    }
}
