use candy_frontend::module::{InMemoryModuleProvider, Module, ModuleKind, Package, PackagesPath};
use extension_trait::extension_trait;
use std::{
    fmt::{self, Debug, Display, Formatter},
    fs,
};
use walkdir::WalkDir;

/// The in-memory provider is heavily used during testing, benchmarking, and
/// fuzzing. Sometimes though, it's nice to be able to import a module from the
/// file system directly (such as the `Builtins`).
#[extension_trait]
pub impl PopulateInMemoryProviderFromFileSystem for InMemoryModuleProvider {
    fn load_package_from_file_system(&mut self, package_name: impl Into<String>) {
        let package_name = package_name.into();
        let packages_path = PackagesPath::try_from("../../packages").unwrap();
        let package_path = packages_path.join(package_name.clone());
        let package = Package::Managed(package_name.into());

        for file in WalkDir::new(&package_path)
            .into_iter()
            .map(Result::unwrap)
            .filter(|it| it.file_type().is_file())
        {
            let module = Module::from_package_and_path(
                &packages_path,
                package.clone(),
                file.path(),
                ModuleKind::Code,
            )
            .unwrap();

            let source_code = fs::read_to_string(file.path()).unwrap();
            self.add_str(&module, source_code);
        }
    }
}

pub trait DebugDisplay: Debug + Display {
    fn to_string(&self, is_debug: bool) -> String {
        if is_debug {
            format!("{:?}", self)
        } else {
            format!("{}", self)
        }
    }
    fn fmt(&self, f: &mut Formatter, is_debug: bool) -> fmt::Result;
}
macro_rules! impl_debug_display_via_debugdisplay {
    ($type:ty) => {
        impl std::fmt::Debug for $type {
            fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                DebugDisplay::fmt(self, f, true)
            }
        }
        impl std::fmt::Display for $type {
            fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                DebugDisplay::fmt(self, f, false)
            }
        }
    };
}

macro_rules! impl_eq_hash_ord_via_get {
    ($type:ty) => {
        impl Eq for $type {}
        impl PartialEq for $type {
            fn eq(&self, other: &Self) -> bool {
                self.get() == other.get()
            }
        }

        impl std::hash::Hash for $type {
            fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
                self.get().hash(state)
            }
        }

        impl Ord for $type {
            fn cmp(&self, other: &Self) -> std::cmp::Ordering {
                self.get().cmp(&other.get())
            }
        }
        impl PartialOrd for $type {
            fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
                Some(self.cmp(other))
            }
        }
    };
}

pub(super) use {impl_debug_display_via_debugdisplay, impl_eq_hash_ord_via_get};
