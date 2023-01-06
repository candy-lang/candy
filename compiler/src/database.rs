use crate::{
    compiler::{
        ast::AstDbStorage, ast_to_hir::AstToHirStorage, cst::CstDbStorage,
        cst_to_ast::CstToAstStorage, hir::HirDbStorage, hir_to_mir::HirToMirStorage,
        mir_optimize::OptimizeMirStorage, mir_to_lir::MirToLirStorage,
        rcst_to_cst::RcstToCstStorage, string_to_rcst::StringToRcstStorage,
    },
    language_server::{
        folding_range::FoldingRangeDbStorage, references::ReferencesDbStorage,
        semantic_tokens::SemanticTokenDbStorage, utils::LspPositionConversionStorage,
    },
    module::{
        FileSystemModuleProvider, GetModuleContentQuery, InMemoryModuleProvider, Module,
        ModuleDbStorage, ModuleProvider, ModuleProviderOwner, OverlayModuleProvider,
    },
};

#[salsa::database(
    AstDbStorage,
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
pub struct Database {
    storage: salsa::Storage<Self>,
    module_provider: OverlayModuleProvider<InMemoryModuleProvider, Box<dyn ModuleProvider + Send>>,
}
impl salsa::Database for Database {}

impl Default for Database {
    fn default() -> Self {
        Self::new(Box::<FileSystemModuleProvider>::default())
    }
}

impl Database {
    pub fn new(module_provider: Box<dyn ModuleProvider + Send>) -> Self {
        Self {
            storage: salsa::Storage::default(),
            module_provider: OverlayModuleProvider::new(
                InMemoryModuleProvider::default(),
                module_provider,
            ),
        }
    }

    pub fn did_open_module(&mut self, module: &Module, content: Vec<u8>) {
        self.module_provider.overlay.add(module, content);
        GetModuleContentQuery.in_db_mut(self).invalidate(module);
    }
    pub fn did_change_module(&mut self, module: &Module, content: Vec<u8>) {
        self.module_provider.overlay.add(module, content);
        GetModuleContentQuery.in_db_mut(self).invalidate(module);
    }
    pub fn did_close_module(&mut self, module: &Module) {
        self.module_provider.overlay.remove(module);
        GetModuleContentQuery.in_db_mut(self).invalidate(module);
    }
    pub fn get_open_modules(&mut self) -> impl Iterator<Item = &Module> {
        self.module_provider.overlay.get_all_modules()
    }
}

impl ModuleProviderOwner for Database {
    fn get_module_provider(&self) -> &dyn ModuleProvider {
        &self.module_provider
    }
}
