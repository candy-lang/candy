use extension_trait::extension_trait;
use rustc_hash::FxHashSet;
use std::{
    ffi::OsString,
    fmt::{self, Display, Formatter},
    fs,
    hash::Hash,
    path::{Path, PathBuf},
};

#[derive(Debug, PartialEq, Eq, Clone, Hash, PartialOrd, Ord)]
pub enum Package {
    /// A package written by the user.
    User(PathBuf),

    /// A package managed by the Candy tooling. This is in some special cache
    /// directory where `use`d packages are downloaded to.
    Managed(PathBuf),

    /// An anonymous package. This is created for single untitled files that are
    /// not yet persisted to disk (such as when opening a new VSCode tab and
    /// typing some code).
    Anonymous { url: String },

    /// This package can make the tooling responsible for calls. For example,
    /// the fuzzer and constant evaluator use this.
    Tooling(String),
}

impl Package {
    pub fn to_path(&self, packages_path: &Path) -> Option<PathBuf> {
        match self {
            Package::User(path) => Some(path.clone()),
            Package::Managed(path) => Some(packages_path.join(path)),
            Package::Anonymous { .. } => None,
            Package::Tooling(_) => None,
        }
    }
}
impl Display for Package {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match self {
            Package::User(path) => write!(f, "user:{path:?}"),
            Package::Managed(path) => write!(f, "managed:{path:?}"),
            Package::Anonymous { url } => write!(f, "anonymous:{url}"),
            Package::Tooling(tooling) => write!(f, "tooling:{tooling}"),
        }
    }
}

#[extension_trait]
pub impl SurroundingPackage for Path {
    fn surrounding_candy_package(&self, packages_path: &Path) -> Option<Package> {
        let mut candidate = if self.is_dir() {
            self.to_path_buf()
        } else {
            self.parent().unwrap().to_path_buf()
        };

        loop {
            let children = fs::read_dir(&candidate)
                .unwrap()
                .map(|child| child.unwrap().file_name())
                .collect::<FxHashSet<OsString>>();

            if !children.contains(&OsString::from("_.candy".to_string())) {
                return None;
            }

            if children.contains(&OsString::from("_package.candy".to_string())) {
                break;
            } else if let Some(parent) = candidate.parent() {
                candidate = parent.to_path_buf();
            } else {
                return None;
            }
        }

        // The `candidate` folder contains the `_package.candy` file.
        Some(
            if let Ok(path_relative_to_packages) =
                candidate.strip_prefix(fs::canonicalize(packages_path).unwrap())
            {
                Package::Managed(path_relative_to_packages.to_path_buf())
            } else {
                Package::User(candidate)
            },
        )
    }
}
