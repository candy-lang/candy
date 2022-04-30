use super::result::DiscoverResult;
use crate::compiler::hir::Id;
use im::HashMap;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Value {
    Int(u64),
    Text(String),
    Symbol(String),
    Struct(HashMap<Value, Value>),
    Lambda(Lambda),
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Environment {
    parent: Option<Box<Environment>>,
    bindings: HashMap<Id, DiscoverResult>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Lambda {
    pub captured_environment: Environment,
    pub id: Id,
}

impl Value {
    pub fn nothing() -> Self {
        Value::Symbol("Nothing".to_owned())
    }
    pub fn bool(value: bool) -> Self {
        Value::Symbol(if value {
            "True".to_owned()
        } else {
            "False".to_owned()
        })
    }

    pub fn list(items: Vec<Value>) -> Self {
        let items = items
            .into_iter()
            .enumerate()
            .map(|(index, it)| (Value::Int(index as u64), it))
            .collect();
        Value::Struct(items)
    }
}
impl From<u64> for Value {
    fn from(value: u64) -> Self {
        Value::Int(value)
    }
}
impl From<String> for Value {
    fn from(value: String) -> Self {
        Value::Text(value)
    }
}
impl From<bool> for Value {
    fn from(value: bool) -> Self {
        Value::bool(value)
    }
}

impl Environment {
    pub fn new() -> Environment {
        Self {
            parent: None,
            bindings: HashMap::new(),
        }
    }
    pub fn new_child(self) -> Environment {
        Self {
            parent: Some(Box::new(self)),
            bindings: HashMap::new(),
        }
    }
    pub fn store(&mut self, id: Id, value: DiscoverResult) {
        if let Some(old_value) = self.bindings.get(&id) {
            if !matches!(old_value, DiscoverResult::DependsOnParameter) {
                panic!(
                    "Tried to overwrite a value at ID {} with {:?} (old value: {:?})",
                    &id,
                    &value,
                    &self.bindings.get(&id).unwrap(),
                );
            }
        }
        assert!(self.bindings.insert(id, value).is_none())
    }
    pub fn get(&self, id: &Id) -> DiscoverResult {
        match self.bindings.get(id).map(|it| it.clone()) {
            Some(value) => value,
            None => match self.parent.as_ref() {
                Some(parent) => parent.get(id),
                None => panic!("Couldn't find a value for ID {}", id),
            },
        }
    }

    pub fn flatten(self) -> HashMap<Id, DiscoverResult> {
        let mut bindings = HashMap::new();
        let mut environment = Some(self);
        while let Some(env) = environment {
            bindings.extend(env.bindings.into_iter());
            environment = env.parent.map(|it| *it);
        }
        bindings
    }
}
