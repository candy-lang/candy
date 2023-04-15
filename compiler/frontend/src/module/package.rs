use extension_trait::extension_trait;
use rustc_hash::FxHashSet;
use std::{
    ffi::OsString,
    fmt::{self, Display, Formatter},
    fs,
    hash::Hash,
    path::PathBuf,
};

#[derive(Debug, PartialEq, Eq, Clone, Hash, PartialOrd, Ord)]
pub enum Package {
    /// A package written by the user.
    User(PathBuf),

    /// A package managed by the Candy tooling. This is in some special cache
    /// directory where `use`d packages are downloaded to.
    ///
    /// For now, this option is also used for files picked from the file system
    /// that are not part of the current working directory.
    //
    // TODO: Maybe add some sort of package indicator file after all so that we
    // can allow arbitrary opened files from the file system to access parent
    // and sibling modules if they're actually part of a larger package.
    //
    // TODO: Change this to just storing the package name or something like
    // that so that the root of the cached packages folder isn't stored
    // everywhere.
    External(PathBuf),

    /// An anonymous package. This is created for single untitled files that are
    /// not yet persisted to disk (such as when opening a new VSCode tab and
    /// typing some code).
    Anonymous { url: String },

    /// This package can make the tooling responsible for calls. For example,
    /// the fuzzer and constant evaluator use this.
    Tooling(String),
}

impl Package {
    pub fn to_path(&self) -> Option<PathBuf> {
        match self {
            Package::User(path) => Some(path.clone()),
            Package::External(path) => Some(path.clone()),
            Package::Anonymous { .. } => None,
            Package::Tooling(_) => None,
        }
    }
}
impl Display for Package {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match self {
            Package::User(path) => write!(f, "user:{path:?}"),
            Package::External(path) => write!(f, "extern:{path:?}"),
            Package::Anonymous { url } => write!(f, "anonymous:{url}"),
            Package::Tooling(tooling) => write!(f, "tooling:{tooling}"),
        }
    }
}

#[extension_trait]
pub impl SurroundingPackage for PathBuf {
    fn surrounding_candy_package(&self) -> Option<Package> {
        let mut candidate_folder = if self.is_dir() {
            self.clone()
        } else {
            self.parent().unwrap().to_path_buf()
        };
        loop {
            let children = fs::read_dir(&candidate_folder)
                .unwrap()
                .map(|child| child.unwrap().file_name())
                .collect::<FxHashSet<OsString>>();

            if !children.contains(&OsString::from("_.candy".to_string())) {
                return None;
            }

            if children.contains(&OsString::from("_package.candy".to_string())) {
                return Some(Package::User(candidate_folder));
            } else if let Some(parent) = candidate_folder.parent() {
                candidate_folder = parent.to_path_buf();
            } else {
                return None;
            }
        }
    }
}
