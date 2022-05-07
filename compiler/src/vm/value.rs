use std::fmt::{self, Display, Formatter};

use super::vm::StackEntry;
use crate::compiler::lir::ChunkIndex;
use im::HashMap;
use itertools::Itertools;

/// A self-contained value. Unlike objects, these are not tied to a running VM,
/// which makes them useful for being sent through channels between multiple
/// reference-counted heaps, for example ones running concurrently logically, on
/// other cores, or on different computers.
///
/// VMs can import these values to turn them into heap-contained,
/// reference-counted objects. They can export objects from the heap into
/// self-contained values.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum Value {
    Int(u64),
    Text(String),
    Symbol(String),
    Struct(HashMap<Value, Value>),
    Closure {
        captured: Vec<StackEntry>,
        body: ChunkIndex,
    },
}

impl Value {
    pub fn nothing() -> Value {
        Value::Symbol("Nothing".to_owned())
    }
    pub fn list(items: Vec<Value>) -> Self {
        let items = items
            .into_iter()
            .enumerate()
            .map(|(index, it)| (Value::Int(index as u64), it))
            .collect();
        Value::Struct(items)
    }

    pub fn into_int(self) -> Option<u64> {
        match self {
            Value::Int(int) => Some(int),
            _ => None,
        }
    }
    pub fn into_text(self) -> Option<String> {
        match self {
            Value::Text(text) => Some(text),
            _ => None,
        }
    }
    pub fn into_symbol(self) -> Option<String> {
        match self {
            Value::Symbol(symbol) => Some(symbol),
            _ => None,
        }
    }
    pub fn into_struct(self) -> Option<HashMap<Value, Value>> {
        match self {
            Value::Struct(entries) => Some(entries),
            _ => None,
        }
    }
    pub fn into_closure(self) -> Option<(Vec<StackEntry>, ChunkIndex)> {
        match self {
            Value::Closure { captured, body } => Some((captured, body)),
            _ => None,
        }
    }
}

impl Display for Value {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Value::Int(int) => write!(f, "{}", int),
            Value::Text(text) => write!(f, "{:?}", text),
            Value::Symbol(symbol) => write!(f, "{}", symbol),
            Value::Struct(entries) => write!(
                f,
                "{{ {} }}",
                entries
                    .iter()
                    .map(|(key, value)| format!("{}: {}", key, value))
                    .join(", ")
            ),
            Value::Closure { captured, body } => {
                write!(f, "closure@{}", body)
            }
        }
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
    fn from(it: bool) -> Self {
        Value::Symbol(if it { "True" } else { "False" }.to_string())
    }
}
