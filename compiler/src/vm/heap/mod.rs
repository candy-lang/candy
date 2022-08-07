mod object;
mod pointer;

pub use self::{
    object::{Builtin, Closure, Data, Int, Object, Struct, Symbol, Text},
    pointer::Pointer,
};
use crate::builtin_functions::BuiltinFunction;
use itertools::Itertools;
use num_bigint::BigInt;
use std::{cmp::Ordering, collections::HashMap};
use tracing::trace;

#[derive(Clone)]
pub struct Heap {
    objects: HashMap<Pointer, Object>,
    next_address: Pointer,
}

impl std::fmt::Debug for Heap {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut objects = self.objects.clone().into_iter().collect_vec();
        objects.sort_by_key(|(address, _)| address.raw());

        writeln!(f, "{{")?;
        for (address, object) in objects {
            writeln!(
                f,
                "{address}: {} {}",
                object.reference_count,
                address.format(self),
            )?;
        }
        write!(f, "}}")
    }
}

impl Default for Heap {
    fn default() -> Self {
        Self {
            objects: HashMap::new(),
            next_address: Pointer::from_raw(1),
        }
    }
}
impl Heap {
    pub fn get(&self, address: Pointer) -> &Object {
        self.objects
            .get(&address)
            .unwrap_or_else(|| panic!("Couldn't get object {address}."))
    }
    pub fn get_mut(&mut self, address: Pointer) -> &mut Object {
        self.objects
            .get_mut(&address)
            .unwrap_or_else(|| panic!("Couldn't get object {address}."))
    }

    pub fn dup(&mut self, address: Pointer) {
        self.get_mut(address).reference_count += 1;
        trace!(
            "RefCount of {address} increased to {}. Value: {}",
            self.get(address).reference_count,
            address.format(self),
        );
    }
    pub fn drop(&mut self, address: Pointer) {
        let formatted_value = address.format(self);
        let object = self.get_mut(address);
        object.reference_count -= 1;
        trace!(
            "RefCount of {address} reduced to {}. Value: {formatted_value}",
            object.reference_count,
        );
        if object.reference_count == 0 {
            self.free(address);
        }
    }

    pub fn create(&mut self, object: Data) -> Pointer {
        let address = self.next_address;
        self.objects.insert(
            address,
            Object {
                reference_count: 1,
                data: object,
            },
        );
        trace!("Created object {} at {address}.", address.format(self));
        self.next_address = Pointer::from_raw(self.next_address.raw() + 1);
        address
    }
    pub fn free(&mut self, address: Pointer) {
        let object = self.objects.remove(&address).unwrap();
        trace!("Freeing object at {address}.");
        assert_eq!(object.reference_count, 0);
        for child in object.children() {
            self.drop(child);
        }
    }

    /// Clones all objects at the `root_addresses` into the `other` heap and
    /// returns a list of their addresses in the other heap.
    pub fn clone_multiple_to_other_heap(
        &self,
        other: &mut Heap,
        addresses: &[Pointer],
    ) -> Vec<Pointer> {
        let mut objects_to_refcounts = HashMap::new();
        for address in addresses {
            self.gather_objects_to_clone(&mut objects_to_refcounts, *address);
        }
        let num_objects = objects_to_refcounts.len();

        let address_map: HashMap<Pointer, Pointer> = objects_to_refcounts
            .keys()
            .cloned()
            .zip(
                (other.next_address.raw()..other.next_address.raw() + num_objects)
                    .map(Pointer::from_raw),
            )
            .collect();

        for (address, refcount) in objects_to_refcounts {
            other.objects.insert(
                address_map[&address],
                Object {
                    reference_count: refcount,
                    data: Self::map_addresses_in_data(&address_map, &self.get(address).data),
                },
            );
        }
        other.next_address = Pointer::from_raw(other.next_address.raw() + num_objects);

        addresses
            .iter()
            .map(|address| address_map[address])
            .collect()
    }
    fn gather_objects_to_clone(
        &self,
        objects_to_refcounts: &mut HashMap<Pointer, usize>,
        address: Pointer,
    ) {
        *objects_to_refcounts.entry(address).or_default() += 1;
        for child in self.get(address).children() {
            self.gather_objects_to_clone(objects_to_refcounts, child);
        }
    }
    fn map_addresses_in_data(address_map: &HashMap<Pointer, Pointer>, data: &Data) -> Data {
        match data {
            Data::Int(int) => Data::Int(int.clone()),
            Data::Text(text) => Data::Text(text.clone()),
            Data::Symbol(symbol) => Data::Symbol(symbol.clone()),
            Data::Struct(struct_) => Data::Struct(Struct {
                fields: struct_
                    .fields
                    .iter()
                    .map(|(hash, key, value)| (*hash, address_map[key], address_map[value]))
                    .collect_vec(),
            }),
            Data::Closure(closure) => Data::Closure(Closure {
                captured: closure
                    .captured
                    .iter()
                    .map(|address| address_map[address])
                    .collect(),
                num_args: closure.num_args,
                body: closure.body.clone(),
            }),
            Data::Builtin(builtin) => Data::Builtin(builtin.clone()),
        }
    }
    pub fn clone_single_to_other_heap(&self, other: &mut Heap, address: Pointer) -> Pointer {
        self.clone_multiple_to_other_heap(other, &[address])
            .pop()
            .unwrap()
    }

    pub fn create_int(&mut self, int: BigInt) -> Pointer {
        self.create(Data::Int(Int { value: int }))
    }
    pub fn create_text(&mut self, text: String) -> Pointer {
        self.create(Data::Text(Text { value: text }))
    }
    pub fn create_symbol(&mut self, symbol: String) -> Pointer {
        self.create(Data::Symbol(Symbol { value: symbol }))
    }
    pub fn create_struct(&mut self, fields: HashMap<Pointer, Pointer>) -> Pointer {
        self.create(Data::Struct(Struct::from_fields(self, fields)))
    }
    pub fn create_closure(&mut self, closure: Closure) -> Pointer {
        self.create(Data::Closure(closure))
    }
    pub fn create_builtin(&mut self, builtin: BuiltinFunction) -> Pointer {
        self.create(Data::Builtin(Builtin { function: builtin }))
    }
    pub fn create_nothing(&mut self) -> Pointer {
        self.create_symbol("Nothing".to_string())
    }
    pub fn create_list(&mut self, items: Vec<Pointer>) -> Pointer {
        let mut fields = vec![];
        for (index, item) in items.into_iter().enumerate() {
            fields.push((self.create_int(index.into()), item));
        }
        self.create_struct(fields.into_iter().collect())
    }
    pub fn create_bool(&mut self, value: bool) -> Pointer {
        self.create_symbol(if value { "True" } else { "False" }.to_string())
    }
    pub fn create_result(&mut self, result: Result<Pointer, Pointer>) -> Pointer {
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
    pub fn create_ordering(&mut self, ordering: Ordering) -> Pointer {
        self.create_symbol(
            match ordering {
                Ordering::Less => "Less",
                Ordering::Equal => "Equal",
                Ordering::Greater => "Greater",
            }
            .to_string(),
        )
    }
}
