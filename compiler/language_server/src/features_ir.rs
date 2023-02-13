use std::sync::Arc;

use async_trait::async_trait;
use candy_frontend::{
    cst_to_ast::CstToAst,
    module::Module,
    position::line_start_offsets_raw,
    rich_ir::{RichIr, ToRichIr, TokenType},
    string_to_rcst::{InvalidModuleError, StringToRcst},
};
use extension_trait::extension_trait;
use lsp_types::{SemanticToken, Url};
use rustc_hash::FxHashMap;
use serde::{Deserialize, Serialize};
use strum_macros::EnumIter;
use tokio::sync::RwLock;
use tower_lsp::jsonrpc;

use crate::{
    database::Database,
    features::LanguageFeatures,
    semantic_tokens::{SemanticTokenType, SemanticTokensBuilder},
    server::Server,
};

#[derive(Debug, EnumIter, PartialEq, Eq, Clone, Copy, Hash)]
pub enum Ir {
    Rcst,
    Ast,
}

#[derive(Debug, Eq, PartialEq, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ViewIrParams {
    pub uri: Url,
}

impl Server {
    pub async fn candy_view_rcst(&self, params: ViewIrParams) -> jsonrpc::Result<String> {
        self.candy_view_ir(params, Ir::Rcst, |db, module| match db.rcst(module) {
            Ok(rcst) => Some(rcst.to_rich_ir()),
            Err(InvalidModuleError::DoesNotExist) => None,
            Err(InvalidModuleError::InvalidUtf8) => Some("# Invalid UTF-8".to_rich_ir()),
            Err(InvalidModuleError::IsToolingModule) => Some("# Is a tooling module".to_rich_ir()),
        })
        .await
    }
    pub async fn candy_view_ast(&self, params: ViewIrParams) -> jsonrpc::Result<String> {
        self.candy_view_ir(params, Ir::Ast, |db, module| {
            db.ast(module).map(|(asts, _)| asts.to_rich_ir())
        })
        .await
    }
    async fn candy_view_ir<F>(
        &self,
        params: ViewIrParams,
        ir: Ir,
        get_ir: F,
    ) -> jsonrpc::Result<String>
    where
        F: FnOnce(&Database, Module) -> Option<RichIr>,
    {
        let module = self.code_module_from_url(params.uri).await;
        let open_irs = {
            let state = self.state.read().await;
            let features = state.require_features();
            let features = features.ir.get(&ir).unwrap();
            features.open_irs.clone()
        };
        let mut open_irs = open_irs.write().await;
        let db = self.db.lock().await;
        let ir = open_irs.entry(module.clone()).or_insert_with(|| {
            get_ir(&db, module).unwrap_or_else(|| "# Module does not exist".to_rich_ir())
        });
        Ok(ir.text.clone())
    }
}

pub struct IrFeatures {
    url_scheme: &'static str,
    open_irs: Arc<RwLock<FxHashMap<Module, RichIr>>>,
}
impl IrFeatures {
    pub fn new(ir: Ir) -> Self {
        let url_scheme = match ir {
            Ir::Rcst => "candy-rcst",
            Ir::Ast => "candy-ast",
        };
        Self {
            url_scheme,
            open_irs: Arc::default(),
        }
    }
}
#[async_trait]
impl LanguageFeatures for IrFeatures {
    fn language_id(&self) -> Option<String> {
        None
    }
    fn supported_url_schemes(&self) -> Vec<String> {
        vec![self.url_scheme.to_string()]
    }

    fn supports_semantic_tokens(&self) -> bool {
        true
    }
    async fn semantic_tokens(&self, _db: &Database, module: Module) -> Vec<SemanticToken> {
        let open_irs = self.open_irs.read().await;
        let ir = open_irs.get(&module).unwrap();
        ir.to_semantic_tokens()
    }
}

#[extension_trait]
impl RichIrExtension for RichIr {
    fn to_semantic_tokens(&self) -> Vec<SemanticToken> {
        let line_start_offsets = line_start_offsets_raw(&self.text);
        let mut builder = SemanticTokensBuilder::new(&self.text, &line_start_offsets);
        for annotation in &self.annotations {
            let Some(token_type) = annotation.token_type else { continue; };
            builder.add(annotation.range.clone(), token_type.to_semantic());
        }
        builder.finish()
    }
}

#[extension_trait]
impl TokenTypeToSemantic for TokenType {
    fn to_semantic(&self) -> SemanticTokenType {
        match self {
            TokenType::Int => SemanticTokenType::Int,
            TokenType::Symbol => SemanticTokenType::Symbol,
            TokenType::Text => SemanticTokenType::Text,
        }
    }
}
