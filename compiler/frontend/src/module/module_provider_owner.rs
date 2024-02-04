use super::{InMemoryModuleProvider, Module, ModuleProvider};

pub trait ModuleProviderOwner {
    #[must_use]
    fn get_module_provider(&self) -> &dyn ModuleProvider;
}

pub trait MutableModuleProviderOwner: ModuleProviderOwner {
    #[must_use]
    fn get_in_memory_module_provider(&mut self) -> &mut InMemoryModuleProvider;
    fn invalidate_module(&mut self, module: &Module);

    fn did_open_module(&mut self, module: &Module, content: Vec<u8>) {
        self.get_in_memory_module_provider().add(module, content);
        self.invalidate_module(module);
    }
    fn did_change_module(&mut self, module: &Module, content: Vec<u8>) {
        self.get_in_memory_module_provider().add(module, content);
        self.invalidate_module(module);
    }
    fn did_close_module(&mut self, module: &Module) {
        self.get_in_memory_module_provider().remove(module);
        self.invalidate_module(module);
    }
    #[must_use]
    fn get_open_modules(&mut self) -> Vec<Module> {
        self.get_in_memory_module_provider()
            .get_all_modules()
            .cloned()
            .collect()
    }
}

#[cfg(test)]
mod test {
    use std::path::PathBuf;

    use super::*;
    use crate::{
        ast::AstDbStorage,
        ast_to_hir::AstToHirStorage,
        cst::{CstDbStorage, CstKind},
        cst_to_ast::CstToAstStorage,
        hir::HirDbStorage,
        hir_to_mir::HirToMirStorage,
        mir_optimize::OptimizeMirStorage,
        module::{GetModuleContentQuery, ModuleDb, ModuleDbStorage, ModuleKind, Package},
        position::PositionConversionStorage,
        rcst_to_cst::RcstToCstStorage,
        string_to_rcst::{StringToRcst, StringToRcstStorage},
    };

    #[salsa::database(
        AstDbStorage,
        AstToHirStorage,
        CstDbStorage,
        CstToAstStorage,
        HirDbStorage,
        HirToMirStorage,
        ModuleDbStorage,
        OptimizeMirStorage,
        PositionConversionStorage,
        RcstToCstStorage,
        StringToRcstStorage
    )]
    #[derive(Default)]
    pub struct Database {
        storage: salsa::Storage<Self>,
        module_provider: InMemoryModuleProvider,
    }
    impl salsa::Database for Database {}
    impl ModuleProviderOwner for Database {
        fn get_module_provider(&self) -> &dyn ModuleProvider {
            &self.module_provider
        }
    }
    impl MutableModuleProviderOwner for Database {
        fn get_in_memory_module_provider(&mut self) -> &mut InMemoryModuleProvider {
            &mut self.module_provider
        }
        fn invalidate_module(&mut self, module: &Module) {
            GetModuleContentQuery.in_db_mut(self).invalidate(module);
        }
    }

    #[test]
    fn on_demand_module_content_works() {
        let mut db = Database::default();
        let module = Module::new(
            Package::User(PathBuf::from("/non/existent")),
            vec!["foo".to_string()],
            ModuleKind::Code,
        );

        db.did_open_module(&module, b"123".to_vec());
        assert_eq!(
            db.get_module_content_as_string(module.clone())
                .unwrap()
                .as_ref(),
            "123",
        );
        assert_eq!(
            db.rcst(module.clone()).unwrap().as_ref().clone(),
            vec![CstKind::Int {
                radix_prefix: None,
                value: 123u8.into(),
                string: "123".to_string(),
            }
            .into()],
        );

        db.did_change_module(&module, b"456".to_vec());
        assert_eq!(
            db.get_module_content_as_string(module.clone())
                .unwrap()
                .as_ref()
                .clone(),
            "456",
        );
        assert_eq!(
            db.rcst(module.clone()).unwrap().as_ref().clone(),
            vec![CstKind::Int {
                radix_prefix: None,
                value: 456u16.into(),
                string: "456".to_string(),
            }
            .into()],
        );

        db.did_close_module(&module);
    }
}
