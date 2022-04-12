use std::{
    fmt::{self, Display, Formatter},
    fs::{self, read_to_string},
    path::PathBuf,
    sync::Arc,
};

use salsa::query_group;

use crate::database::PROJECT_DIRECTORY;

#[query_group(InputDbStorage)]
pub trait InputDb: InputWatcher {
    fn get_input(&self, input: Input) -> Option<Arc<String>>;
    fn get_open_input(&self, input: Input) -> Option<Arc<String>>;
}

fn get_input(db: &dyn InputDb, input: Input) -> Option<Arc<String>> {
    if let Some(content) = db.get_open_input(input.clone()) {
        return Some(content);
    };

    match input {
        Input::File(_) | Input::ExternalFile(_) => {
            let path = input.to_path().unwrap();
            match read_to_string(path.clone()) {
                Ok(content) => Some(Arc::new(content)),
                Err(error) if matches!(error.kind(), std::io::ErrorKind::NotFound) => None,
                _ => panic!("Unexpected error when reading file {:?}.", path),
            }
        }
        Input::Untitled(_) => None,
    }
}
fn get_open_input(db: &dyn InputDb, input: Input) -> Option<Arc<String>> {
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
    fn get_open_input_raw(&self, input: &Input) -> Option<String>;
}

#[derive(Debug, PartialEq, Eq, Clone, Hash, PartialOrd, Ord)]
pub enum Input {
    /// Contains the path components from [PROJECT_DIRECTORY] to the file.
    ///
    /// `.` and `..` are not allowed.
    File(Vec<String>),
    /// A file not belonging to the current project.
    ///
    /// This is temporary and should become obsolete when we support packages.
    ExternalFile(PathBuf),
    Untitled(String),
}

impl From<PathBuf> for Input {
    fn from(path: PathBuf) -> Self {
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
impl Input {
    pub fn to_path(&self) -> Option<PathBuf> {
        match self {
            Input::File(components) => {
                let mut path = PROJECT_DIRECTORY.lock().unwrap().clone().unwrap().clone();
                for component in components {
                    path.push(component);
                }
                Some(path)
            }
            Input::ExternalFile(path) => Some(path.clone()),
            Input::Untitled(_) => None,
        }
    }
}
impl Display for Input {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match self {
            Input::File(components) => {
                write!(f, "project-file:")?;
                if let Some(component) = components.first() {
                    write!(f, "{}", component)?;
                }
                for component in &components[1..] {
                    write!(f, "/{}", component)?;
                }
                Ok(())
            }
            Input::ExternalFile(path) => write!(f, "external-file:{}", path.display()),
            Input::Untitled(name) => write!(f, "untitled:{}", name),
        }
    }
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
        let input: Input = PathBuf::from("/foo.rs").into();

        db.did_open_input(&input, "123".to_owned());
        assert_eq!(
            db.get_input(input.clone()).unwrap().as_ref().to_owned(),
            "123"
        );
        assert_eq!(
            db.rcst(input.clone()).unwrap().as_ref().to_owned(),
            vec![Rcst::Int(123),],
        );

        db.did_change_input(&input, "456".to_owned());
        assert_eq!(
            db.get_input(input.clone()).unwrap().as_ref().to_owned(),
            "456"
        );
        assert_eq!(
            db.rcst(input.clone()).unwrap().as_ref().to_owned(),
            vec![Rcst::Int(456)],
        );

        db.did_close_input(&input);
    }
}
