use im::HashMap;

use crate::compiler::hir::Id;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Value {
    Int(u64),
    Text(String),
    Symbol(String),
    Lambda(Lambda),
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Environment {
    parent: Option<Box<Environment>>,
    bindings: HashMap<Id, Value>,
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
    pub fn bool_true() -> Self {
        Value::Symbol("True".to_owned())
    }
    pub fn bool_false() -> Self {
        Value::Symbol("False".to_owned())
    }
    pub fn argument_count_mismatch_text(
        function_name: &str,
        parameter_count: usize,
        argument_count: usize,
    ) -> Value {
        format!(
            "Function `{}` expects {} arguments, but {} were given.",
            function_name, parameter_count, argument_count
        )
        .into()
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
        if value {
            Value::bool_true()
        } else {
            Value::bool_false()
        }
    }
}

impl Environment {
    pub fn new(bindings: HashMap<Id, Value>) -> Environment {
        Self {
            parent: None,
            bindings,
        }
    }
    pub fn store(&mut self, id: Id, value: Value) {
        assert!(
            !self.bindings.contains_key(&id),
            "Tried to overwrite a value at ID {}: {:?}",
            &id,
            &value
        );
        assert!(self.bindings.insert(id, value).is_none())
    }
    pub fn get(&self, id: &Id) -> Option<Value> {
        self.bindings
            .get(id)
            .map(|it| it.clone())
            .or_else(|| self.parent.as_ref()?.get(id))
    }
}
