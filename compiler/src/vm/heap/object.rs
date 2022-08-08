use super::{pointer::Pointer, Heap};
use crate::{
    builtin_functions::BuiltinFunction,
    compiler::{
        hir_to_lir::HirToLir,
        lir::{Instruction, Lir},
    },
    database::Database,
    module::Module,
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
    Closure(Closure),
    Builtin(Builtin),
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
    // TODO: Choose a more efficient representation.
    pub fields: Vec<(u64, Pointer, Pointer)>, // hash, key, and value
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
    fn insert(&mut self, heap: &Heap, key: Pointer, value: Pointer) {
        let hash = key.hash(heap);
        for (existing_hash, existing_key, existing_value) in &mut self.fields {
            if hash != *existing_hash {
                continue;
            }
            if existing_key.equals(heap, key) {
                *existing_value = value;
                return;
            }
        }
        let entry = (hash, key, value);
        if let Some(index) = self
            .fields
            .iter()
            .position(|(the_hash, _, _)| *the_hash > hash)
        {
            self.fields.insert(index, entry);
        } else {
            self.fields.push(entry);
        }
    }
    pub fn get(&self, heap: &Heap, key: Pointer) -> Option<Pointer> {
        let hash = key.hash(heap);
        Some(
            self.fields
                .iter()
                .find(|(field_hash, field_key, _)| {
                    hash == *field_hash && key.equals(heap, *field_key)
                })?
                .2,
        )
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
                if key_a.equals(heap, key_b) && !value_a.equals(heap, value_b) {
                    return false;
                }
            }
        }
        true
    }
}

impl Closure {
    pub fn of_lir(module: Module, lir: Lir) -> Self {
        Closure {
            captured: vec![],
            num_args: 0,
            body: vec![
                Instruction::TraceModuleStarts { module },
                Instruction::CreateClosure {
                    captured: vec![],
                    num_args: 0,
                    body: lir.instructions,
                },
                Instruction::Call { num_args: 0 },
                Instruction::TraceModuleEnds,
                Instruction::Return,
            ],
        }
    }
    pub fn of_module(db: &Database, module: Module) -> Option<Self> {
        let lir = db.lir(module.clone())?;
        Some(Self::of_lir(module, (*lir).clone()))
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
            Data::Closure(closure) => {
                for captured in &closure.captured {
                    captured.hash_with_state(heap, state);
                }
                closure.num_args.hash(state);
                closure.body.hash(state);
            }
            Data::Builtin(builtin) => builtin.function.hash(state),
        }
    }

    pub fn equals(&self, heap: &Heap, other: &Self) -> bool {
        match (self, other) {
            (Data::Int(a), Data::Int(b)) => a.value == b.value,
            (Data::Text(a), Data::Text(b)) => a.value == b.value,
            (Data::Symbol(a), Data::Symbol(b)) => a.value == b.value,
            (Data::Struct(a), Data::Struct(b)) => a.equals(heap, b),
            (Data::Closure(_), Data::Closure(_)) => false,
            (Data::Builtin(a), Data::Builtin(b)) => a.function == b.function,
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
                    .fields
                    .iter()
                    .map(|(_, key, value)| (key.format(heap), value.format(heap)))
                    .sorted_by(|(key_a, _), (key_b, _)| key_a.cmp(key_b))
                    .map(|(key, value)| format!("{}: {}", key, value))
                    .join(", ")
            ),
            Data::Closure(_) => "{...}".to_string(),
            Data::Builtin(builtin) => format!("builtin{:?}", builtin.function),
        }
    }

    pub fn children(&self) -> Vec<Pointer> {
        match self {
            Data::Int(_) | Data::Text(_) | Data::Symbol(_) | Data::Builtin(_) => vec![],
            Data::Struct(struct_) => struct_
                .iter()
                .flat_map(|(a, b)| vec![a, b].into_iter())
                .collect_vec(),
            Data::Closure(closure) => closure.captured.clone(),
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
