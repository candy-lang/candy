use crate::{
    compiler::{hir_to_lir::HirToLir, lir::Lir},
    database::Database,
    module::{Module, ModuleDb},
};

pub trait UseProvider {
    fn use_asset_module(&self, module: Module) -> Result<Vec<u8>, String>;
    fn use_code_module(&self, module: Module) -> Option<Lir>;
}

pub struct DbUseProvider<'a> {
    pub db: &'a Database,
}
impl<'a> UseProvider for DbUseProvider<'a> {
    fn use_asset_module(&self, module: Module) -> Result<Vec<u8>, String> {
        self.db
            .get_module_content(module.clone())
            .map(|bytes| (*bytes).clone())
            .ok_or_else(|| format!("Couldn't import file '{}'.", module))
    }

    fn use_code_module(&self, module: Module) -> Option<Lir> {
        self.db.lir(module).map(|lir| (*lir).clone())
    }
}
