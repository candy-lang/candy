use super::package::{Package, PackagesPath};
use crate::{
    impl_display_via_richir,
    rich_ir::{RichIrBuilder, ToRichIr, TokenType},
};
use enumset::EnumSet;
use itertools::Itertools;
use std::{
    fmt::{self, Display, Formatter},
    fs,
    hash::Hash,
    path::{Path, PathBuf},
    sync::Arc,
};
use tracing::{error, warn};

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Module(Arc<InnerModule>);
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
struct InnerModule {
    package: Package,
    path: Vec<String>,
    kind: ModuleKind,
}
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ModuleKind {
    Code,
    Asset,
}
impl From<InnerModule> for Module {
    fn from(val: InnerModule) -> Self {
        Self(Arc::new(val))
    }
}
impl Module {
    #[must_use]
    pub fn package(&self) -> &Package {
        &self.0.package
    }
    #[must_use]
    pub fn path(&self) -> &Vec<String> {
        &self.0.path
    }
    #[must_use]
    pub fn kind(&self) -> ModuleKind {
        self.0.kind
    }

    #[must_use]
    pub fn new(package: Package, path: Vec<String>, kind: ModuleKind) -> Self {
        Self(Arc::new(InnerModule {
            package,
            path,
            kind,
        }))
    }

    #[must_use]
    pub fn from_package_name(name: String) -> Self {
        InnerModule {
            package: Package::Managed(name.into()),
            path: vec![],
            kind: ModuleKind::Code,
        }
        .into()
    }

    pub fn from_path(
        packages_path: &PackagesPath,
        path: &Path,
        kind: ModuleKind,
    ) -> Result<Self, ModuleFromPathError> {
        let package = packages_path
            .find_surrounding_package(path)
            .unwrap_or_else(|| Package::User(path.to_path_buf()));

        Self::from_package_and_path(packages_path, package, path, kind)
    }
    pub fn from_package_and_path(
        packages_path: &PackagesPath,
        package: Package,
        path: &Path,
        kind: ModuleKind,
    ) -> Result<Self, ModuleFromPathError> {
        let canonicalized = dunce::canonicalize(path)
            .map_err(|_| ModuleFromPathError::NotFound(path.to_owned()))?;
        let relative_path = canonicalized
            .strip_prefix(package.to_path(packages_path).unwrap())
            .map_err(|_| ModuleFromPathError::NotInPackage(path.to_owned()))?;

        let mut path = relative_path
            .components()
            .map(|component| match component {
                std::path::Component::Prefix(_) => unreachable!(),
                std::path::Component::RootDir => unreachable!(),
                std::path::Component::CurDir => panic!("`.` is not allowed in a module path."),
                std::path::Component::ParentDir => {
                    panic!("`..` is not allowed in a module path.")
                }
                std::path::Component::Normal(it) => {
                    it.to_str().expect("Invalid UTF-8 in path.").to_owned()
                }
            })
            .collect_vec();

        if kind == ModuleKind::Code && !path.is_empty() {
            let last = path.pop().unwrap();
            let last = last
                .strip_suffix(".candy")
                .expect("Code module doesn't end with `.candy`?");
            if last != "_" {
                path.push(last.to_string());
            }
        }

        Ok(InnerModule {
            package,
            path,
            kind,
        }
        .into())
    }

    #[must_use]
    pub fn to_possible_paths(&self, packages_path: &PackagesPath) -> Option<Vec<PathBuf>> {
        let mut path = self.package().to_path(packages_path)?;
        for component in self.path() {
            path.push(component);
        }
        Some(match self.kind() {
            ModuleKind::Asset => vec![path],
            ModuleKind::Code => vec![
                {
                    let mut path = path.clone();
                    path.push("_.candy");
                    path
                },
                {
                    let mut path = path.clone();
                    path.set_extension("candy");
                    path
                },
            ],
        })
    }
    #[must_use]
    pub fn try_to_path(&self, packages_path: &PackagesPath) -> Option<PathBuf> {
        for path in self.to_possible_paths(packages_path)? {
            match path.try_exists() {
                Ok(true) => return Some(path),
                Ok(false) => {}
                Err(error) if matches!(error.kind(), std::io::ErrorKind::NotFound) => {}
                Err(error) => error!("Unexpected error when reading file {path:?}: {error}."),
            }
        }
        None
    }

    pub fn dump_associated_debug_file(
        &self,
        packages_path: &PackagesPath,
        debug_type: &str,
        content: &str,
    ) {
        let Some(mut path) = self.try_to_path(packages_path) else {
            return;
        };

        path.set_extension(format!("candy.{}", debug_type));
        fs::write(path.clone(), content).unwrap_or_else(|error| {
            warn!(
                "Couldn't write to associated debug file {}: {error}.",
                path.to_string_lossy(),
            );
        });
    }
}

impl ToRichIr for Module {
    fn build_rich_ir(&self, builder: &mut RichIrBuilder) {
        let range = builder.push(
            format!(
                "{}:{}",
                self.package(),
                self.path().iter().map(ToString::to_string).join("/"),
            ),
            TokenType::Module,
            EnumSet::default(),
        );
        builder.push_reference(self.clone(), range);
    }
}
impl_display_via_richir!(Module);

#[derive(Debug)]
pub enum ModuleFromPathError {
    NotFound(PathBuf),
    NotInPackage(PathBuf),
}
impl Display for ModuleFromPathError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound(path) => {
                write!(
                    f,
                    "File `{}` does not exist or its path is invalid.",
                    path.to_string_lossy(),
                )
            }
            Self::NotInPackage(path) => {
                write!(
                    f,
                    "File `{}` is not located in the package.",
                    path.to_string_lossy()
                )
            }
        }
    }
}
