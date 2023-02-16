use async_trait::async_trait;
use candy_frontend::{
    ast_to_hir::AstToHir,
    cst_to_ast::CstToAst,
    hir_to_mir::HirToMir,
    module::{Module, ModuleKind},
    position::{line_start_offsets_raw, Offset},
    rich_ir::{Reference, RichIr, ToRichIr, TokenModifier, TokenType},
    string_to_rcst::{InvalidModuleError, StringToRcst},
    TracingConfig,
};
use extension_trait::extension_trait;
use lsp_types::{
    self, notification::Notification, DocumentHighlight, DocumentHighlightKind, FoldingRange,
    FoldingRangeKind, LocationLink, SemanticToken, Url,
};
use rustc_hash::FxHashMap;
use serde::{Deserialize, Serialize};
use std::{fmt::Debug, hash::Hash, ops::Range, path::Path, sync::Arc};
use strum_macros::EnumIter;
use tokio::sync::{Mutex, RwLock};
use tower_lsp::jsonrpc;

use crate::{
    database::Database,
    features::LanguageFeatures,
    semantic_tokens::{SemanticTokenModifier, SemanticTokenType, SemanticTokensBuilder},
    server::Server,
    utils::{lsp_position_to_offset_raw, module_from_package_root_and_url, range_to_lsp_range_raw},
};

#[derive(Debug, Eq, PartialEq, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ViewIrParams {
    pub uri: Url,
}

impl Server {
    pub async fn candy_view_ir(&self, params: ViewIrParams) -> jsonrpc::Result<String> {
        let config = self.decode_config(&params.uri).await;
        let ir = {
            let db = self.db.lock().await;
            match config.ir {
                Ir::Rcst => match db.rcst(config.module.clone()) {
                    Ok(rcst) => Some(rcst.to_rich_ir()),
                    Err(InvalidModuleError::DoesNotExist) => None,
                    Err(InvalidModuleError::InvalidUtf8) => Some("# Invalid UTF-8".to_rich_ir()),
                    Err(InvalidModuleError::IsToolingModule) => {
                        Some("# Is a tooling module".to_rich_ir())
                    }
                },
                Ir::Ast => db
                    .ast(config.module.clone())
                    .map(|(asts, _)| asts.to_rich_ir()),
                Ir::Hir => db
                    .hir(config.module.clone())
                    .map(|(body, _)| body.to_rich_ir()),
                Ir::Mir => db
                    .mir(config.module.clone(), TracingConfig::off()) // FIXME
                    .map(|mir| mir.to_rich_ir()),
            }
        }
        .unwrap_or_else(|| "# Module does not exist".to_rich_ir());

        let open_irs = {
            let state = self.state.read().await;
            state.require_features().ir.open_irs.clone()
        };
        let text = ir.text.clone();
        open_irs.write().await.insert(
            params.uri,
            OpenIr {
                config,
                ir,
                line_start_offsets: line_start_offsets_raw(&text),
            },
        );
        Ok(text)
    }
    async fn decode_config(&self, uri: &Url) -> IrConfig {
        let (path, ir) = uri.path().rsplit_once('.').unwrap();
        let original_scheme = uri.fragment().unwrap();
        let original_uri = format!("{original_scheme}:{path}").parse().unwrap();

        let ir = match ir {
            "rcst" => Ir::Rcst,
            "ast" => Ir::Ast,
            "hir" => Ir::Hir,
            "mir" => Ir::Mir,
            _ => panic!("Unsupported IR: {ir}"),
        };

        let state = self.state.read().await;
        IrConfig {
            module: module_from_package_root_and_url(
                state.require_running().project_directory.to_owned(),
                &original_uri,
                ModuleKind::Code, // FIXME
            )
            .unwrap(),
            ir,
        }
    }
}

#[derive(Debug, Default)]
pub struct IrFeatures {
    open_irs: Arc<RwLock<FxHashMap<Url, OpenIr>>>,
}

#[derive(Debug)]
struct OpenIr {
    config: IrConfig,
    ir: RichIr,
    line_start_offsets: Vec<Offset>,
}
#[derive(Debug)]
struct IrConfig {
    module: Module,
    ir: Ir,
}
#[derive(Debug, EnumIter, PartialEq, Eq, Clone, Copy, Hash)]
pub enum Ir {
    Rcst,
    Ast,
    Hir,
    Mir,
}
impl IrFeatures {
    pub async fn generate_update_notifications(
        &self,
        module: &Module,
    ) -> Vec<UpdateIrNotification> {
        let open_irs = self.open_irs.read().await;
        open_irs
            .iter()
            .filter(|(_, open_ir)| &open_ir.config.module == module)
            .map(|(uri, _)| UpdateIrNotification { uri: uri.clone() })
            .collect()
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
impl LanguageFeatures for IrFeatures {
    fn language_id(&self) -> Option<String> {
        None
    }
    fn supported_url_schemes(&self) -> Vec<&'static str> {
        vec!["candy-ir"]
    }

    fn supports_find_definition(&self) -> bool {
        true
    }
    async fn find_definition(
        &self,
        _db: &Mutex<Database>,
        _project_directory: &Path,
        uri: Url,
        position: lsp_types::Position,
    ) -> Option<LocationLink> {
        let open_irs = self.open_irs.read().await;
        let open_ir = open_irs.get(&uri).unwrap();
        open_ir.find_definition(uri, position)
    }

    fn supports_folding_ranges(&self) -> bool {
        true
    }
    async fn folding_ranges(
        &self,
        _db: &Mutex<Database>,
        _project_directory: &Path,
        uri: Url,
    ) -> Vec<FoldingRange> {
        let open_irs = self.open_irs.read().await;
        let open_ir = open_irs.get(&uri).unwrap();
        open_ir.folding_ranges()
    }

    fn supports_references(&self) -> bool {
        true
    }
    async fn references(
        &self,
        _db: &Mutex<Database>,
        _project_directory: &Path,
        uri: Url,
        position: lsp_types::Position,
        include_declaration: bool,
    ) -> Option<Vec<DocumentHighlight>> {
        let open_irs = self.open_irs.read().await;
        let open_ir = open_irs.get(&uri).unwrap();
        open_ir.references(position, include_declaration)
    }

    fn supports_semantic_tokens(&self) -> bool {
        true
    }
    async fn semantic_tokens(
        &self,
        _db: &Mutex<Database>,
        _project_directory: &Path,
        uri: Url,
    ) -> Vec<SemanticToken> {
        let open_irs = self.open_irs.read().await;
        let open_ir = open_irs.get(&uri).unwrap();
        open_ir.semantic_tokens()
    }
}

impl OpenIr {
    fn find_definition(&self, uri: Url, position: lsp_types::Position) -> Option<LocationLink> {
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
            target_uri: uri,
            target_range,
            target_selection_range: target_range,
        })
    }

    fn folding_ranges(&self) -> Vec<FoldingRange> {
        self.ir
            .folding_ranges
            .iter()
            .map(|range| {
                let range = self.range_to_lsp_range(range);
                FoldingRange {
                    start_line: range.start.line,
                    start_character: Some(range.start.character),
                    end_line: range.end.line,
                    end_character: Some(range.end.character),
                    kind: Some(FoldingRangeKind::Region),
                }
            })
            .collect()
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
        self.ir.references.iter().find(|value| {
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
            TokenType::Module => SemanticTokenType::Module,
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
