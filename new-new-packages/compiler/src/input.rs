use std::{fs::read_to_string, path::PathBuf, sync::Arc};

use salsa::query_group;

#[query_group(InputStorage)]
pub trait Input: InputWatcher {
    fn get_input(&self, input_reference: InputReference) -> Option<Arc<String>>;
}

fn get_input(db: &dyn Input, input_reference: InputReference) -> Option<Arc<String>> {
    db.salsa_runtime()
        .report_synthetic_read(salsa::Durability::LOW);
    if let Some(content) = db.get_open_input(&input_reference) {
        return Some(Arc::new(content));
    };

    match input_reference {
        InputReference::File(path) => match read_to_string(&path) {
            Ok(content) => Some(Arc::new(content)),
            Err(error) if matches!(error.kind(), std::io::ErrorKind::NotFound) => None,
            _ => panic!("Unexpected error when reading file {:?}.", path),
        },
        InputReference::Untitled(_) => None,
    }
}

pub trait InputWatcher {
    fn get_open_input(&self, input_reference: &InputReference) -> Option<String>;
}

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub enum InputReference {
    File(PathBuf),
    Untitled(String),
}
