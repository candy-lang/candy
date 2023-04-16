use async_trait::async_trait;
use candy_frontend::{
    ast::Ast,
    ast_to_hir::AstToHir,
    cst_to_ast::CstToAst,
    hir,
    hir_to_mir::HirToMir,
    mir::Mir,
    mir_optimize::OptimizeMir,
    module::{Module, ModuleKind},
    position::{line_start_offsets_raw, Offset},
    rich_ir::{
        ReferenceCollection, ReferenceKey, RichIr, RichIrBuilder, ToRichIr, TokenModifier,
        TokenType,
    },
    string_to_rcst::{InvalidModuleError, RcstResult, StringToRcst},
    TracingConfig, TracingMode,
};
use candy_vm::{
    lir::{Lir, RichIrForLir},
    mir_to_lir::MirToLir,
};
use enumset::EnumSet;
use extension_trait::extension_trait;
use lsp_types::{
    self, notification::Notification, FoldingRange, FoldingRangeKind, LocationLink, SemanticToken,
    Url,
};
use rustc_hash::FxHashMap;
use serde::{Deserialize, Serialize};
use std::{
    fmt::Debug,
    hash::Hash,
    ops::Range,
    path::{Path, PathBuf},
    sync::Arc,
};
use strum::{EnumDiscriminants, EnumString, IntoStaticStr};
use tokio::sync::{Mutex, RwLock};
use tower_lsp::jsonrpc;

use crate::{
    database::Database,
    features::{LanguageFeatures, Reference},
    semantic_tokens::{SemanticTokenModifier, SemanticTokenType, SemanticTokensBuilder},
    server::Server,
    utils::{
        lsp_position_to_offset_raw, module_from_package_root_and_url, module_to_url,
        range_to_lsp_range_raw, LspPositionConversion,
    },
};

#[derive(Debug, Eq, PartialEq, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ViewIrParams {
    pub uri: Url,
}

impl Server {
    pub async fn candy_view_ir(&self, params: ViewIrParams) -> jsonrpc::Result<String> {
        let project_directory = {
            let state = self.state.read().await;
            state.require_running().project_directory.to_owned()
        };
        let config = IrConfig::decode(&params.uri, project_directory);

        let state = self.state.read().await;
        let features = state.require_features();

        features.ir.open(&self.db, config, params.uri.clone()).await;

        let open_irs = features.ir.open_irs.read().await;
        Ok(open_irs.get(&params.uri).unwrap().ir.text.to_owned())
    }
}

#[derive(Debug, Default)]
pub struct IrFeatures {
    open_irs: Arc<RwLock<FxHashMap<Url, OpenIr>>>,
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

    async fn ensure_is_open(&self, db: &Mutex<Database>, config: IrConfig) {
        let uri = Url::from(&config);
        {
            let open_irs = self.open_irs.read().await;
            if open_irs.contains_key(&uri) {
                return;
            }
        }

        self.open(db, config, uri).await;
    }
    async fn open(&self, db: &Mutex<Database>, config: IrConfig, uri: Url) {
        let db = db.lock().await;
        let open_ir = self.create(&db, config);
        let mut open_irs = self.open_irs.write().await;
        open_irs.insert(uri, open_ir);
    }
    fn create(&self, db: &Database, config: IrConfig) -> OpenIr {
        let ir = match &config.ir {
            Ir::Rcst => RichIr::for_rcst(&config.module, &db.rcst(config.module.clone())),
            Ir::Ast => db
                .ast(config.module.clone())
                .map(|(asts, _)| RichIr::for_ast(&config.module, &asts)),
            Ir::Hir => db
                .hir(config.module.clone())
                .map(|(body, _)| RichIr::for_hir(&config.module, &body)),
            Ir::Mir(tracing_config) => db
                .mir(config.module.clone(), tracing_config.to_owned())
                .map(|mir| RichIr::for_mir(&config.module, &mir, tracing_config)),
            Ir::OptimizedMir(tracing_config) => db
                .mir_with_obvious_optimized(config.module.clone(), tracing_config.to_owned())
                .map(|mir| RichIr::for_optimized_mir(&config.module, &mir, tracing_config)),
            Ir::Lir(tracing_config) => db
                .lir(config.module.clone(), tracing_config.to_owned())
                .map(|lir| RichIr::for_lir(&config.module, &lir, tracing_config)),
        };
        let ir = ir.unwrap_or_else(|| {
            let mut builder = RichIrBuilder::default();
            builder.push(
                format!("# Module does not exist: {}", config.module.to_rich_ir()),
                TokenType::Comment,
                EnumSet::empty(),
            );
            builder.finish()
        });

        let line_start_offsets = line_start_offsets_raw(&ir.text);
        OpenIr {
            config,
            ir,
            line_start_offsets,
        }
    }
}

#[derive(Debug)]
struct OpenIr {
    config: IrConfig,
    ir: RichIr,
    line_start_offsets: Vec<Offset>,
}
#[derive(Clone, Debug)]
struct IrConfig {
    module: Module,
    ir: Ir,
}
impl IrConfig {
    fn decode(uri: &Url, package_root: PathBuf) -> Self {
        let (path, ir) = uri.path().rsplit_once('.').unwrap();
        let details = urlencoding::decode(uri.fragment().unwrap()).unwrap();
        let mut details: serde_json::Map<String, serde_json::Value> =
            serde_json::from_str(&details).unwrap();

        let original_scheme = details.get("scheme").unwrap().as_str().unwrap();
        let original_uri = format!("{original_scheme}:{path}").parse().unwrap();

        let module_kind = match details.get("moduleKind").unwrap().as_str().unwrap() {
            "code" => ModuleKind::Code,
            "asset" => ModuleKind::Asset,
            module_kind => panic!("Unknown module kind: `{module_kind}`"),
        };

        let tracing_config = details
            .remove("tracingConfig")
            .map(|it| serde_json::from_value(it).unwrap());

        let ir = IrDiscriminants::try_from(ir).unwrap_or_else(|_| panic!("Unsupported IR: {ir}"));
        let ir = match ir {
            IrDiscriminants::Rcst => Ir::Rcst,
            IrDiscriminants::Ast => Ir::Ast,
            IrDiscriminants::Hir => Ir::Hir,
            IrDiscriminants::Mir => Ir::Mir(tracing_config.unwrap()),
            IrDiscriminants::OptimizedMir => Ir::OptimizedMir(tracing_config.unwrap()),
            IrDiscriminants::Lir => Ir::Lir(tracing_config.unwrap()),
        };

        IrConfig {
            module: module_from_package_root_and_url(package_root, &original_uri, module_kind)
                .unwrap(),
            ir,
        }
    }
}
impl From<&IrConfig> for Url {
    fn from(config: &IrConfig) -> Self {
        let ir: &'static str = IrDiscriminants::from(&config.ir).into();
        let original_url = module_to_url(&config.module).unwrap();

        let mut details = serde_json::Map::new();
        details.insert("scheme".to_string(), original_url.scheme().into());
        match &config.ir {
            Ir::Mir(tracing_config)
            | Ir::OptimizedMir(tracing_config)
            | Ir::Lir(tracing_config) => {
                details.insert(
                    "tracingConfig".to_string(),
                    serde_json::to_value(tracing_config).unwrap(),
                );
            }
            _ => {}
        }

        Url::parse(
            format!(
                "candy-ir:{}.{ir}#{}",
                original_url.path(),
                urlencoding::encode(serde_json::to_string(&details).unwrap().as_str()),
            )
            .as_str(),
        )
        .unwrap()
    }
}

#[derive(Clone, Debug, EnumDiscriminants, Eq, Hash, PartialEq)]
#[strum_discriminants(
    derive(EnumString, Hash, IntoStaticStr),
    strum(serialize_all = "camelCase")
)]
pub enum Ir {
    Rcst,
    Ast,
    Hir,
    Mir(TracingConfig),
    OptimizedMir(TracingConfig),
    Lir(TracingConfig),
}
impl Ir {
    fn tracing_config(&self) -> Option<&TracingConfig> {
        match self {
            Ir::Rcst | Ir::Ast | Ir::Hir => None,
            Ir::Mir(tracing_config)
            | Ir::OptimizedMir(tracing_config)
            | Ir::Lir(tracing_config) => Some(tracing_config),
        }
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
        db: &Mutex<Database>,
        _project_directory: &Path,
        uri: Url,
        position: lsp_types::Position,
    ) -> Option<LocationLink> {
        let (origin_selection_range, key, config) = {
            let open_irs = self.open_irs.read().await;
            let open_ir = open_irs.get(&uri).unwrap();
            let offset = open_ir.lsp_position_to_offset(position);

            let (key, result) = open_ir.find_references_entry(offset)?;

            let origin_selection_range = result
                .references
                .iter()
                .find(|it| it.contains(&offset))
                .unwrap_or_else(|| result.definition.as_ref().unwrap());
            let origin_selection_range = open_ir.range_to_lsp_range(origin_selection_range);

            if let Some(definition) = &result.definition {
                let target_range = open_ir.range_to_lsp_range(definition);
                return Some(LocationLink {
                    origin_selection_range: Some(origin_selection_range),
                    target_uri: uri,
                    target_range,
                    target_selection_range: target_range,
                });
            }

            (
                origin_selection_range,
                key.to_owned(),
                open_ir.config.to_owned(),
            )
        };

        let find_in_other_ir = async move |config: IrConfig, key: &ReferenceKey| {
            let uri = Url::from(&config);
            self.ensure_is_open(db, config).await;

            let rich_irs = self.open_irs.read().await;
            let other_ir = rich_irs.get(&uri).unwrap();
            let result = other_ir.ir.references.get(key).unwrap();
            let target_range = other_ir.range_to_lsp_range(result.definition.as_ref().unwrap());

            (uri, target_range)
        };

        let (uri, target_range) = match &key {
            ReferenceKey::Int(_)
            | ReferenceKey::Text(_)
            | ReferenceKey::Symbol(_)
            | ReferenceKey::BuiltinFunction(_) => {
                // These don't have a definition in Candy source code.
                return None;
            }
            ReferenceKey::Module(module) => {
                (module_to_url(module).unwrap(), lsp_types::Range::default())
            }
            ReferenceKey::ModuleWithSpan(module, span) => {
                let db = db.lock().await;
                let range = db.range_to_lsp_range(module.to_owned(), span.to_owned());
                (module_to_url(module).unwrap(), range)
            }
            ReferenceKey::HirId(id) => {
                let config = IrConfig {
                    module: id.module.to_owned(),
                    ir: Ir::Hir,
                };
                find_in_other_ir(config, &key).await
            }
            ReferenceKey::MirId(_) => {
                let config = IrConfig {
                    module: config.module.to_owned(),
                    ir: Ir::Mir(
                        config
                            .ir
                            .tracing_config()
                            .map(|it| it.to_owned())
                            .unwrap_or_else(TracingConfig::off),
                    ),
                };
                find_in_other_ir(config, &key).await
            }
        };
        Some(LocationLink {
            origin_selection_range: Some(origin_selection_range),
            target_uri: uri,
            target_range,
            target_selection_range: target_range,
        })
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
        only_in_same_document: bool,
        include_declaration: bool,
    ) -> FxHashMap<Url, Vec<Reference>> {
        let open_irs = self.open_irs.read().await;
        let open_ir = open_irs.get(&uri).unwrap();

        let offset = open_ir.lsp_position_to_offset(position);
        let Some((reference_key, _)) = open_ir.find_references_entry(offset) else { return FxHashMap::default(); };
        if only_in_same_document {
            FxHashMap::from_iter([(
                uri,
                open_ir.find_references(reference_key, include_declaration),
            )])
        } else {
            open_irs
                .iter()
                .map(|(uri, ir)| {
                    (
                        uri.to_owned(),
                        ir.find_references(reference_key, include_declaration),
                    )
                })
                .filter(|(_, references)| !references.is_empty())
                .collect()
        }
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
                    // TODO: Customize collapsed text
                    collapsed_text: None,
                }
            })
            .collect()
    }

    fn find_references(
        &self,
        reference_key: &ReferenceKey,
        include_declaration: bool,
    ) -> Vec<Reference> {
        let Some(result) = self.ir.references.get(reference_key) else { return vec![]; };

        let mut references = vec![];
        if include_declaration && let Some(definition) = &result.definition {
            references.push(Reference {
                range: self.range_to_lsp_range(definition),
                is_write: true,
            })
        }
        for reference in &result.references {
            references.push(Reference {
                range: self.range_to_lsp_range(reference),
                is_write: true,
            })
        }
        references
    }
    fn find_references_entry(
        &self,
        offset: Offset,
    ) -> Option<(&ReferenceKey, &ReferenceCollection)> {
        self.ir.references.iter().find(|(_, value)| {
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
            TokenType::Comment => SemanticTokenType::Comment,
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
