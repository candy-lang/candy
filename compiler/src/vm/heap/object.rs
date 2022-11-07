use super::{pointer::Pointer, Heap};
use crate::{
    builtin_functions::BuiltinFunction,
    compiler::{
        hir::Id,
        hir_to_mir::MirConfig,
        lir::{Instruction, Lir},
        mir_to_lir::MirToLir,
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
    Struct(Struct),
    Responsibility(Responsibility),
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
pub struct Struct {
    pub fields: Vec<(u64, Pointer, Pointer)>, // list of hash, key, and value
}

#[derive(Clone, Hash, Debug)]
pub struct Responsibility {
    pub id: Id,
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
        for (key_a, value_a) in self.iter() {
            for (key_b, value_b) in other.iter() {
                if !key_a.equals(heap, key_b) || !value_a.equals(heap, value_b) {
                    return false;
                }
            }
        }
        true
    }
}

impl Closure {
    pub fn of_module_lir(module: Module, lir: Lir) -> Self {
        Closure {
            captured: vec![],
            num_args: 0,
            body: vec![
                Instruction::ModuleStarts {
                    module: module.clone(),
                },
                Instruction::CreateClosure {
                    captured: vec![],
                    num_args: 0,
                    body: lir.instructions,
                },
                Instruction::CreateResponsibility {
                    id: Id::new(module, vec![]),
                },
                Instruction::Call { num_args: 0 },
                Instruction::ModuleEnds,
                Instruction::Return,
            ],
        }
    }
    pub fn of_module(db: &Database, module: Module, config: MirConfig) -> Option<Self> {
        let lir = db.lir(module.clone(), config)?;
        Some(Self::of_module_lir(module, (*lir).clone()))
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
            Data::Struct(struct_) => {
                let mut s = 0;
                for (key, value) in struct_.iter() {
                    s ^= key.hash(heap);
                    s ^= value.hash(heap);
                }
                s.hash(state)
            }
            Data::Responsibility(responsibility) => responsibility.hash(state),
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
            (Data::Struct(a), Data::Struct(b)) => a.equals(heap, b),
            (Data::Responsibility(a), Data::Responsibility(b)) => a.id == b.id,
            (Data::Closure(_), Data::Closure(_)) => false,
            (Data::Builtin(a), Data::Builtin(b)) => a.function == b.function,
            (Data::SendPort(a), Data::SendPort(b)) => a.channel == b.channel,
            (Data::ReceivePort(a), Data::ReceivePort(b)) => a.channel == b.channel,
            _ => false,
        }
    }

    pub fn format(&self, heap: &Heap) -> String {
        match self {
            Data::Int(int) => format!("{}", int.value),
            Data::Text(text) => format!("\"{}\"", text.value),
            Data::Symbol(symbol) => symbol.value.to_string(),
            Data::Struct(struct_) => format!(
                "[{}]",
                struct_
                    .iter()
                    .map(|(key, value)| (key.format(heap), value.format(heap)))
                    .sorted_by(|(key_a, _), (key_b, _)| key_a.cmp(key_b))
                    .map(|(key, value)| format!("{}: {}", key, value))
                    .join(", ")
            ),
            Data::Responsibility(id) => format!("{id:?}"),
            Data::Closure(_) => "{…}".to_string(),
            Data::Builtin(builtin) => format!("builtin{:?}", builtin.function),
            Data::SendPort(port) => format!("sendPort {:?}", port.channel),
            Data::ReceivePort(port) => format!("receivePort {:?}", port.channel),
        }
    }

    pub fn children(&self) -> Vec<Pointer> {
        match self {
            Data::Int(_)
            | Data::Text(_)
            | Data::Symbol(_)
            | Data::Builtin(_)
            | Data::Responsibility(_)
            | Data::SendPort(_)
            | Data::ReceivePort(_) => vec![],
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
        heap.get(*self).format(heap)
    }
}
