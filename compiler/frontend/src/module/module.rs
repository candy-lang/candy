use super::package::{Package, SurroundingPackage};
use crate::rich_ir::{RichIrBuilder, ToRichIr, TokenType};
use itertools::Itertools;
use std::{
    fs,
    hash::Hash,
    path::{Path, PathBuf},
};
use tracing::{error, warn};

#[derive(Debug, PartialEq, Eq, Clone, Hash, PartialOrd, Ord)]
pub struct Module {
    pub package: Package,
    pub path: Vec<String>,
    pub kind: ModuleKind,
}
#[derive(Debug, PartialEq, Eq, Clone, Hash, PartialOrd, Ord)]
pub enum ModuleKind {
    Code,
    Asset,
}

impl Module {
    pub fn from_package_name(name: &str) -> Self {
        Module {
            package: Package::Managed(name.into()),
            path: vec![],
            kind: ModuleKind::Code,
        }
    }

    pub fn from_path(packages_path: &Path, path: &Path, kind: ModuleKind) -> Result<Self, String> {
        assert!(path.is_absolute());

        let package = path
            .surrounding_candy_package(packages_path)
            .unwrap_or_else(|| Package::User(path.to_path_buf()));

        Self::from_package_and_path(packages_path, package, path, kind)
    }
    pub fn from_package_and_path(
        packages_path: &Path,
        package: Package,
        path: &Path,
        kind: ModuleKind,
    ) -> Result<Self, String> {
        let Ok(canonicalized) = fs::canonicalize(path) else {
            return Err(format!(
                "File `{}` does not exist or its path is invalid.",
                path.to_string_lossy(),
            ))
        };
        let relative_path = canonicalized
            .strip_prefix(package.to_path(packages_path).unwrap())
            .expect("File is not located in the package.");

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

        Ok(Module {
            package,
            path,
            kind,
        })
    }

    pub fn to_possible_paths(&self, packages_path: &Path) -> Option<Vec<PathBuf>> {
        let mut path = self.package.to_path(packages_path)?;
        for component in self.path.clone() {
            path.push(component);
        }
        Some(match self.kind {
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
    fn try_to_path(&self, packages_path: &Path) -> Option<PathBuf> {
        let paths = self.to_possible_paths(packages_path).unwrap_or_else(|| {
            panic!(
                "Tried to get content of anonymous module {} that is not cached by the language server.",
                self.to_rich_ir(),
            )
        });
        for path in paths {
            match path.try_exists() {
                Ok(true) => return Some(path),
                Ok(false) => {}
                Err(error) if matches!(error.kind(), std::io::ErrorKind::NotFound) => {}
                Err(_) => error!("Unexpected error when reading file {path:?}."),
            }
        }
        None
    }

    pub fn dump_associated_debug_file(
        &self,
        packages_path: &Path,
        debug_type: &str,
        content: &str,
    ) {
        let Some(mut path) = self.try_to_path(packages_path) else { return; };
        path.set_extension(format!("candy.{}", debug_type));
        fs::write(path.clone(), content).unwrap_or_else(|error| {
            warn!(
                "Couldn't write to associated debug file {}: {error}.",
                path.to_string_lossy(),
            )
        });
    }
}
impl ToRichIr for Module {
    fn build_rich_ir(&self, builder: &mut RichIrBuilder) {
        let range = builder.push(
            format!(
                "{}:{}",
                self.package,
                self.path
                    .iter()
                    .map(|component| component.to_string())
                    .join("/")
            ),
            TokenType::Module,
            Default::default(),
        );
        builder.push_reference(self.to_owned(), range);
    }
}
