use super::value::{Closure, Value};
use crate::{builtin_functions::BuiltinFunction, compiler::lir::Instruction};
use itertools::Itertools;
use num_bigint::BigInt;
use std::collections::HashMap;

#[derive(Clone)]
pub struct Heap {
    objects: HashMap<ObjectPointer, Object>,
    next_address: ObjectPointer,
}

pub type ObjectPointer = usize;

#[derive(Clone)]
pub struct Object {
    reference_count: usize,
    pub data: ObjectData,
}
#[derive(Clone)]
pub enum ObjectData {
    Int(BigInt),
    Text(String),
    Symbol(String),
    Struct(HashMap<ObjectPointer, ObjectPointer>),
    Closure {
        captured: Vec<ObjectPointer>,
        num_args: usize,
        body: Vec<Instruction>,
    },
    Builtin(BuiltinFunction),
}

impl std::fmt::Debug for Heap {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut objects = self.objects.clone().into_iter().collect_vec();
        objects.sort_by_key(|(address, _)| *address);

        writeln!(f, "{{")?;
        for (address, object) in objects {
            writeln!(
                f,
                "{address}: {} {}",
                object.reference_count,
                self.export_without_dropping(address)
            )?;
        }
        write!(f, "}}")
    }
}

impl Heap {
    pub fn new() -> Self {
        Self {
            objects: HashMap::new(),
            next_address: 1,
        }
    }

    pub fn get(&self, address: ObjectPointer) -> &Object {
        self.objects
            .get(&address)
            .unwrap_or_else(|| panic!("Couldn't get object {address}."))
    }
    pub fn get_mut(&mut self, address: ObjectPointer) -> &mut Object {
        self.objects
            .get_mut(&address)
            .unwrap_or_else(|| panic!("Couldn't get object {address}."))
    }

    pub fn dup(&mut self, address: ObjectPointer) {
        self.get_mut(address).reference_count += 1;
        log::trace!(
            "RefCount of {address} increased to {}. Value: {}",
            self.get(address).reference_count,
            self.export_without_dropping(address),
        );
    }
    pub fn drop(&mut self, address: ObjectPointer) {
        let value = self.export_without_dropping(address); // TODO: Only for logging
        let object = self.get_mut(address);
        object.reference_count -= 1;
        log::trace!(
            "RefCount of {address} reduced to {}. Value: {value}",
            object.reference_count,
        );
        if object.reference_count == 0 {
            self.free(address);
        }
    }

    pub fn create(&mut self, object: ObjectData) -> ObjectPointer {
        let address = self.next_address;
        self.objects.insert(
            address,
            Object {
                reference_count: 1,
                data: object,
            },
        );
        let value = self.export_without_dropping(address);
        log::trace!("Created object {value} at {address}.");
        self.next_address += 1;
        address
    }
    pub fn free(&mut self, address: ObjectPointer) {
        let object = self.objects.remove(&address).unwrap();
        log::trace!("Freeing object at {address}.");
        assert_eq!(object.reference_count, 0);
        match object.data {
            ObjectData::Int(_) => {}
            ObjectData::Text(_) => {}
            ObjectData::Symbol(_) => {}
            ObjectData::Struct(entries) => {
                for (key, value) in entries {
                    self.drop(key);
                    self.drop(value);
                }
            }
            ObjectData::Closure { captured, .. } => {
                for object in captured {
                    self.drop(object);
                }
            }
            ObjectData::Builtin(_) => {}
        }
    }

    pub fn import(&mut self, value: Value) -> ObjectPointer {
        let value = match value {
            Value::Int(int) => ObjectData::Int(int),
            Value::Text(text) => ObjectData::Text(text),
            Value::Symbol(symbol) => ObjectData::Symbol(symbol),
            Value::Struct(struct_) => {
                let mut entries = HashMap::new();
                for (key, value) in struct_ {
                    let key = self.import(key);
                    let value = self.import(value);
                    entries.insert(key, value);
                }
                ObjectData::Struct(entries)
            }
            Value::Closure(Closure {
                captured,
                num_args,
                body,
            }) => ObjectData::Closure {
                captured: captured
                    .into_iter()
                    .map(|value| self.import(value))
                    .collect(),
                num_args,
                body,
            },
            Value::Builtin(builtin) => ObjectData::Builtin(builtin),
        };
        self.create(value)
    }
    pub fn export(&mut self, address: ObjectPointer) -> Value {
        let value = self.export_without_dropping(address);
        self.drop(address);
        value
    }
    pub fn export_without_dropping(&self, address: ObjectPointer) -> Value {
        match &self.get(address).data {
            ObjectData::Int(int) => Value::Int(int.clone()),
            ObjectData::Text(text) => Value::Text(text.clone()),
            ObjectData::Symbol(symbol) => Value::Symbol(symbol.clone()),
            ObjectData::Struct(struct_) => {
                let mut entries = im::HashMap::new();
                for (key, value) in struct_ {
                    let key = self.export_without_dropping(*key);
                    let value = self.export_without_dropping(*value);
                    entries.insert(key, value);
                }
                Value::Struct(entries)
            }
            ObjectData::Closure {
                captured,
                num_args,
                body,
            } => Value::Closure(Closure {
                captured: captured
                    .iter()
                    .map(|address| self.export_without_dropping(*address))
                    .collect(),
                num_args: *num_args,
                body: body.clone(),
            }),
            ObjectData::Builtin(builtin) => Value::Builtin(*builtin),
        }
    }
}
