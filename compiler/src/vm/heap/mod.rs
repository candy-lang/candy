mod object;
mod pointer;

pub use self::{
    object::{
        Builtin, Closure, Data, Int, List, Object, ReceivePort, SendPort, Struct, Symbol, Text,
    },
    pointer::Pointer,
};
use super::ids::ChannelId;
use crate::{builtin_functions::BuiltinFunction, compiler::hir::Id};
use itertools::Itertools;
use num_bigint::BigInt;
use std::{cmp::Ordering, collections::HashMap};

const TRACE: bool = false;

macro_rules! trace {
    ($format_string:tt, $($args:expr,)+) => {
        if TRACE {
            tracing::trace!($format_string, $($args),+)
        }
    };
    ($format_string:tt, $($args:expr),+) => {
        if TRACE {
            tracing::trace!($format_string, $($args),+)
        }
    };
    ($format_string:tt) => {
        if TRACE {
            tracing::trace!($format_string)
        }
    };
}

#[derive(Clone)]
pub struct Heap {
    objects: HashMap<Pointer, Object>,
    channel_refcounts: HashMap<ChannelId, usize>,
    next_address: Pointer,
}

impl std::fmt::Debug for Heap {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut objects = self.objects.iter().collect_vec();
        objects.sort_by_key(|(address, _)| address.raw());

        writeln!(f, "{{")?;
        for (address, object) in objects {
            writeln!(
                f,
                "{address}: {} {}",
                object.reference_count,
                address.format_debug(self),
            )?;
        }
        write!(f, "}}")
    }
}

impl Default for Heap {
    fn default() -> Self {
        Self {
            objects: HashMap::new(),
            channel_refcounts: HashMap::new(),
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
    pub fn get_hir_id(&self, address: Pointer) -> Id {
        let Data::HirId(id) = &self.get(address).data else { panic!(); };
        id.clone()
    }

    pub fn dup(&mut self, address: Pointer) {
        self.dup_by(address, 1);
    }
    pub fn dup_by(&mut self, address: Pointer, amount: usize) {
        let object = self.get_mut(address);
        object.reference_count += amount;
        let new_reference_count = object.reference_count;

        if let Some(channel) = object.data.channel() {
            *self
                .channel_refcounts
                .entry(channel)
                .or_insert_with(|| panic!("Called `dup_by` on a channel that doesn't exist.")) +=
                amount;
        };

        trace!(
            "RefCount of {address} increased to {}. Value: {}",
            new_reference_count,
            address.format_debug(self),
        );
    }
    pub fn drop(&mut self, address: Pointer) {
        trace!(
            "RefCount of {address} reduced to {}. Value: {}",
            self.get(address).reference_count - 1,
            address.format(self),
        );

        let object = self.get_mut(address);

        object.reference_count -= 1;
        let new_reference_count = object.reference_count;

        if let Some(channel) = object.data.channel() {
            let channel_refcount = self
                .channel_refcounts
                .entry(channel)
                .or_insert_with(|| panic!("Called `drop` on a channel that doesn't exist."));
            *channel_refcount -= 1;
            if *channel_refcount == 0 {
                self.channel_refcounts.remove(&channel).unwrap();
            }
        };

        if new_reference_count == 0 {
            self.free(address);
        }
    }

    pub fn create(&mut self, object: Data) -> Pointer {
        let address = self.reserve_address();
        let object = Object {
            reference_count: 1,
            data: object,
        };
        trace!("Creating object at {address}.");
        self.objects.insert(address, object);
        address
    }
    fn reserve_address(&mut self) -> Pointer {
        let address = self.next_address;
        self.next_address = Pointer::from_raw(self.next_address.raw() + 1);
        address
    }
    fn free(&mut self, address: Pointer) {
        let object = self.objects.remove(&address).unwrap();
        trace!("Freeing object at {address}.");
        assert_eq!(object.reference_count, 0);
        for child in object.children() {
            self.drop(child);
        }
    }

    /// Clones all objects at the `root_addresses` into the `other` heap and
    /// returns a list of their addresses in the other heap.
    pub fn clone_multiple_to_other_heap_with_existing_mapping(
        &self,
        other: &mut Heap,
        addresses: &[Pointer],
        address_map: &mut HashMap<Pointer, Pointer>,
    ) -> Vec<Pointer> {
        let mut additional_refcounts = HashMap::new();
        for address in addresses {
            self.prepare_object_cloning(address_map, &mut additional_refcounts, other, *address);
        }

        for object in additional_refcounts.keys() {
            address_map
                .entry(*object)
                .or_insert_with(|| other.reserve_address());
        }

        for (address, refcount) in additional_refcounts {
            other
                .objects
                .entry(address_map[&address])
                .or_insert_with(|| Object {
                    reference_count: 0,
                    data: Self::map_data(address_map, &self.get(address).data),
                })
                .reference_count += refcount;
        }

        addresses
            .iter()
            .map(|address| address_map[address])
            .collect()
    }
    fn prepare_object_cloning(
        &self,
        address_map: &mut HashMap<Pointer, Pointer>,
        additional_refcounts: &mut HashMap<Pointer, usize>,
        other: &mut Heap,
        address: Pointer,
    ) {
        *additional_refcounts.entry(address).or_default() += 1;

        let is_new = !address_map.contains_key(&address);
        if is_new {
            address_map.insert(address, other.reserve_address());
            for child in self.get(address).children() {
                self.prepare_object_cloning(address_map, additional_refcounts, other, child);
            }
        }
    }
    fn map_data(address_map: &HashMap<Pointer, Pointer>, data: &Data) -> Data {
        match data {
            Data::Int(int) => Data::Int(int.clone()),
            Data::Text(text) => Data::Text(text.clone()),
            Data::Symbol(symbol) => Data::Symbol(symbol.clone()),
            Data::List(List { items }) => Data::List(List {
                items: items.iter().map(|item| address_map[item]).collect(),
            }),
            Data::Struct(struct_) => Data::Struct(Struct {
                fields: struct_
                    .fields
                    .iter()
                    .map(|(hash, key, value)| (*hash, address_map[key], address_map[value]))
                    .collect(),
            }),
            Data::HirId(id) => Data::HirId(id.clone()),
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
            Data::SendPort(port) => Data::SendPort(SendPort::new(port.channel)),
            Data::ReceivePort(port) => Data::ReceivePort(ReceivePort::new(port.channel)),
        }
    }
    pub fn clone_single_to_other_heap_with_existing_mapping(
        &self,
        other: &mut Heap,
        address: Pointer,
        address_map: &mut HashMap<Pointer, Pointer>,
    ) -> Pointer {
        self.clone_multiple_to_other_heap_with_existing_mapping(other, &[address], address_map)
            .pop()
            .unwrap()
    }
    pub fn clone_single_to_other_heap(&self, other: &mut Heap, address: Pointer) -> Pointer {
        self.clone_single_to_other_heap_with_existing_mapping(other, address, &mut HashMap::new())
    }

    pub fn known_channels(&self) -> impl IntoIterator<Item = ChannelId> + '_ {
        self.channel_refcounts.keys().copied()
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
    pub fn create_hir_id(&mut self, id: Id) -> Pointer {
        self.create(Data::HirId(id))
    }
    pub fn create_closure(&mut self, closure: Closure) -> Pointer {
        self.create(Data::Closure(closure))
    }
    pub fn create_builtin(&mut self, builtin: BuiltinFunction) -> Pointer {
        self.create(Data::Builtin(Builtin { function: builtin }))
    }
    pub fn create_send_port(&mut self, channel: ChannelId) -> Pointer {
        self.channel_refcounts
            .entry(channel)
            .and_modify(|count| *count += 1)
            .or_insert(1);
        self.create(Data::SendPort(SendPort::new(channel)))
    }
    pub fn create_receive_port(&mut self, channel: ChannelId) -> Pointer {
        self.channel_refcounts
            .entry(channel)
            .and_modify(|count| *count += 1)
            .or_insert(1);
        self.create(Data::ReceivePort(ReceivePort::new(channel)))
    }
    pub fn create_nothing(&mut self) -> Pointer {
        self.create_symbol("Nothing".to_string())
    }
    pub fn create_list(&mut self, items: Vec<Pointer>) -> Pointer {
        self.create(Data::List(List { items }))
    }
    pub fn create_bool(&mut self, value: bool) -> Pointer {
        self.create_symbol(if value { "True" } else { "False" }.to_string())
    }
    pub fn create_result(&mut self, result: Result<Pointer, Pointer>) -> Pointer {
        let (type_, value) = match result {
            Ok(it) => ("Ok".to_string(), it),
            Err(it) => ("Error".to_string(), it),
        };
        let fields = HashMap::from([
            (
                self.create_symbol("Type".to_string()),
                self.create_symbol(type_),
            ),
            (self.create_symbol("Value".to_string()), value),
        ]);
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
