use std::{collections::HashMap, fs, sync::Arc};

use salsa::query_group;
use tracing::error;

pub use self::{
    module::{Module, ModuleKind},
    package::Package,
    use_path::UsePath,
};

mod module;
mod package;
mod use_path;

pub trait ModuleProviderOwner {
    fn get_module_provider(&self) -> &dyn ModuleProvider;
}

pub trait ModuleProvider {
    fn get_content(&self, module: &Module) -> Option<Arc<Vec<u8>>>;
}

impl<M: ModuleProvider + ?Sized> ModuleProvider for Box<M> {
    fn get_content(&self, module: &Module) -> Option<Arc<Vec<u8>>> {
        self.as_ref().get_content(module)
    }
}

#[derive(Default)]
pub struct InMemoryModuleProvider {
    modules: HashMap<Module, Arc<Vec<u8>>>,
}
impl InMemoryModuleProvider {
    // It's exported in `lib.rs`, but the linter still complains about it.
    #[allow(dead_code)]
    pub fn for_modules<S: AsRef<str>>(modules: HashMap<Module, S>) -> Self {
        let mut result = Self::default();
        for (module, content) in modules {
            result.add_str(&module, content);
        }
        result
    }

    pub fn add(&mut self, module: &Module, content: Vec<u8>) {
        self.modules.insert(module.clone(), Arc::new(content));
    }
    pub fn add_str<S: AsRef<str>>(&mut self, module: &Module, content: S) {
        self.add(module, content.as_ref().as_bytes().to_vec())
    }
    pub fn remove(&mut self, module: &Module) {
        self.modules.remove(module);
    }

    pub fn get_all_modules(&self) -> impl Iterator<Item = &Module> {
        self.modules.keys()
    }
}
impl ModuleProvider for InMemoryModuleProvider {
    fn get_content(&self, module: &Module) -> Option<Arc<Vec<u8>>> {
        self.modules.get(module).cloned()
    }
}

#[derive(Default)]
pub struct FileSystemModuleProvider {}
impl ModuleProvider for FileSystemModuleProvider {
    fn get_content(&self, module: &Module) -> Option<Arc<Vec<u8>>> {
        let paths = module.to_possible_paths().unwrap_or_else(|| {
            panic!(
                "Tried to get content of anonymous module {module} that is not cached by the language server."
            )
        });
        for path in paths {
            match fs::read(path.clone()) {
                Ok(content) => return Some(Arc::new(content)),
                Err(error) if matches!(error.kind(), std::io::ErrorKind::NotFound) => {}
                Err(_) => error!("Unexpected error when reading file {path:?}."),
            }
        }
        None
    }
}

pub struct OverlayModuleProvider<O: ModuleProvider, F: ModuleProvider> {
    pub overlay: O,
    pub fallback: F,
}
impl<O: ModuleProvider, F: ModuleProvider> OverlayModuleProvider<O, F> {
    pub fn new(overlay: O, fallback: F) -> Self {
        Self { overlay, fallback }
    }
}
impl<O: ModuleProvider, F: ModuleProvider> ModuleProvider for OverlayModuleProvider<O, F> {
    fn get_content(&self, module: &Module) -> Option<Arc<Vec<u8>>> {
        self.overlay
            .get_content(module)
            .or_else(|| self.fallback.get_content(module))
    }
}

#[query_group(ModuleDbStorage)]
pub trait ModuleDb: ModuleProviderOwner {
    fn get_module_content_as_string(&self, module: Module) -> Option<Arc<String>>;
    fn get_module_content(&self, module: Module) -> Option<Arc<Vec<u8>>>;
}

fn get_module_content_as_string(db: &dyn ModuleDb, module: Module) -> Option<Arc<String>> {
    let content = get_module_content(db, module)?;
    String::from_utf8((*content).clone()).ok().map(Arc::new)
}

fn get_module_content(db: &dyn ModuleDb, module: Module) -> Option<Arc<Vec<u8>>> {
    // The following line of code shouldn't be neccessary, but it is.
    //
    // We call `GetModuleContentQuery.in_db_mut(self).invalidate(module);`
    // in `Database.did_open_module(…)`, `.did_change_module(…)`, and
    // `.did_close_module(…)` which correctly forces Salsa to re-run this query
    // function the next time this module is used. However, even though the
    // return value changes, Salsa doesn't record an updated `changed_at` value
    // in its internal `ActiveQuery` struct. `Runtime.report_untracked_read()`
    // manually sets this to the current revision.
    db.salsa_runtime().report_untracked_read();

    db.get_module_provider().get_content(&module)
}

#[cfg(test)]
mod test {
    use std::path::PathBuf;

    use super::*;
    use crate::{
        compiler::{rcst::Rcst, string_to_rcst::StringToRcst},
        database::Database,
    };

    #[test]
    fn on_demand_module_content_works() {
        let mut db = Database::default();
        let module = Module {
            package: Package::User(PathBuf::from("/non/existent")),
            path: vec!["foo".to_string()],
            kind: ModuleKind::Code,
        };

        db.did_open_module(&module, "123".to_string().into_bytes());
        assert_eq!(
            db.get_module_content_as_string(module.clone())
                .unwrap()
                .as_ref(),
            "123",
        );
        assert_eq!(
            db.rcst(module.clone()).unwrap().as_ref().to_owned(),
            vec![Rcst::Int {
                value: 123u8.into(),
                string: "123".to_string(),
            }],
        );

        db.did_change_module(&module, "456".to_string().into_bytes());
        assert_eq!(
            db.get_module_content_as_string(module.clone())
                .unwrap()
                .as_ref()
                .to_owned(),
            "456",
        );
        assert_eq!(
            db.rcst(module.clone()).unwrap().as_ref().to_owned(),
            vec![Rcst::Int {
                value: 456u16.into(),
                string: "456".to_string(),
            }],
        );

        db.did_close_module(&module);
    }
}
