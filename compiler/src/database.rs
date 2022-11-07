use crate::{
    compiler::{
        ast_to_hir::AstToHirStorage, cst::CstDbStorage, cst_to_ast::CstToAstStorage,
        hir::HirDbStorage, hir_to_mir::HirToMirStorage, mir_to_lir::MirToLirStorage,
        optimize::OptimizeMirStorage, rcst_to_cst::RcstToCstStorage,
        string_to_rcst::StringToRcstStorage,
    },
    language_server::{
        folding_range::FoldingRangeDbStorage, references::ReferencesDbStorage,
        semantic_tokens::SemanticTokenDbStorage, utils::LspPositionConversionStorage,
    },
    module::{GetOpenModuleContentQuery, Module, ModuleDbStorage, ModuleWatcher},
};
use std::collections::HashMap;
use tracing::warn;

#[salsa::database(
    AstToHirStorage,
    CstDbStorage,
    CstToAstStorage,
    FoldingRangeDbStorage,
    HirDbStorage,
    HirToMirStorage,
    LspPositionConversionStorage,
    MirToLirStorage,
    ModuleDbStorage,
    OptimizeMirStorage,
    RcstToCstStorage,
    ReferencesDbStorage,
    SemanticTokenDbStorage,
    StringToRcstStorage
)]
#[derive(Default)]
pub struct Database {
    storage: salsa::Storage<Self>,
    pub open_modules: HashMap<Module, Vec<u8>>,
}
impl salsa::Database for Database {}

impl Database {
    pub fn did_open_module(&mut self, module: &Module, content: Vec<u8>) {
        let old_value = self.open_modules.insert(module.clone(), content);
        if old_value.is_some() {
            warn!("Module {module} was opened, but it was already open.");
        }

        GetOpenModuleContentQuery.in_db_mut(self).invalidate(module);
    }
    pub fn did_change_module(&mut self, module: &Module, content: Vec<u8>) {
        let old_value = self.open_modules.insert(module.to_owned(), content);
        if old_value.is_none() {
            warn!("Module {module} was changed, but it wasn't open before.");
        }

        GetOpenModuleContentQuery.in_db_mut(self).invalidate(module);
    }
    pub fn did_close_module(&mut self, module: &Module) {
        let old_value = self.open_modules.remove(module);
        if old_value.is_none() {
            warn!("Module {module} was closed, but it wasn't open before.");
        }

        GetOpenModuleContentQuery.in_db_mut(self).invalidate(module);
    }
}
impl ModuleWatcher for Database {
    fn get_open_module_raw(&self, module: &Module) -> Option<Vec<u8>> {
        self.open_modules.get(module).cloned()
    }
}
