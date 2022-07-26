use salsa::query_group;
use std::{
    fmt::{self, Display, Formatter},
    fs,
    path::PathBuf,
    sync::Arc,
};

#[derive(Debug, PartialEq, Eq, Clone, Hash, PartialOrd, Ord)]
pub enum Input {
    File {
        package: Package,
        path: Vec<String>, // path components, `.` and `..` are not allowed
    },
    Untitled(String),
}

#[derive(Debug, PartialEq, Eq, Clone, Hash, PartialOrd, Ord)]
pub enum Package {
    User(PathBuf),

    /// A package managed by the Candy tooling. This is in some special cache
    /// directory where used packages are downloaded to.
    // TODO: Change this to just storing the package name or something like
    // that so that the root of the cached packages folder isn't stored
    // everywhere.
    Extern(PathBuf),
}

impl Input {
    fn from_user_file(project_root: PathBuf, file: PathBuf) -> Option<Self> {
        let project_dir = PROJECT_DIRECTORY.lock().unwrap().clone().unwrap();
        let path = match fs::canonicalize(&path)
            .expect("Path does not exist or is invalid.")
            .strip_prefix(fs::canonicalize(project_dir).unwrap().clone())
        {
            Ok(path) => path.to_owned(),
            Err(_) => return Input::ExternalFile(path),
        };

        let components = path
            .components()
            .into_iter()
            .map(|it| match it {
                std::path::Component::Prefix(_) => unreachable!(),
                std::path::Component::RootDir => unreachable!(),
                std::path::Component::CurDir => panic!("`.` is not allowed in an input path."),
                std::path::Component::ParentDir => {
                    panic!("`..` is not allowed in an input path.")
                }
                std::path::Component::Normal(it) => {
                    it.to_str().expect("Invalid UTF-8 in path.").to_owned()
                }
            })
            .collect();
        Input::File(components)
    }
}

impl Package {
    pub fn to_path(&self) -> PathBuf {
        match self {
            Package::User(path) => path.clone(),
            Package::Extern(path) => path.clone(),
        }
    }
}
impl Input {
    pub fn to_path(&self) -> Option<PathBuf> {
        match self {
            Input::File { package, path } => {
                let mut total_path = package.to_path();
                for component in path {
                    total_path.push(component);
                }
                Some(total_path)
            }
            Input::Untitled(_) => None,
        }
    }
}

impl Display for Package {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match self {
            Package::User(path) => write!(f, "user:{path:?}"),
            Package::Extern(path) => write!(f, "extern:{path:?}"),
        }
    }
}
impl Display for Input {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match self {
            Input::File { package, path } => {
                write!(f, "~:")?;
                if let Some(component) = path.first() {
                    write!(f, "{component}")?;
                }
                for component in &path[1..] {
                    write!(f, "/{component}")?;
                }
                Ok(())
            }
            Input::Untitled(name) => write!(f, "untitled:{name}"),
        }
    }
}

#[query_group(InputDbStorage)]
pub trait InputDb: InputWatcher {
    fn get_string_input(&self, input: Input) -> Option<Arc<String>>;
    fn get_input(&self, input: Input) -> Option<Arc<Vec<u8>>>;
    fn get_open_input(&self, input: Input) -> Option<Arc<Vec<u8>>>;
}

fn get_string_input(db: &dyn InputDb, input: Input) -> Option<Arc<String>> {
    let content = get_input(db, input)?;
    String::from_utf8((*content).clone())
        .ok()
        .map(|it| Arc::new(it))
}

fn get_input(db: &dyn InputDb, input: Input) -> Option<Arc<Vec<u8>>> {
    if let Some(content) = db.get_open_input(input.clone()) {
        return Some(content);
    };

    match input {
        Input::File { .. } => {
            let path = input.to_path().unwrap();
            match fs::read(path.clone()) {
                Ok(content) => Some(Arc::new(content)),
                Err(error) if matches!(error.kind(), std::io::ErrorKind::NotFound) => None,
                Err(_) => {
                    log::error!("Unexpected error when reading file {:?}.", path);
                    None
                }
            }
        }
        Input::Untitled(_) => None,
    }
}
fn get_open_input(db: &dyn InputDb, input: Input) -> Option<Arc<Vec<u8>>> {
    // The following line of code shouldn't be neccessary, but it is.
    //
    // We call `GetOpenInputQuery.in_db_mut(self).invalidate(input);`
    // in `Database.did_open_input(…)`, `.did_change_input(…)`, and
    // `.did_close_input(…)` which correctly forces Salsa to re-run this query
    // function the next time this input is used. However, even though the
    // return value changes, Salsa doesn't record an updated `changed_at` value
    // in its internal `ActiveQuery` struct. `Runtime.report_untracked_read()`
    // manually sets this to the current revision.
    db.salsa_runtime().report_untracked_read();

    let content = db.get_open_input_raw(&input)?;
    Some(Arc::new(content))
}

pub trait InputWatcher {
    fn get_open_input_raw(&self, input: &Input) -> Option<Vec<u8>>;
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        compiler::{rcst::Rcst, string_to_rcst::StringToRcst},
        database::Database,
    };

    #[test]
    fn on_demand_input_works() {
        let mut db = Database::default();
        let input: Input = Input::File {
            package: Package::User(PathBuf::from("/non/existent").into()),
            path: vec!["foo.candy".to_string()],
        };

        db.did_open_input(&input, "123".to_string().into_bytes());
        assert_eq!(
            String::from_utf8(db.get_input(input.clone()).unwrap().as_ref().to_owned()).unwrap(),
            "123"
        );
        assert_eq!(
            db.rcst(input.clone()).unwrap().as_ref().to_owned(),
            vec![Rcst::Int {
                value: 123,
                string: "123".to_string()
            },],
        );

        db.did_change_input(&input, "456".to_string().into_bytes());
        assert_eq!(
            String::from_utf8(db.get_input(input.clone()).unwrap().as_ref().to_owned()).unwrap(),
            "456"
        );
        assert_eq!(
            db.rcst(input.clone()).unwrap().as_ref().to_owned(),
            vec![Rcst::Int {
                value: 456,
                string: "456".to_string()
            }],
        );

        db.did_close_input(&input);
    }
}
