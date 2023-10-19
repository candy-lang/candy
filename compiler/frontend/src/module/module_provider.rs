use super::{module::Module, package::PackagesPath};
use rustc_hash::FxHashMap;
use std::{fs, io, sync::Arc};
use tracing::error;

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
    modules: FxHashMap<Module, Arc<Vec<u8>>>,
}
impl InMemoryModuleProvider {
    // It's exported in `lib.rs`, but the linter still complains about it.
    #[allow(dead_code)]
    #[must_use]
    pub fn for_modules<S: AsRef<str>>(modules: FxHashMap<Module, S>) -> Self {
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
        self.add(module, content.as_ref().as_bytes().to_vec());
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

pub struct FileSystemModuleProvider {
    pub packages_path: PackagesPath,
}
impl ModuleProvider for FileSystemModuleProvider {
    fn get_content(&self, module: &Module) -> Option<Arc<Vec<u8>>> {
        let paths = module.to_possible_paths(&self.packages_path).unwrap_or_else(|| {
            panic!(
                "Tried to get content of anonymous module {module} that is not cached by the language server.",
            )
        });
        for path in paths {
            match fs::read(path.clone()) {
                Ok(content) => return Some(Arc::new(content)),
                Err(error)
                    if matches!(
                        error.kind(),
                        io::ErrorKind::NotFound | io::ErrorKind::NotADirectory,
                    ) => {}
                Err(error) => error!("Unexpected error when reading file {path:?}: {error:?}"),
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
    pub const fn new(overlay: O, fallback: F) -> Self {
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
