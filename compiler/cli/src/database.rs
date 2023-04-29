use candy_frontend::{
    ast::AstDbStorage,
    ast_to_hir::AstToHirStorage,
    cst::CstDbStorage,
    cst_to_ast::CstToAstStorage,
    hir::HirDbStorage,
    hir_to_mir::HirToMirStorage,
    mir_optimize::OptimizeMirStorage,
    module::{
        FileSystemModuleProvider, GetModuleContentQuery, InMemoryModuleProvider, Module,
        ModuleDbStorage, ModuleProvider, ModuleProviderOwner, MutableModuleProviderOwner,
        OverlayModuleProvider, PackagesPath,
    },
    position::PositionConversionStorage,
    rcst_to_cst::RcstToCstStorage,
    string_to_rcst::StringToRcstStorage,
};
use candy_vm::mir_to_lir::MirToLirStorage;

#[salsa::database(
    AstDbStorage,
    AstToHirStorage,
    CstDbStorage,
    CstToAstStorage,
    HirDbStorage,
    HirToMirStorage,
    MirToLirStorage,
    ModuleDbStorage,
    OptimizeMirStorage,
    PositionConversionStorage,
    RcstToCstStorage,
    StringToRcstStorage
)]
pub struct Database {
    storage: salsa::Storage<Self>,
    module_provider: OverlayModuleProvider<InMemoryModuleProvider, Box<dyn ModuleProvider + Send>>,
}
impl salsa::Database for Database {}

impl Database {
    pub fn new_with_file_system_module_provider(packages_path: PackagesPath) -> Self {
        Self::new(Box::new(FileSystemModuleProvider { packages_path }))
    }
    pub fn new(module_provider: Box<dyn ModuleProvider + Send>) -> Self {
        Self {
            storage: salsa::Storage::default(),
            module_provider: OverlayModuleProvider::new(
                InMemoryModuleProvider::default(),
                module_provider,
            ),
        }
    }
}

impl ModuleProviderOwner for Database {
    fn get_module_provider(&self) -> &dyn ModuleProvider {
        &self.module_provider
    }
}
impl MutableModuleProviderOwner for Database {
    fn get_in_memory_module_provider(&mut self) -> &mut InMemoryModuleProvider {
        &mut self.module_provider.overlay
    }
    fn invalidate_module(&mut self, module: &Module) {
        GetModuleContentQuery.in_db_mut(self).invalidate(module);
    }
}
