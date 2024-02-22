use crate::{
    database::Database,
    debug_adapter::DebugSessionManager,
    features::{LanguageFeatures, Reference, RenameError},
    features_candy::{
        analyzer::{insights::Hint, HintsNotification},
        CandyFeatures, ServerStatusNotification,
    },
    features_ir::{IrFeatures, UpdateIrNotification},
    semantic_tokens,
    utils::{module_from_url, module_to_url},
};
use async_trait::async_trait;
use candy_frontend::module::{Module, ModuleKind, PackagesPath};
use lsp_types::{
    Diagnostic, DidChangeTextDocumentParams, DidCloseTextDocumentParams, DidOpenTextDocumentParams,
    DocumentFilter, DocumentFormattingParams, DocumentHighlight, DocumentHighlightKind,
    DocumentHighlightParams, FoldingRange, FoldingRangeParams, GotoDefinitionParams,
    GotoDefinitionResponse, InitializeParams, InitializeResult, InitializedParams, Location,
    MessageType, Position, PrepareRenameResponse, ReferenceParams, Registration, RenameOptions,
    RenameParams, SemanticTokens, SemanticTokensFullOptions, SemanticTokensOptions,
    SemanticTokensParams, SemanticTokensRegistrationOptions, SemanticTokensResult,
    SemanticTokensServerCapabilities, ServerCapabilities, ServerInfo, StaticRegistrationOptions,
    TextDocumentChangeRegistrationOptions, TextDocumentPositionParams,
    TextDocumentRegistrationOptions, TextEdit, Url, WorkDoneProgressOptions, WorkspaceEdit,
};
use rustc_hash::FxHashMap;
use serde::{Deserialize, Serialize};
use std::{borrow::Cow, mem};
use tokio::sync::{Mutex, RwLock, RwLockMappedWriteGuard, RwLockReadGuard, RwLockWriteGuard};
use tower_lsp::{jsonrpc, Client, ClientSocket, LanguageServer, LspService};
use tracing::{debug, span, Level};

pub struct Server {
    pub client: Client,
    pub db: Mutex<Database>,
    pub state: RwLock<ServerState>,
}
#[derive(Debug)]
pub enum ServerState {
    Initial {
        features: ServerFeatures,
        debug_session_manager: DebugSessionManager,
    },
    Running(RunningServerState),
    Shutdown,
}
#[derive(Debug)]
pub struct RunningServerState {
    pub features: ServerFeatures,
    pub packages_path: PackagesPath,
    pub debug_session_manager: DebugSessionManager,
}
impl ServerState {
    pub fn require_features(&self) -> &ServerFeatures {
        match self {
            Self::Initial { features, .. } => features,
            Self::Running(RunningServerState { features, .. }) => features,
            Self::Shutdown => panic!("Server is shut down."),
        }
    }
    pub fn require_running(&self) -> &RunningServerState {
        match self {
            Self::Running(state) => state,
            _ => panic!("Server is not running."),
        }
    }
    pub fn require_running_mut(&mut self) -> &mut RunningServerState {
        match self {
            Self::Running(state) => state,
            _ => panic!("Server is not running."),
        }
    }
}

#[derive(Debug)]
pub struct ServerFeatures {
    pub candy: CandyFeatures,
    pub ir: IrFeatures,
}
impl ServerFeatures {
    fn all_features<'this, 'a>(&'this self) -> [&'a dyn LanguageFeatures; 2]
    where
        'this: 'a,
    {
        [&self.candy, &self.ir]
    }

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
                scheme: Some(scheme.to_owned()),
                pattern: None,
            }));
        };
        for features in self.all_features() {
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

pub struct AnalyzerClient {
    client: Client,
    packages_path: PackagesPath,
}
impl AnalyzerClient {
    pub async fn update_status(&self, status: Option<String>) {
        self.client
            .send_notification::<ServerStatusNotification>(ServerStatusNotification {
                text: status.map_or_else(|| "🍭".to_string(), |status| format!("🍭 {status}")),
            })
            .await;
    }
    pub async fn update_diagnostics(&self, module: Module, diagnostics: Vec<Diagnostic>) {
        self.client
            .publish_diagnostics(
                module_to_url(&module, &self.packages_path).unwrap(),
                diagnostics,
                None,
            )
            .await;
    }
    pub async fn update_hints(&self, module: Module, hints: Vec<Hint>) {
        self.client
            .send_notification::<HintsNotification>(HintsNotification {
                uri: module_to_url(&module, &self.packages_path).unwrap(),
                hints,
            })
            .await;
    }
}

impl Server {
    pub fn create(packages_path: PackagesPath) -> (LspService<Self>, ClientSocket) {
        let (service, client) = LspService::build(|client| {
            let state = ServerState::Initial {
                features: ServerFeatures {
                    candy: CandyFeatures::new(
                        packages_path.clone(),
                        AnalyzerClient {
                            client: client.clone(),
                            packages_path: packages_path.clone(),
                        },
                    ),
                    ir: IrFeatures::default(),
                },
                debug_session_manager: DebugSessionManager::default(),
            };

            Self {
                client,
                db: Mutex::new(Database::new_with_file_system_module_provider(
                    packages_path,
                )),
                state: RwLock::new(state),
            }
        })
        .custom_method(
            "candy/debugAdapter/create",
            Self::candy_debug_adapter_create,
        )
        .custom_method(
            "candy/debugAdapter/message",
            Self::candy_debug_adapter_message,
        )
        .custom_method("candy/viewIr", Self::candy_view_ir)
        .finish();

        (service, client)
    }

    pub async fn require_features(&self) -> RwLockReadGuard<ServerFeatures> {
        RwLockReadGuard::map(self.state.read().await, ServerState::require_features)
    }

    pub async fn require_running_state(&self) -> RwLockReadGuard<RunningServerState> {
        RwLockReadGuard::map(self.state.read().await, |state| state.require_running())
    }
    pub async fn require_running_state_mut(&self) -> RwLockMappedWriteGuard<RunningServerState> {
        RwLockWriteGuard::map(self.state.write().await, |state| {
            state.require_running_mut()
        })
    }
    pub fn features_from_url<'a>(
        &self,
        server_features: &'a ServerFeatures,
        url: &Url,
    ) -> &'a dyn LanguageFeatures {
        let scheme = url.scheme();
        server_features
            .all_features()
            .into_iter()
            .find(|it| it.supported_url_schemes().contains(&scheme))
            .unwrap()
    }
}

#[async_trait]
impl LanguageServer for Server {
    async fn initialize(&self, params: InitializeParams) -> jsonrpc::Result<InitializeResult> {
        span!(Level::DEBUG, "LSP: initialize");
        self.client
            .log_message(MessageType::INFO, "Initializing!")
            .await;

        {
            let state = self.state.read().await;
            for features in state.require_features().all_features() {
                features.initialize().await;
            }
        }

        let packages_path = {
            let options = params
                .initialization_options
                .as_ref()
                .expect("No initialization options provided.")
                .as_object()
                .unwrap();
            match PackagesPath::try_from(options.get("packagesPath").unwrap().as_str().unwrap()) {
                Ok(packages_path) => packages_path,
                Err(err) => {
                    let message = format!("Failed to initialize: {err}");
                    self.client
                        .show_message(MessageType::ERROR, message.clone())
                        .await;
                    return Err(jsonrpc::Error::invalid_params(message));
                }
            }
        };

        {
            let mut state = self.state.write().await;
            let owned_state = mem::replace(&mut *state, ServerState::Shutdown);
            let ServerState::Initial {
                features,
                debug_session_manager,
            } = owned_state
            else {
                panic!("Server is already initialized.");
            };
            *state = ServerState::Running(RunningServerState {
                features,
                packages_path,
                debug_session_manager,
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
        fn registration(method: &'static str, options: impl Serialize) -> Registration {
            Registration {
                id: method.to_string(),
                method: method.to_string(),
                register_options: Some(serde_json::to_value(options).unwrap()),
            }
        }

        debug!("LSP: initialized");

        let state = self.state.read().await;
        let features = state.require_features();

        // TODO: Fix lifetimes and remove this allow
        #[allow(clippy::redundant_closure_for_method_calls)]
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
                    "textDocument/formatting",
                    features.registration_options_where(|it| it.supports_format()),
                ),
                registration(
                    "textDocument/rename",
                    RenameRegistrationOptions {
                        text_document_registration_options: features
                            .registration_options_where(|it| it.supports_rename()),
                        rename_options: RenameOptions {
                            prepare_provider: Some(true),
                            work_done_progress_options: WorkDoneProgressOptions {
                                work_done_progress: None,
                            },
                        },
                    },
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
        let state = {
            let mut state = self.state.write().await;
            mem::replace(&mut *state, ServerState::Shutdown)
        };
        for features in state.require_features().all_features() {
            features.shutdown().await;
        }
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let state = self.require_running_state().await;
        let features = self.features_from_url(&state.features, &params.text_document.uri);
        assert!(features.supports_did_open());
        let content = params.text_document.text.into_bytes();
        features
            .did_open(&self.db, params.text_document.uri, content)
            .await;
    }
    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let state = self.require_running_state().await;
        {
            let features = self.features_from_url(&state.features, &params.text_document.uri);
            assert!(features.supports_did_change());
            features
                .did_change(
                    &self.db,
                    params.text_document.uri.clone(),
                    params.content_changes,
                )
                .await;
        };

        let module_result = module_from_url(
            &params.text_document.uri,
            if std::path::Path::new(params.text_document.uri.path())
                .extension()
                .map_or(false, |ext| ext.eq_ignore_ascii_case("candy"))
            {
                ModuleKind::Code
            } else {
                ModuleKind::Asset
            },
            &state.packages_path,
        );
        if let Ok(module) = module_result {
            let notifications = {
                let state = self.state.read().await;
                state
                    .require_features()
                    .ir
                    .generate_update_notifications(&module)
                    .await
            };
            for notification in notifications {
                self.client
                    .send_notification::<UpdateIrNotification>(notification)
                    .await;
            }
        }
    }
    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        let state = self.require_running_state().await;
        let features = self.features_from_url(&state.features, &params.text_document.uri);
        assert!(features.supports_did_close());
        features.did_close(&self.db, params.text_document.uri).await;
    }

    async fn goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> jsonrpc::Result<Option<GotoDefinitionResponse>> {
        let state = self.require_running_state().await;
        let features = self.features_from_url(
            &state.features,
            &params.text_document_position_params.text_document.uri,
        );
        assert!(features.supports_find_definition());
        let response = features
            .find_definition(
                &self.db,
                params.text_document_position_params.text_document.uri,
                params.text_document_position_params.position,
            )
            .await
            .map(|link| GotoDefinitionResponse::Link(vec![link]));
        Ok(response)
    }

    async fn references(&self, params: ReferenceParams) -> jsonrpc::Result<Option<Vec<Location>>> {
        let uri = params.text_document_position.text_document.uri;
        let highlights = self
            .references_raw(
                uri.clone(),
                params.text_document_position.position,
                false,
                params.context.include_declaration,
            )
            .await;
        let response = highlights
            .iter()
            .flat_map(|(uri, references)| {
                references.iter().map(|highlight| Location {
                    uri: uri.clone(),
                    range: highlight.range,
                })
            })
            .collect();
        Ok(Some(response))
    }
    async fn document_highlight(
        &self,
        params: DocumentHighlightParams,
    ) -> jsonrpc::Result<Option<Vec<DocumentHighlight>>> {
        let mut response = self
            .references_raw(
                params
                    .text_document_position_params
                    .text_document
                    .uri
                    .clone(),
                params.text_document_position_params.position,
                true,
                true,
            )
            .await;
        let highlights = response
            .remove(&params.text_document_position_params.text_document.uri)
            .unwrap_or_default()
            .iter()
            .map(|reference| DocumentHighlight {
                range: reference.range,
                kind: Some(if reference.is_write {
                    DocumentHighlightKind::WRITE
                } else {
                    DocumentHighlightKind::READ
                }),
            })
            .collect();
        Ok(Some(highlights))
    }

    async fn folding_range(
        &self,
        params: FoldingRangeParams,
    ) -> jsonrpc::Result<Option<Vec<FoldingRange>>> {
        let state = self.require_running_state().await;
        let features = self.features_from_url(&state.features, &params.text_document.uri);
        assert!(features.supports_folding_ranges());
        Ok(Some(
            features
                .folding_ranges(&self.db, params.text_document.uri)
                .await,
        ))
    }

    async fn formatting(
        &self,
        params: DocumentFormattingParams,
    ) -> jsonrpc::Result<Option<Vec<TextEdit>>> {
        let state = self.require_running_state().await;
        let features = self.features_from_url(&state.features, &params.text_document.uri);
        assert!(features.supports_format());
        Ok(Some(
            features.format(&self.db, params.text_document.uri).await,
        ))
    }

    async fn prepare_rename(
        &self,
        params: TextDocumentPositionParams,
    ) -> jsonrpc::Result<Option<PrepareRenameResponse>> {
        let state = self.require_running_state().await;
        let uri = params.text_document.uri;
        let features = self.features_from_url(&state.features, &uri);
        let result = features
            .prepare_rename(&self.db, uri, params.position)
            .await;
        Ok(result.map(PrepareRenameResponse::Range))
    }
    async fn rename(&self, params: RenameParams) -> jsonrpc::Result<Option<WorkspaceEdit>> {
        let state = self.require_running_state().await;
        let uri = params.text_document_position.text_document.uri;
        let features = self.features_from_url(&state.features, &uri);
        let result = features
            .rename(
                &self.db,
                uri,
                params.text_document_position.position,
                params.new_name,
            )
            .await;
        match result {
            Ok(changes) => Ok(Some(WorkspaceEdit {
                changes: Some(changes),
                ..Default::default()
            })),
            Err(RenameError::NewNameInvalid) => Err(jsonrpc::Error {
                code: jsonrpc::ErrorCode::InvalidParams,
                message: Cow::Borrowed("The new name is not valid."),
                data: None,
            }),
        }
    }

    async fn semantic_tokens_full(
        &self,
        params: SemanticTokensParams,
    ) -> jsonrpc::Result<Option<SemanticTokensResult>> {
        let state = self.require_running_state().await;
        let uri = params.text_document.uri;
        let features = self.features_from_url(&state.features, &uri);
        let tokens = features.semantic_tokens(&self.db, uri);
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
        only_in_same_document: bool,
        include_declaration: bool,
    ) -> FxHashMap<Url, Vec<Reference>> {
        let state = self.state.read().await;
        let state = state.require_running();
        let features = self.features_from_url(&state.features, &uri);
        assert!(features.supports_references());
        features
            .references(
                &self.db,
                uri,
                position,
                only_in_same_document,
                include_declaration,
            )
            .await
    }
}

/// <https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/#renameRegistrationOptions>
#[derive(Debug, Eq, PartialEq, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RenameRegistrationOptions {
    #[serde(flatten)]
    pub text_document_registration_options: TextDocumentRegistrationOptions,

    #[serde(flatten)]
    pub rename_options: RenameOptions,
}
