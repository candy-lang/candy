use async_trait::async_trait;
use candy_frontend::{
    ast_to_hir::AstToHir,
    cst_to_ast::CstToAst,
    module::Module,
    position::{line_start_offsets_raw, Offset},
    rich_ir::{Reference, RichIr, ToRichIr, TokenModifier, TokenType},
    string_to_rcst::{InvalidModuleError, StringToRcst},
};
use extension_trait::extension_trait;
use lsp_types::{
    self, notification::Notification, DocumentHighlight, DocumentHighlightKind, LocationLink,
    SemanticToken, Url,
};
use rustc_hash::FxHashMap;
use serde::{Deserialize, Serialize};
use std::{fmt::Debug, hash::Hash, ops::Range, sync::Arc};
use strum_macros::EnumIter;
use tokio::sync::{Mutex, RwLock};
use tower_lsp::jsonrpc;

use crate::{
    database::Database,
    features::LanguageFeatures,
    semantic_tokens::{SemanticTokenModifier, SemanticTokenType, SemanticTokensBuilder},
    server::{Server, ServerFeatures},
    utils::{lsp_position_to_offset_raw, range_to_lsp_range_raw},
};

#[derive(Debug, EnumIter, PartialEq, Eq, Clone, Copy, Hash, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Ir {
    Rcst,
    Ast,
    Hir,
}

#[derive(Debug, Eq, PartialEq, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ViewIrParams {
    pub uri: Url,
    pub ir: Ir,
}

impl Server {
    pub async fn candy_view_ir(&self, params: ViewIrParams) -> jsonrpc::Result<String> {
        match params.ir {
            Ir::Rcst => {
                self.view_ir(
                    params,
                    |features| &features.rcst,
                    |db, module| match db.rcst(module) {
                        Ok(rcst) => Some(rcst.to_rich_ir()),
                        Err(InvalidModuleError::DoesNotExist) => None,
                        Err(InvalidModuleError::InvalidUtf8) => {
                            Some("# Invalid UTF-8".to_rich_ir())
                        }
                        Err(InvalidModuleError::IsToolingModule) => {
                            Some("# Is a tooling module".to_rich_ir())
                        }
                    },
                )
                .await
            }
            Ir::Ast => {
                self.view_ir(
                    params,
                    |features| &features.ast,
                    |db, module| db.ast(module).map(|(asts, _)| asts.to_rich_ir()),
                )
                .await
            }
            Ir::Hir => {
                self.view_ir(
                    params,
                    |features| &features.hir,
                    |db, module| db.hir(module).map(|(body, _)| body.to_rich_ir()),
                )
                .await
            }
        }
    }
    async fn view_ir<FF, IF, RK>(
        &self,
        params: ViewIrParams,
        get_features: FF,
        get_ir: IF,
    ) -> jsonrpc::Result<String>
    where
        FF: FnOnce(&ServerFeatures) -> &IrFeatures<RK>,
        IF: FnOnce(&Database, Module) -> Option<RichIr<RK>>,
        RK: Eq + Hash,
    {
        let module = self.code_module_from_url(params.uri.clone()).await;

        let (url_scheme, open_irs) = {
            let state = self.state.read().await;
            let features = get_features(state.require_features());
            (features.url_scheme, features.open_irs.clone())
        };
        let ir_uri = format!(
            "{url_scheme}:{}?scheme={}",
            params.uri.path(),
            urlencoding::encode(params.uri.scheme()),
        )
        .parse()
        .unwrap();

        let ir = {
            let db = self.db.lock().await;
            get_ir(&db, module.clone()).unwrap_or_else(|| "# Module does not exist".to_rich_ir())
        };
        let text = ir.text.clone();
        open_irs.write().await.insert(
            module,
            OpenIr {
                uri: ir_uri,
                ir,
                line_start_offsets: line_start_offsets_raw(&text),
            },
        );
        Ok(text)
    }
}

#[derive(Debug)]
pub struct IrFeatures<RK: Eq + Hash> {
    url_scheme: &'static str,
    open_irs: Arc<RwLock<FxHashMap<Module, OpenIr<RK>>>>,
}
#[derive(Debug)]
struct OpenIr<RK: Eq + Hash> {
    uri: Url,
    ir: RichIr<RK>,
    line_start_offsets: Vec<Offset>,
}
impl<RK: Eq + Hash> IrFeatures<RK> {
    pub fn new_rcst() -> Self {
        Self::new("candy-rcst")
    }
    pub fn new_ast() -> Self {
        Self::new("candy-ast")
    }
    pub fn new_hir() -> Self {
        Self::new("candy-hir")
    }
    fn new(url_scheme: &'static str) -> Self {
        Self {
            url_scheme,
            open_irs: Arc::default(),
        }
    }

    pub async fn maybe_generate_update_notification(
        &self,
        module: &Module,
    ) -> Option<UpdateIrNotification> {
        let open_irs = self.open_irs.read().await;
        let open_ir = open_irs.get(module)?;
        Some(UpdateIrNotification {
            uri: open_ir.uri.clone(),
        })
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateIrNotification {
    pub uri: Url,
}
impl Notification for UpdateIrNotification {
    const METHOD: &'static str = "candy/updateIr";

    type Params = Self;
}

#[async_trait]
impl<RK: Eq + Hash + Send + Sync + Debug> LanguageFeatures for IrFeatures<RK> {
    fn language_id(&self) -> Option<String> {
        None
    }
    fn supported_url_schemes(&self) -> Vec<String> {
        vec![self.url_scheme.to_string()]
    }

    fn supports_find_definition(&self) -> bool {
        true
    }
    async fn find_definition(
        &self,
        _db: &Mutex<Database>,
        module: Module,
        position: lsp_types::Position,
    ) -> Option<LocationLink> {
        let open_irs = self.open_irs.read().await;
        let open_ir = open_irs.get(&module).unwrap();
        open_ir.find_definition(position)
    }

    fn supports_references(&self) -> bool {
        true
    }
    async fn references(
        &self,
        _db: &Mutex<Database>,
        module: Module,
        position: lsp_types::Position,
        include_declaration: bool,
    ) -> Option<Vec<DocumentHighlight>> {
        let open_irs = self.open_irs.read().await;
        let open_ir = open_irs.get(&module).unwrap();
        open_ir.references(position, include_declaration)
    }

    fn supports_semantic_tokens(&self) -> bool {
        true
    }
    async fn semantic_tokens(&self, _db: &Mutex<Database>, module: Module) -> Vec<SemanticToken> {
        let open_irs = self.open_irs.read().await;
        let open_ir = open_irs.get(&module).unwrap();
        open_ir.semantic_tokens()
    }
}

impl<RK: Eq + Hash> OpenIr<RK> {
    fn find_definition(&self, position: lsp_types::Position) -> Option<LocationLink> {
        let offset = self.lsp_position_to_offset(position);
        let result = self.references_entry(offset)?;
        let definition = result.definition.clone()?;

        let origin_selection_range = result
            .references
            .iter()
            .find(|it| it.contains(&offset))
            .unwrap_or(&definition);
        let target_range = self.range_to_lsp_range(&definition);
        Some(LocationLink {
            origin_selection_range: Some(self.range_to_lsp_range(origin_selection_range)),
            target_uri: self.uri.clone(),
            target_range,
            target_selection_range: target_range,
        })
    }
    fn references(
        &self,
        position: lsp_types::Position,
        include_declaration: bool,
    ) -> Option<Vec<DocumentHighlight>> {
        let offset = self.lsp_position_to_offset(position);
        let result = self.references_entry(offset)?;
        let mut highlights = Vec::with_capacity(
            (include_declaration && result.definition.is_some()) as usize + result.references.len(),
        );
        if include_declaration && let Some(definition) = &result.definition {
            highlights.push(DocumentHighlight {
                range: self.range_to_lsp_range(definition),
                kind: Some(DocumentHighlightKind::WRITE),
            })
        }
        for reference in &result.references {
            highlights.push(DocumentHighlight {
                range: self.range_to_lsp_range(reference),
                kind: Some(DocumentHighlightKind::READ),
            })
        }
        Some(highlights)
    }
    fn references_entry(&self, offset: Offset) -> Option<&Reference> {
        self.ir.references.values().find(|value| {
            value
                .definition
                .as_ref()
                .map(|it| it.contains(&offset))
                .unwrap_or_default()
                || value.references.iter().any(|it| it.contains(&offset))
        })
    }

    fn semantic_tokens(&self) -> Vec<SemanticToken> {
        let mut builder = SemanticTokensBuilder::new(&self.ir.text, &self.line_start_offsets);
        for annotation in &self.ir.annotations {
            let Some(token_type) = annotation.token_type else { continue; };
            builder.add(
                annotation.range.clone(),
                token_type.to_semantic(),
                annotation
                    .token_modifiers
                    .iter()
                    .map(|it| it.to_semantic())
                    .collect(),
            );
        }
        builder.finish()
    }

    fn lsp_position_to_offset(&self, position: lsp_types::Position) -> Offset {
        lsp_position_to_offset_raw(&self.ir.text, &self.line_start_offsets, position)
    }
    fn range_to_lsp_range(&self, range: &Range<Offset>) -> lsp_types::Range {
        range_to_lsp_range_raw(&self.ir.text, &self.line_start_offsets, range)
    }
}

#[extension_trait]
impl TokenTypeToSemantic for TokenType {
    fn to_semantic(&self) -> SemanticTokenType {
        match self {
            TokenType::Parameter => SemanticTokenType::Parameter,
            TokenType::Variable => SemanticTokenType::Variable,
            TokenType::Function => SemanticTokenType::Function,
            TokenType::Symbol => SemanticTokenType::Symbol,
            TokenType::Text => SemanticTokenType::Text,
            TokenType::Int => SemanticTokenType::Int,
        }
    }
}

#[extension_trait]
impl TokenModifierToSemantic for TokenModifier {
    fn to_semantic(&self) -> SemanticTokenModifier {
        match self {
            TokenModifier::Builtin => SemanticTokenModifier::Builtin,
        }
    }
}
