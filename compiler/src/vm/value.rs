use super::heap::{Heap, ObjectPointer};
use crate::{
    builtin_functions::BuiltinFunction,
    compiler::{
        hir_to_lir::HirToLir,
        lir::{Instruction, Lir},
    },
    database::Database,
    module::Module,
};
use im::{hashmap, HashMap};
use itertools::Itertools;
use num_bigint::BigInt;
use std::{
    cell::Cell,
    fmt::{self, Display, Formatter},
    rc::Rc,
};

struct Value {
    heap: Rc<Cell<Heap>>,
    address: ObjectPointer,
}

impl Value {
    pub fn nothing() -> Self {
        Value::Symbol("Nothing".to_owned())
    }

    pub fn list(items: Vec<Value>) -> Self {
        let items = items
            .into_iter()
            .enumerate()
            .map(|(index, it)| (Value::Int(BigInt::from(index)), it))
            .collect();
        Value::Struct(items)
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

impl Display for Value {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Value::Int(int) => write!(f, "{int}"),
            Value::Text(text) => write!(f, "{text:?}"),
            Value::Symbol(symbol) => write!(f, "{symbol}"),
            Value::Struct(entries) => write!(
                f,
                "[{}]",
                entries
                    .iter()
                    .map(|(key, value)| (format!("{}", key), value))
                    .sorted_by(|(key_a, _), (key_b, _)| key_a.cmp(key_b))
                    .map(|(key, value)| format!("{}: {}", key, value))
                    .join(", ")
            ),
            Value::Closure(_) => write!(f, "{{…}}"),
            Value::Builtin(builtin) => write!(f, "builtin{builtin:?}"),
        }
    }
}
