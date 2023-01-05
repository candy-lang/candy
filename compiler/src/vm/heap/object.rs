use super::{pointer::Pointer, Heap};
use crate::{
    builtin_functions::BuiltinFunction,
    compiler::{
        hir::Id,
        lir::{Instruction, Lir},
        mir_to_lir::MirToLir,
        TracingConfig,
    },
    database::Database,
    module::Module,
    vm::ids::ChannelId,
};
use itertools::Itertools;
use num_bigint::BigInt;
use std::{
    collections::{hash_map::DefaultHasher, HashMap},
    hash::{Hash, Hasher},
    ops::Deref,
};

#[derive(Clone)]
pub struct Object {
    pub reference_count: usize,
    pub data: Data,
}
#[derive(Clone)]
pub enum Data {
    Int(Int),
    Text(Text),
    Symbol(Symbol),
    List(List),
    Struct(Struct),
    HirId(Id),
    Closure(Closure),
    Builtin(Builtin),
    SendPort(SendPort),
    ReceivePort(ReceivePort),
}

#[derive(Clone)]
pub struct Int {
    pub value: BigInt,
}

#[derive(Clone)]
pub struct Text {
    pub value: String,
}

#[derive(Clone)]
pub struct Symbol {
    // TODO: Choose a more efficient representation.
    pub value: String,
}

#[derive(Default, Clone)]
pub struct List {
    pub items: Vec<Pointer>,
}

#[derive(Default, Clone)]
pub struct Struct {
    pub fields: Vec<(u64, Pointer, Pointer)>, // list of hash, key, and value
}

#[derive(Clone)]
pub struct Closure {
    pub captured: Vec<Pointer>,
    pub num_args: usize,
    pub body: Vec<Instruction>,
}

#[derive(Clone)]
pub struct Builtin {
    pub function: BuiltinFunction,
}

impl List {
    fn equals(&self, heap: &Heap, other: &List) -> bool {
        if self.items.len() != other.items.len() {
            return false;
        }

        self.items
            .iter()
            .zip_eq(other.items.iter())
            .all(|(a, &b)| a.equals(heap, b))
    }
}
impl Struct {
    pub fn from_fields(heap: &Heap, fields: HashMap<Pointer, Pointer>) -> Self {
        let mut s = Self::default();
        for (key, value) in fields {
            s.insert(heap, key, value);
        }
        s
    }
    /// If the struct contains the key, returns the index of its field.
    /// Otherwise, returns the index of where the key would be inserted.
    fn index_of_key(&self, heap: &Heap, key: Pointer, key_hash: u64) -> Result<usize, usize> {
        let index_of_first_hash_occurrence = self
            .fields
            .partition_point(|(existing_hash, _, _)| *existing_hash < key_hash);
        let fields_with_same_hash = self.fields[index_of_first_hash_occurrence..]
            .iter()
            .enumerate()
            .take_while(|(_, (existing_hash, _, _))| *existing_hash == key_hash)
            .map(|(i, (_, key, _))| (index_of_first_hash_occurrence + i, key));

        for (index, existing_key) in fields_with_same_hash {
            if existing_key.equals(heap, key) {
                return Ok(index);
            }
        }
        Err(index_of_first_hash_occurrence)
    }
    fn insert(&mut self, heap: &Heap, key: Pointer, value: Pointer) {
        let hash = key.hash(heap);
        let field = (hash, key, value);
        match self.index_of_key(heap, key, hash) {
            Ok(index) => self.fields[index] = field,
            Err(index) => self.fields.insert(index, field),
        }
    }
    pub fn get(&self, heap: &Heap, key: Pointer) -> Option<Pointer> {
        let index = self.index_of_key(heap, key, key.hash(heap)).ok()?;
        Some(self.fields[index].2)
    }
    fn len(&self) -> usize {
        self.fields.len()
    }
    pub fn iter(&self) -> impl Iterator<Item = (Pointer, Pointer)> {
        self.fields
            .clone()
            .into_iter()
            .map(|(_, key, value)| (key, value))
    }
    fn equals(&self, heap: &Heap, other: &Struct) -> bool {
        if self.len() != other.len() {
            return false;
        }

        self.iter()
            .zip_eq(other.iter())
            .all(|((key_a, value_a), (key_b, value_b))| {
                key_a.equals(heap, key_b) && value_a.equals(heap, value_b)
            })
    }
}

impl Closure {
    pub fn of_module_lir(lir: Lir) -> Self {
        Closure {
            captured: vec![],
            num_args: 0,
            body: lir.instructions,
        }
    }
    pub fn of_module(db: &Database, module: Module, tracing: TracingConfig) -> Option<Self> {
        let lir = db.lir(module, tracing)?;
        Some(Self::of_module_lir((*lir).clone()))
    }
}

#[derive(Clone)]
pub struct SendPort {
    pub channel: ChannelId,
}
#[derive(Clone)]
pub struct ReceivePort {
    pub channel: ChannelId,
}

impl SendPort {
    pub fn new(channel: ChannelId) -> Self {
        Self { channel }
    }
}
impl ReceivePort {
    pub fn new(channel: ChannelId) -> Self {
        Self { channel }
    }
}

impl Data {
    fn hash(&self, heap: &Heap) -> u64 {
        let mut state = DefaultHasher::new();
        self.hash_with_state(heap, &mut state);
        state.finish()
    }

    fn hash_with_state<H: Hasher>(&self, heap: &Heap, state: &mut H) {
        match self {
            Data::Int(int) => int.value.hash(state),
            Data::Text(text) => text.value.hash(state),
            Data::Symbol(symbol) => symbol.value.hash(state),
            Data::List(List { items }) => {
                let mut s = 0;
                for item in items {
                    s ^= item.hash(heap);
                }
                s.hash(state)
            }
            Data::Struct(struct_) => {
                let mut s = 0;
                for (key, value) in struct_.iter() {
                    s ^= key.hash(heap);
                    s ^= value.hash(heap);
                }
                s.hash(state)
            }
            Data::HirId(id) => id.hash(state),
            Data::Closure(closure) => {
                for captured in &closure.captured {
                    captured.hash_with_state(heap, state);
                }
                closure.num_args.hash(state);
                closure.body.hash(state);
            }
            Data::Builtin(builtin) => builtin.function.hash(state),
            Data::SendPort(port) => port.channel.hash(state),
            Data::ReceivePort(port) => port.channel.hash(state),
        }
    }

    pub fn equals(&self, heap: &Heap, other: &Self) -> bool {
        match (self, other) {
            (Data::Int(a), Data::Int(b)) => a.value == b.value,
            (Data::Text(a), Data::Text(b)) => a.value == b.value,
            (Data::Symbol(a), Data::Symbol(b)) => a.value == b.value,
            (Data::List(a), Data::List(b)) => a.equals(heap, b),
            (Data::Struct(a), Data::Struct(b)) => a.equals(heap, b),
            (Data::HirId(a), Data::HirId(b)) => a == b,
            (Data::Closure(_), Data::Closure(_)) => false,
            (Data::Builtin(a), Data::Builtin(b)) => a.function == b.function,
            (Data::SendPort(a), Data::SendPort(b)) => a.channel == b.channel,
            (Data::ReceivePort(a), Data::ReceivePort(b)) => a.channel == b.channel,
            _ => false,
        }
    }

    pub fn children(&self) -> Vec<Pointer> {
        match self {
            Data::Int(_)
            | Data::Text(_)
            | Data::Symbol(_)
            | Data::Builtin(_)
            | Data::HirId(_)
            | Data::SendPort(_)
            | Data::ReceivePort(_) => vec![],
            Data::List(List { items }) => items.clone(),
            Data::Struct(struct_) => struct_
                .iter()
                .flat_map(|(a, b)| vec![a, b].into_iter())
                .collect_vec(),
            Data::Closure(closure) => closure.captured.clone(),
        }
    }

    pub fn channel(&self) -> Option<ChannelId> {
        if let Data::SendPort(SendPort { channel }) | Data::ReceivePort(ReceivePort { channel }) =
            self
        {
            Some(*channel)
        } else {
            None
        }
    }
}

impl Deref for Object {
    type Target = Data;

    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

impl Pointer {
    fn hash(&self, heap: &Heap) -> u64 {
        heap.get(*self).hash(heap)
    }
    fn hash_with_state<H: Hasher>(&self, heap: &Heap, state: &mut H) {
        heap.get(*self).hash_with_state(heap, state)
    }

    pub fn equals(&self, heap: &Heap, other: Self) -> bool {
        if *self == other {
            return true;
        }
        heap.get(*self).equals(heap, heap.get(other))
    }

    pub fn format(&self, heap: &Heap) -> String {
        self.format_helper(heap, false)
    }
    pub fn format_debug(&self, heap: &Heap) -> String {
        self.format_helper(heap, true)
    }
    fn format_helper(&self, heap: &Heap, is_debug: bool) -> String {
        match &heap.get(*self).data {
            Data::Int(int) => format!("{}", int.value),
            Data::Text(text) => format!("\"{}\"", text.value),
            Data::Symbol(symbol) => symbol.value.to_string(),
            Data::List(List { items }) => format!(
                "({})",
                if items.is_empty() {
                    ",".to_owned()
                } else {
                    items.iter().map(|item| item.format(heap)).join(", ")
                },
            ),
            Data::Struct(struct_) => format!(
                "[{}]",
                struct_
                    .iter()
                    .map(|(key, value)| (key.format(heap), value.format(heap)))
                    .sorted_by(|(key_a, _), (key_b, _)| key_a.cmp(key_b))
                    .map(|(key, value)| format!("{}: {}", key, value))
                    .join(", ")
            ),
            Data::HirId(id) => format!("{id:?}"),
            Data::Closure(_) => {
                if is_debug {
                    format!("{{{self}}}")
                } else {
                    "{…}".to_string()
                }
            }
            Data::Builtin(builtin) => format!("builtin{:?}", builtin.function),
            Data::SendPort(port) => format!("sendPort {:?}", port.channel),
            Data::ReceivePort(port) => format!("receivePort {:?}", port.channel),
        }
    }
}

macro_rules! impl_data_try_into_type {
    ($type:ty, $variant:tt, $error_message:expr$(,)?) => {
        impl TryInto<$type> for Data {
            type Error = String;

            fn try_into(self) -> Result<$type, Self::Error> {
                match self {
                    Data::$variant(it) => Ok(it),
                    _ => Err($error_message.to_string()),
                }
            }
        }
        impl<'a> TryInto<&'a $type> for &'a Data {
            type Error = String;

            fn try_into(self) -> Result<&'a $type, Self::Error> {
                match &self {
                    Data::$variant(it) => Ok(it),
                    _ => Err($error_message.to_string()),
                }
            }
        }
    };
}
impl_data_try_into_type!(Int, Int, "Expected an int.");
impl_data_try_into_type!(Text, Text, "Expected a text.");
impl_data_try_into_type!(Symbol, Symbol, "Expected a symbol.");
impl_data_try_into_type!(List, List, "Expected a list.");
impl_data_try_into_type!(Struct, Struct, "Expected a struct.");
impl_data_try_into_type!(Id, HirId, "Expected a HIR ID.");
impl_data_try_into_type!(Closure, Closure, "Expected a closure.");
impl_data_try_into_type!(SendPort, SendPort, "Expected a send port.");
impl_data_try_into_type!(ReceivePort, ReceivePort, "Expected a receive port.");

impl TryInto<bool> for &Data {
    type Error = String;

    fn try_into(self) -> Result<bool, Self::Error> {
        let symbol: &Symbol = self.try_into()?;
        match symbol.value.as_str() {
            "True" => Ok(true),
            "False" => Ok(false),
            _ => Err("Expected `True` or `False`.".to_string()),
        }
    }
}
