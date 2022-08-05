use itertools::Itertools;
use lsp_types::Url;
use salsa::query_group;
use std::{
    fmt::{self, Display, Formatter},
    fs,
    hash::Hash,
    path::PathBuf,
    sync::Arc,
};

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
}

impl Module {
    pub fn from_package_root_and_url(package_root: PathBuf, url: Url, kind: ModuleKind) -> Self {
        match url.scheme() {
            "file" => {
                Module::from_package_root_and_file(package_root, url.to_file_path().unwrap(), kind)
            }
            "untitled" => Module {
                package: Package::Anonymous {
                    url: url
                        .to_string()
                        .strip_prefix("untitled:")
                        .unwrap()
                        .to_string(),
                },
                path: vec![],
                kind,
            },
            _ => panic!("Unsupported URI scheme: {}", url.scheme()),
        }
    }
    pub fn from_package_root_and_file(
        package_root: PathBuf,
        file: PathBuf,
        kind: ModuleKind,
    ) -> Self {
        let relative_path =
            fs::canonicalize(&file).expect("File does not exist or its path is invalid.");
        let relative_path =
            match relative_path.strip_prefix(fs::canonicalize(package_root.clone()).unwrap()) {
                Ok(path) => path,
                Err(_) => {
                    return Module {
                        package: Package::External(file),
                        path: vec![],
                        kind,
                    }
                }
            };

        let mut path = relative_path
            .components()
            .into_iter()
            .map(|component| match component {
                std::path::Component::Prefix(_) => unreachable!(),
                std::path::Component::RootDir => unreachable!(),
                std::path::Component::CurDir => panic!("`.` is not allowed in an module path."),
                std::path::Component::ParentDir => {
                    panic!("`..` is not allowed in an module path.")
                }
                std::path::Component::Normal(it) => {
                    it.to_str().expect("Invalid UTF-8 in path.").to_owned()
                }
            })
            .collect_vec();

        if kind == ModuleKind::Code {
            let last = path.pop().unwrap();
            let last = last
                .strip_suffix(".candy")
                .expect("Code module doesn't end with `.candy`?");
            if !last.is_empty() {
                path.push(last.to_string());
            }
        }

        Module {
            package: Package::User(package_root),
            path,
            kind,
        }
    }
}

impl Package {
    pub fn to_path(&self) -> Option<PathBuf> {
        match self {
            Package::User(path) => Some(path.clone()),
            Package::External(path) => Some(path.clone()),
            Package::Anonymous { .. } => None,
        }
    }
}
impl Module {
    pub fn to_possible_paths(&self) -> Option<Vec<PathBuf>> {
        let mut path = self.package.to_path()?;
        for component in self.path.clone() {
            path.push(component);
        }
        Some(match self.kind {
            ModuleKind::Asset => vec![path],
            ModuleKind::Code => vec![
                {
                    let mut path = path.clone();
                    path.push(".candy");
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

    pub fn dump_associated_debug_file(&self, debug_type: &str, content: &str) {
        let mut path = match self.to_possible_paths() {
            Some(path) => path,
            None => return,
        }
        .pop()
        .unwrap();
        path.set_extension(format!("candy.{}", debug_type));
        fs::write(path, content).unwrap();
    }
}

impl Display for Package {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match self {
            Package::User(path) => write!(f, "user:{path:?}"),
            Package::External(path) => write!(f, "extern:{path:?}"),
            Package::Anonymous { url } => write!(f, "anonymous:{url}"),
        }
    }
}
impl Display for Module {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(
            f,
            "{}:{}",
            self.package,
            self.path
                .iter()
                .map(|component| component.to_string())
                .join("/")
        )?;
        Ok(())
    }
}

#[query_group(ModuleDbStorage)]
pub trait ModuleDb: ModuleWatcher {
    fn get_module_content_as_string(&self, module: Module) -> Option<Arc<String>>;
    fn get_module_content(&self, module: Module) -> Option<Arc<Vec<u8>>>;
    fn get_open_module_content(&self, module: Module) -> Option<Arc<Vec<u8>>>;
}

fn get_module_content_as_string(db: &dyn ModuleDb, module: Module) -> Option<Arc<String>> {
    let content = get_module_content(db, module)?;
    String::from_utf8((*content).clone()).ok().map(Arc::new)
}

fn get_module_content(db: &dyn ModuleDb, module: Module) -> Option<Arc<Vec<u8>>> {
    if let Some(content) = db.get_open_module_content(module.clone()) {
        return Some(content);
    };

    for path in module.to_possible_paths().expect(
        "Tried to get content of anonymous module that is not cached by the language server.",
    ) {
        match fs::read(path.clone()) {
            Ok(content) => return Some(Arc::new(content)),
            Err(error) if matches!(error.kind(), std::io::ErrorKind::NotFound) => {}
            Err(_) => {
                log::error!("Unexpected error when reading file {:?}.", path);
            }
        }
    }
    None
}
fn get_open_module_content(db: &dyn ModuleDb, module: Module) -> Option<Arc<Vec<u8>>> {
    // The following line of code shouldn't be neccessary, but it is.
    //
    // We call `GetOpenModuleQuery.in_db_mut(self).invalidate(module);`
    // in `Database.did_open_module(…)`, `.did_change_module(…)`, and
    // `.did_close_module(…)` which correctly forces Salsa to re-run this query
    // function the next time this module is used. However, even though the
    // return value changes, Salsa doesn't record an updated `changed_at` value
    // in its internal `ActiveQuery` struct. `Runtime.report_untracked_read()`
    // manually sets this to the current revision.
    db.salsa_runtime().report_untracked_read();

    let content = db.get_open_module_raw(&module)?;
    Some(Arc::new(content))
}

pub trait ModuleWatcher {
    fn get_open_module_raw(&self, module: &Module) -> Option<Vec<u8>>;
}

#[cfg(test)]
mod test {
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
            "123"
        );
        assert_eq!(
            db.rcst(module.clone()).unwrap().as_ref().to_owned(),
            vec![Rcst::Int {
                value: 123u8.into(),
                string: "123".to_string()
            },],
        );

        db.did_change_module(&module, "456".to_string().into_bytes());
        assert_eq!(
            db.get_module_content_as_string(module.clone())
                .unwrap()
                .as_ref()
                .to_owned(),
            "456"
        );
        assert_eq!(
            db.rcst(module.clone()).unwrap().as_ref().to_owned(),
            vec![Rcst::Int {
                value: 456u16.into(),
                string: "456".to_string()
            }],
        );

        db.did_close_module(&module);
    }
}
