use async_trait::async_trait;
#[cfg(feature = "inkwell")]
use candy_backend_inkwell::LlvmIrDb;
use candy_frontend::{
    ast_to_hir::{AstToHir, HirResult},
    cst_to_ast::{AstResult, CstToAst},
    hir_to_mir::{ExecutionTarget, HirToMir, MirResult},
    lir_optimize::OptimizeLir,
    mir_optimize::{OptimizeMir, OptimizedMirResult},
    mir_to_lir::{LirResult, MirToLir},
    module::{Module, ModuleKind, PackagesPath},
    position::{line_start_offsets_raw, Offset},
    rich_ir::{
        ReferenceCollection, ReferenceKey, RichIr, RichIrBuilder, ToRichIr, TokenModifier,
        TokenType,
    },
    string_to_rcst::{ModuleError, RcstResult, StringToRcst},
    TracingConfig,
};
use rustc_hash::FxHashMap;
use serde::{Deserialize, Serialize};
use std::{fmt::Debug, hash::Hash, ops::Range, sync::Arc};
use strum::{EnumDiscriminants, EnumString, IntoStaticStr};
use tokio::sync::{Mutex, RwLock};
use tower_lsp::jsonrpc;

use crate::{
    database::Database,
    features::{LanguageFeatures, Reference},
    semantic_tokens::{SemanticTokenModifier, SemanticTokenType, SemanticTokensBuilder},
    server::Server,
    utils::{
        lsp_position_to_offset_raw, module_from_url, module_to_url, range_to_lsp_range_raw,
        LspPositionConversion,
    },
};
use enumset::EnumSet;
use extension_trait::extension_trait;
use lsp_types::{
    notification::Notification, FoldingRange, FoldingRangeKind, LocationLink, SemanticToken,
};
use url::Url;

#[derive(Debug, Eq, PartialEq, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ViewIrParams {
    pub uri: Url,
}

impl Server {
    pub async fn candy_view_ir(&self, params: ViewIrParams) -> jsonrpc::Result<String> {
        let state = self.state.read().await;
        let config = IrConfig::decode(&params.uri, &state.require_running().packages_path);
        let features = state.require_features();

        features.ir.open(&self.db, config, params.uri.clone()).await;

        let open_irs = features.ir.open_irs.read().await;
        Ok(open_irs.get(&params.uri).unwrap().ir.text.clone())
    }
}

#[derive(Debug, Default)]
pub struct IrFeatures {
    /// If a tab gets closed just after a request, the IR might be missing in
    /// this map already while we are still working on requests for it. In that
    /// case, we return empty responses.
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
        let packages_path = {
            let db = db.lock().await;
            db.packages_path.clone()
        };
        let uri = Url::from_config(&config, &packages_path);
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
        let open_ir = Self::create(&db, config);
        let mut open_irs = self.open_irs.write().await;
        open_irs.insert(uri, open_ir);
    }
    fn create(db: &Database, config: IrConfig) -> OpenIr {
        let ir = match &config.ir {
            Ir::Rcst => Self::rich_ir_for_rcst(&config.module, db.rcst(config.module.clone())),
            Ir::Ast => Self::rich_ir_for_ast(&config.module, db.ast(config.module.clone())),
            Ir::Hir => Self::rich_ir_for_hir(&config.module, db.hir(config.module.clone())),
            Ir::Mir(tracing_config) => Self::rich_ir_for_mir(
                &config.module,
                db.mir(
                    ExecutionTarget::Module(config.module.clone()),
                    *tracing_config,
                ),
                *tracing_config,
            ),
            Ir::OptimizedMir(tracing_config) => Self::rich_ir_for_optimized_mir(
                &config.module,
                db.optimized_mir(
                    ExecutionTarget::Module(config.module.clone()),
                    *tracing_config,
                ),
                *tracing_config,
            ),
            Ir::Lir(tracing_config) => Self::rich_ir_for_lir(
                &config.module,
                &db.lir(
                    ExecutionTarget::Module(config.module.clone()),
                    *tracing_config,
                ),
                *tracing_config,
            ),
            Ir::OptimizedLir(tracing_config) => Self::rich_ir_for_optimized_lir(
                &config.module,
                db.optimized_lir(
                    ExecutionTarget::Module(config.module.clone()),
                    *tracing_config,
                ),
                *tracing_config,
            ),
            Ir::VmByteCode(tracing_config) => Self::rich_ir_for_vm_byte_code(
                &config.module,
                &candy_vm::lir_to_byte_code::compile_byte_code(
                    db,
                    ExecutionTarget::Module(config.module.clone()),
                    *tracing_config,
                )
                .0,
                *tracing_config,
            ),
            #[cfg(feature = "inkwell")]
            Ir::LlvmIr => db
                .llvm_ir(ExecutionTarget::Module(config.module.clone()))
                .unwrap(),
        };

        let line_start_offsets = line_start_offsets_raw(&ir.text);
        OpenIr {
            config,
            ir,
            line_start_offsets,
        }
    }
    fn rich_ir_for_rcst(module: &Module, rcst: RcstResult) -> RichIr {
        Self::rich_ir_for("RCST", module, None, |builder| match rcst {
            Ok(rcst) => rcst.build_rich_ir(builder),
            Err(error) => Self::build_rich_ir_for_module_error(builder, module, error),
        })
    }
    fn rich_ir_for_ast(module: &Module, asts: AstResult) -> RichIr {
        Self::rich_ir_for("AST", module, None, |builder| match asts {
            Ok((asts, _)) => asts.build_rich_ir(builder),
            Err(error) => Self::build_rich_ir_for_module_error(builder, module, error),
        })
    }
    fn rich_ir_for_hir(module: &Module, hir: HirResult) -> RichIr {
        Self::rich_ir_for("HIR", module, None, |builder| match hir {
            Ok((hir, _)) => hir.build_rich_ir(builder),
            Err(error) => Self::build_rich_ir_for_module_error(builder, module, error),
        })
    }
    fn rich_ir_for_mir(module: &Module, mir: MirResult, tracing_config: TracingConfig) -> RichIr {
        Self::rich_ir_for("MIR", module, tracing_config, |builder| match mir {
            Ok((mir, _)) => mir.build_rich_ir(builder),
            Err(error) => Self::build_rich_ir_for_module_error(builder, module, error),
        })
    }
    fn rich_ir_for_optimized_mir(
        module: &Module,
        mir: OptimizedMirResult,
        tracing_config: TracingConfig,
    ) -> RichIr {
        Self::rich_ir_for(
            "Optimized MIR",
            module,
            tracing_config,
            |builder| match mir {
                Ok((mir, _)) => mir.build_rich_ir(builder),
                Err(error) => Self::build_rich_ir_for_module_error(builder, module, error),
            },
        )
    }
    fn rich_ir_for_lir(module: &Module, lir: &LirResult, tracing_config: TracingConfig) -> RichIr {
        Self::rich_ir_for("LIR", module, tracing_config, |builder| match lir {
            Ok((lir, _)) => lir.build_rich_ir(builder),
            Err(error) => Self::build_rich_ir_for_module_error(builder, module, *error),
        })
    }
    fn rich_ir_for_optimized_lir(
        module: &Module,
        lir: LirResult,
        tracing_config: TracingConfig,
    ) -> RichIr {
        Self::rich_ir_for(
            "Optimized LIR",
            module,
            tracing_config,
            |builder| match lir {
                Ok((lir, _)) => lir.build_rich_ir(builder),
                Err(error) => Self::build_rich_ir_for_module_error(builder, module, error),
            },
        )
    }
    fn rich_ir_for_vm_byte_code(
        module: &Module,
        byte_code: &candy_vm::byte_code::ByteCode,
        tracing_config: TracingConfig,
    ) -> RichIr {
        Self::rich_ir_for("VM Byte Code", module, tracing_config, |builder| {
            byte_code.build_rich_ir(builder);
        })
    }
    fn rich_ir_for(
        ir_name: &str,
        module: &Module,
        tracing_config: impl Into<Option<TracingConfig>>,
        build_rich_ir: impl FnOnce(&mut RichIrBuilder),
    ) -> RichIr {
        let mut builder = RichIrBuilder::default();
        builder.push_comment_line(format!("{ir_name} for module {module}"));
        if let Some(tracing_config) = tracing_config.into() {
            builder.push_tracing_config(tracing_config);
        }
        builder.push_newline();

        build_rich_ir(&mut builder);

        builder.finish(true)
    }
    fn build_rich_ir_for_module_error(
        builder: &mut RichIrBuilder,
        module: &Module,
        module_error: ModuleError,
    ) {
        match module_error {
            ModuleError::DoesNotExist => {
                builder.push(
                    format!("# Module {module} does not exist"),
                    TokenType::Comment,
                    EnumSet::empty(),
                );
            }
            ModuleError::InvalidUtf8 => {
                builder.push("# Invalid UTF-8", TokenType::Comment, EnumSet::empty());
            }
            ModuleError::IsNotCandy => {
                builder.push("# Is not Candy code", TokenType::Comment, EnumSet::empty());
            }
            ModuleError::IsToolingModule => {
                builder.push(
                    "# Is a tooling module",
                    TokenType::Comment,
                    EnumSet::empty(),
                );
            }
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
    fn decode(uri: &Url, packages_path: &PackagesPath) -> Self {
        let (path, ir) = uri
            .path()
            .rsplit_once('.')
            .expect("URI path is missing a file extension.");
        let details = urlencoding::decode(uri.fragment().expect("URI is missing a fragment"))
            .expect("URI Fragment is not URL-encoded.");
        let mut details: serde_json::Map<String, serde_json::Value> =
            serde_json::from_str(&details).expect("Decoded URI fragment is not a valid JSON map.");

        let original_scheme = details
            .get("scheme")
            .expect("Decoded URI fragment is missing the `scheme`.")
            .as_str()
            .expect("Decoded URI fragment's `scheme` is not a string.");
        let original_uri = format!("{original_scheme}:{path}")
            .parse()
            .expect("Recreated original URI is not a valid URI.");

        let module_kind = match details
            .get("moduleKind")
            .expect("Decoded URI fragment is missing the `moduleKind`.")
            .as_str()
            .expect("Decoded URI fragment's `moduleKind` is not a string.")
        {
            "code" => ModuleKind::Code,
            "asset" => ModuleKind::Asset,
            module_kind => panic!("Unknown module kind: `{module_kind}`"),
        };

        let tracing_config = details.remove("tracingConfig").map(|it| {
            serde_json::from_value(it)
                .expect("Decoded URI fragment's `tracingConfig` is not a valid JSON map.")
        });

        let ir = IrDiscriminants::try_from(ir).unwrap_or_else(|_| panic!("Unsupported IR: {ir}"));
        let ir = match ir {
            IrDiscriminants::Rcst => Ir::Rcst,
            IrDiscriminants::Ast => Ir::Ast,
            IrDiscriminants::Hir => Ir::Hir,
            IrDiscriminants::Mir => Ir::Mir(tracing_config.expect("Tracing config is missing.")),
            IrDiscriminants::OptimizedMir => {
                Ir::OptimizedMir(tracing_config.expect("Tracing config is missing."))
            }
            IrDiscriminants::Lir => Ir::Lir(tracing_config.expect("Tracing config is missing.")),
            IrDiscriminants::OptimizedLir => {
                Ir::OptimizedLir(tracing_config.expect("Tracing config is missing."))
            }
            IrDiscriminants::VmByteCode => {
                Ir::VmByteCode(tracing_config.expect("Tracing config is missing."))
            }
            #[cfg(feature = "inkwell")]
            IrDiscriminants::LlvmIr => Ir::LlvmIr,
        };

        Self {
            module: module_from_url(&original_uri, module_kind, packages_path).unwrap(),
            ir,
        }
    }
}

#[extension_trait]
impl UrlFromIrConfig for Url {
    fn from_config(config: &IrConfig, packages_path: &PackagesPath) -> Self {
        let ir: &'static str = IrDiscriminants::from(&config.ir).into();
        let original_url = module_to_url(&config.module, packages_path).unwrap();

        let mut details = serde_json::Map::new();
        details.insert("scheme".to_string(), original_url.scheme().into());
        details.insert(
            "moduleKind".to_string(),
            match config.module.kind() {
                ModuleKind::Code => "code".into(),
                ModuleKind::Asset => "asset".into(),
            },
        );
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

        Self::parse(
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
    OptimizedLir(TracingConfig),
    VmByteCode(TracingConfig),
    #[cfg(feature = "inkwell")]
    LlvmIr,
}
impl Ir {
    const fn tracing_config(&self) -> Option<TracingConfig> {
        match self {
            Self::Rcst | Self::Ast | Self::Hir => None,
            Self::Mir(tracing_config)
            | Self::OptimizedMir(tracing_config)
            | Self::Lir(tracing_config)
            | Self::OptimizedLir(tracing_config)
            | Self::VmByteCode(tracing_config) => Some(*tracing_config),
            #[cfg(feature = "inkwell")]
            Self::LlvmIr => None,
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
        uri: Url,
        position: lsp_types::Position,
    ) -> Option<LocationLink> {
        let (origin_selection_range, key, config) = {
            let open_irs = self.open_irs.read().await;
            let open_ir = open_irs.get(&uri)?;
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

            (origin_selection_range, key.clone(), open_ir.config.clone())
        };

        let packages_path = {
            let db = db.lock().await;
            db.packages_path.clone()
        };

        let packages_path_for_function = packages_path.clone();
        let find_in_other_ir = async move |config: IrConfig, key: &ReferenceKey| {
            let uri = Url::from_config(&config, &packages_path_for_function);
            self.ensure_is_open(db, config).await;

            let rich_irs = self.open_irs.read().await;
            let other_ir = rich_irs.get(&uri)?;
            // Some HIR IDs are synthetic: They are created during HIR to MIR
            // lowering.
            // TODO(JonasWanke): We could always use "$" to mark synthetic parts
            // in a HIR ID and then navigate to the prefix that's not synthetic.
            let result = other_ir.ir.references.get(key)?;
            let target_range = other_ir.range_to_lsp_range(result.definition.as_ref().unwrap());

            Some((uri, target_range))
        };

        let (uri, target_range) = match &key {
            ReferenceKey::Int(_)
            | ReferenceKey::Text(_)
            | ReferenceKey::Symbol(_)
            | ReferenceKey::BuiltinFunction(_) => {
                // These don't have a definition in Candy source code.
                return None;
            }
            ReferenceKey::Module(module) => (
                module_to_url(module, &packages_path).unwrap(),
                lsp_types::Range::default(),
            ),
            ReferenceKey::ModuleWithSpan(module, span) => {
                let db = db.lock().await;
                let range = db.range_to_lsp_range(module.clone(), span.clone());
                (module_to_url(module, &packages_path).unwrap(), range)
            }
            ReferenceKey::HirId(id) => {
                let config = IrConfig {
                    module: id.module.clone(),
                    ir: Ir::Hir,
                };
                find_in_other_ir(config, &key).await?
            }
            ReferenceKey::MirId(_) => {
                let config = IrConfig {
                    module: config.module.clone(),
                    ir: Ir::Mir(
                        config
                            .ir
                            .tracing_config()
                            .unwrap_or_else(TracingConfig::off),
                    ),
                };
                find_in_other_ir(config, &key).await?
            }
            ReferenceKey::LirId(_)
            | ReferenceKey::LirConstantId(_)
            | ReferenceKey::LirBodyId(_) => {
                let config = IrConfig {
                    module: config.module.clone(),
                    ir: Ir::Lir(
                        config
                            .ir
                            .tracing_config()
                            .unwrap_or_else(TracingConfig::off),
                    ),
                };
                find_in_other_ir(config, &key).await?
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
    async fn folding_ranges(&self, _db: &Mutex<Database>, uri: Url) -> Vec<FoldingRange> {
        let open_irs = self.open_irs.read().await;
        let Some(open_ir) = open_irs.get(&uri) else {
            return vec![];
        };
        open_ir.folding_ranges()
    }

    fn supports_references(&self) -> bool {
        true
    }
    async fn references(
        &self,
        _db: &Mutex<Database>,
        uri: Url,
        position: lsp_types::Position,
        only_in_same_document: bool,
        include_declaration: bool,
    ) -> FxHashMap<Url, Vec<Reference>> {
        let open_irs = self.open_irs.read().await;
        let Some(open_ir) = open_irs.get(&uri) else {
            return FxHashMap::default();
        };

        let offset = open_ir.lsp_position_to_offset(position);
        let Some((reference_key, _)) = open_ir.find_references_entry(offset) else {
            return FxHashMap::default();
        };
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
                        uri.clone(),
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
    async fn semantic_tokens(&self, _db: &Mutex<Database>, uri: Url) -> Vec<SemanticToken> {
        let open_irs = self.open_irs.read().await;
        let Some(open_ir) = open_irs.get(&uri) else {
            return vec![];
        };
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
        let Some(result) = self.ir.references.get(reference_key) else {
            return vec![];
        };

        let mut references = vec![];
        if include_declaration && let Some(definition) = &result.definition {
            references.push(Reference {
                range: self.range_to_lsp_range(definition),
                is_write: true,
            });
        }
        for reference in &result.references {
            references.push(Reference {
                range: self.range_to_lsp_range(reference),
                is_write: true,
            });
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
                .is_some_and(|it| it.contains(&offset))
                || value.references.iter().any(|it| it.contains(&offset))
        })
    }

    fn semantic_tokens(&self) -> Vec<SemanticToken> {
        let mut builder = SemanticTokensBuilder::new(&self.ir.text, &self.line_start_offsets);
        for annotation in &self.ir.annotations {
            let Some(token_type) = annotation.token_type else {
                continue;
            };
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
            Self::Module => SemanticTokenType::Module,
            Self::Parameter => SemanticTokenType::Parameter,
            Self::Variable => SemanticTokenType::Variable,
            Self::Function => SemanticTokenType::Function,
            Self::Comment => SemanticTokenType::Comment,
            Self::Symbol => SemanticTokenType::Symbol,
            Self::Text => SemanticTokenType::Text,
            Self::Int => SemanticTokenType::Int,
            Self::Address => SemanticTokenType::Address,
            Self::Constant => SemanticTokenType::Constant,
        }
    }
}

#[extension_trait]
impl TokenModifierToSemantic for TokenModifier {
    fn to_semantic(&self) -> SemanticTokenModifier {
        match self {
            Self::Builtin => SemanticTokenModifier::Builtin,
        }
    }
}
