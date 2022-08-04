use crate::{builtin_functions::BuiltinFunction, compiler::lir::Instruction};
use itertools::Itertools;
use num_bigint::BigInt;
use std::{
    cmp::Ordering,
    collections::HashMap,
    fmt::{self, Formatter},
    hash::{Hash, Hasher},
    rc::Rc,
};

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

    /// Clones all objects at the `root_addresses` into the `other` heap and
    /// returns a list of their addresses in the other heap.
    pub fn clone_to_other_heap(
        &self,
        other: &mut Heap,
        root_addresses: Vec<ObjectPointer>,
    ) -> Vec<ObjectPointer> {
        let objects_to_refcounts = HashMap::new();
        for address in root_addresses {
            self.gather_objects_to_clone(&mut objects_to_refcounts, address);
        }
        let mapped_addresses = vec![];
        for (address, refcount) in objects_to_refcounts {
            mapped_addresses.push(other.create(self.get(address).data.clone()));
            other.get_mut(address).reference_count = refcount;
        }
        mapped_addresses
    }
    fn gather_objects_to_clone(
        &self,
        objects_to_refcounts: &mut HashMap<ObjectPointer, usize>,
        address: ObjectPointer,
    ) {
        *objects_to_refcounts.entry(address).or_default() += 1;
        match self.get(address).data {
            ObjectData::Int(_)
            | ObjectData::Text(_)
            | ObjectData::Symbol(_)
            | ObjectData::Builtin(_) => todo!(),
            ObjectData::Struct(fields) => {
                for (key, value) in fields {
                    self.gather_objects_to_clone(objects_to_refcounts, key);
                    self.gather_objects_to_clone(objects_to_refcounts, value);
                }
            }
            ObjectData::Closure {
                captured,
                num_args,
                body,
            } => {
                for captured in captured {
                    self.gather_objects_to_clone(objects_to_refcounts, captured);
                }
            }
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
    pub fn export_all(&mut self, addresses: &[ObjectPointer]) -> Vec<Value> {
        let mut values = vec![];
        for address in addresses {
            values.push(self.export(*address));
        }
        values
    }

    pub fn create_int(&mut self, int: BigInt) -> ObjectPointer {
        self.create(ObjectData::Int(int))
    }
    pub fn create_text(&mut self, text: String) -> ObjectPointer {
        self.create(ObjectData::Text(text))
    }
    pub fn create_symbol(&mut self, symbol: String) -> ObjectPointer {
        self.create(ObjectData::Symbol(symbol))
    }
    pub fn create_struct(
        &mut self,
        fields: HashMap<ObjectPointer, ObjectPointer>,
    ) -> ObjectPointer {
        self.create(ObjectData::Struct(fields))
    }
    pub fn create_closure(
        &mut self,
        captured: Vec<ObjectPointer>,
        num_args: usize,
        body: Vec<Instruction>,
    ) -> ObjectPointer {
        self.create(ObjectData::Closure {
            captured,
            num_args,
            body,
        })
    }
    pub fn create_builtin(&mut self, builtin: BuiltinFunction) -> ObjectPointer {
        self.create(ObjectData::Builtin(builtin))
    }
    pub fn create_nothing(&mut self) -> ObjectPointer {
        self.create(ObjectData::Symbol("Nothing".to_owned()))
    }
    pub fn create_list(&mut self, items: Vec<ObjectPointer>) -> ObjectPointer {
        let fields = vec![];
        for (index, item) in items.into_iter().enumerate() {
            fields.push((self.create_int(index.into()), item));
        }
        self.create_struct(fields.into_iter().collect())
    }
    pub fn create_bool(&mut self, value: bool) -> ObjectPointer {
        self.create_symbol(if value { "True" } else { "False" }.to_string())
    }
    pub fn create_result(&mut self, result: Result<ObjectPointer, ObjectPointer>) -> ObjectPointer {
        let (type_, value) = match result {
            Ok(it) => ("Ok".to_string(), it),
            Err(it) => ("Error".to_string(), it),
        };
        let fields = vec![
            (
                self.create_symbol("Type".to_string()),
                self.create_symbol(type_),
            ),
            (self.create_symbol("Value".to_string()), value),
        ]
        .into_iter()
        .collect();
        self.create_struct(fields)
    }

    fn hash<H: Hasher>(&self, state: &mut H, address: ObjectPointer) {
        match self.get(address).data {
            ObjectData::Int(int) => int.hash(state),
            ObjectData::Text(text) => text.hash(state),
            ObjectData::Symbol(symbol) => symbol.hash(state),
            ObjectData::Struct(fields) => {}
            ObjectData::Closure {
                captured,
                num_args,
                body,
            } => todo!(),
            ObjectData::Builtin(_) => todo!(),
        }
    }
    fn compare(&self, a: ObjectPointer, b: ObjectPointer) -> Ordering {
        if a == b {
            return Ordering::Equal;
        }
        let a = self.get(a);
        let b = self.get(b);
        match (a.data, b.data) {
            (ObjectData::Int(_), ObjectData::Int(_)) => todo!(),
            (ObjectData::Text(_), ObjectData::Text(_)) => todo!(),
            (ObjectData::Symbol(_), ObjectData::Symbol(_)) => todo!(),
            (ObjectData::Struct(_), ObjectData::Struct(_)) => todo!(),
            (
                ObjectData::Closure {
                    captured,
                    num_args,
                    body,
                },
                ObjectData::Closure {
                    captured,
                    num_args,
                    body,
                },
            ) => todo!(),
            (ObjectData::Builtin(_), ObjectData::Builtin(_)) => todo!(),
        }
    }

    fn format(&self, f: &mut Formatter<'_>, address: ObjectPointer) -> fmt::Result {
        match self.get(address).data {
            ObjectData::Int(int) => write!(f, "{int}"),
            ObjectData::Text(text) => write!(f, "{text:?}"),
            ObjectData::Symbol(symbol) => write!(f, "{symbol}"),
            ObjectData::Struct(entries) => write!(
                f,
                "[{}]",
                entries
                    .iter()
                    .map(|(key, value)| (format!("{}", key), value))
                    .sorted_by(|(key_a, _), (key_b, _)| key_a.cmp(key_b))
                    .map(|(key, value)| format!("{}: {}", key, value))
                    .join(", ")
            ),
            ObjectData::Closure(_) => write!(f, "{{…}}"),
            ObjectData::Builtin(builtin) => write!(f, "builtin{builtin:?}"),
        }
    }
}
