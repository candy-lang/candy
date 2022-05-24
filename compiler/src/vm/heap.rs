use super::value::Value;
use crate::compiler::lir::ChunkIndex;
use std::collections::HashMap;

#[derive(Debug)]
pub struct Heap {
    objects: HashMap<ObjectPointer, Object>,
    next_address: ObjectPointer,
}

pub type ObjectPointer = usize;

#[derive(Debug, Clone)] // TODO: remove Clone once it's no longer needed
pub struct Object {
    reference_count: usize,
    pub data: ObjectData,
}
#[derive(Clone, Debug)] // TODO: remove Clone once it's no longer needed
pub enum ObjectData {
    Int(u64),
    Text(String),
    Symbol(String),
    Struct(HashMap<ObjectPointer, ObjectPointer>),
    Closure {
        // TODO: This could later be just a vector of object pointers, but for
        // now we capture the whole stack.
        captured: Vec<ObjectPointer>,
        body: ChunkIndex,
    },
}

impl Heap {
    pub fn new() -> Self {
        Self {
            objects: HashMap::new(),
            next_address: 0,
        }
    }

    pub fn get(&self, address: ObjectPointer) -> &Object {
        self.objects
            .get(&address)
            .expect(&format!("Couldn't get object {}.", address))
    }
    pub fn get_mut(&mut self, address: ObjectPointer) -> &mut Object {
        self.objects
            .get_mut(&address)
            .expect(&format!("Couldn't get object {}.", address))
    }

    pub fn dup(&mut self, address: ObjectPointer) {
        self.get_mut(address).reference_count += 1;
    }
    pub fn drop(&mut self, address: ObjectPointer) {
        let object = self.get_mut(address);
        object.reference_count -= 1;
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
        self.next_address += 1;
        address
    }
    pub fn free(&mut self, address: ObjectPointer) {
        let object = self.objects.remove(&address).unwrap();
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
        }
    }

    pub(super) fn import(&mut self, value: Value) -> ObjectPointer {
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
            Value::Closure { captured, body } => {
                let mut captured = vec![];
                ObjectData::Closure { captured, body }
            }
        };
        self.create(value)
    }
    pub(super) fn export(&mut self, address: ObjectPointer) -> Value {
        let value = self.export_without_dropping(address);
        self.drop(address);
        value
    }
    pub fn export_without_dropping(&self, address: ObjectPointer) -> Value {
        match &self.get(address).data {
            ObjectData::Int(int) => Value::Int(*int),
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
            ObjectData::Closure { captured, body } => Value::Closure {
                captured: captured.clone(),
                body: *body,
            },
        }
    }
}
