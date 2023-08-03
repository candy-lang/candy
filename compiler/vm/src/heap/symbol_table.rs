use derive_more::From;
use std::{
    borrow::Cow,
    cmp::{self, Ordering},
    fmt::{self, Display, Formatter},
    intrinsics,
};

#[derive(Clone, Debug)]
pub struct SymbolTable {
    symbols: Vec<String>,
}

impl SymbolTable {
    pub fn get(&self, id: SymbolId) -> &str {
        &self.symbols[id.0]
    }
    pub fn find_or_add(&mut self, symbol: impl Into<Cow<str>>) -> SymbolId {
        let symbol: Cow<str> = symbol.into();
        if let Some(index) = self.symbols.iter().position(|it| it == symbol.as_ref()) {
            return SymbolId(index);
        }
        let id = SymbolId(self.symbols.len());
        self.symbols.push(symbol.into_owned());
        id
    }

    pub fn choose(&self, rng: &mut impl Rng) -> SymbolId {
        SymbolId(rng.gen_range(0..self.symbols.len()))
    }
}

impl AsRef<[String]> for SymbolTable {
    fn as_ref(&self) -> &[String] {
        &self.symbols
    }
}

impl Default for SymbolTable {
    fn default() -> Self {
        // These symbols must be kept in sync with the `SymbolId` constants.
        Self {
            symbols: vec![
                "Builtin".to_string(),
                "Equal".to_string(),
                "Error".to_string(),
                "False".to_string(),
                "Function".to_string(),
                "Greater".to_string(),
                "Int".to_string(),
                "Less".to_string(),
                "List".to_string(),
                "Main".to_string(),
                "Nothing".to_string(),
                "Ok".to_string(),
                "ReceivePort".to_string(),
                "ReturnChannel".to_string(),
                "SendPort".to_string(),
                "Stdin".to_string(),
                "Stdout".to_string(),
                "Struct".to_string(),
                "Tag".to_string(),
                "Text".to_string(),
                "True".to_string(),
            ],
        }
    }
}

#[derive(Copy, Clone, Debug, Eq, From, Hash, PartialEq)]
pub struct SymbolId(usize);
impl SymbolId {
    // These symbols are created by built-in functions or used for starting the
    // program (main and environment keys). They have a fixed ID so that they
    // can be used in the VM without lookups.
    //
    // Sorted alphabetically and must be kept in sync with
    // `SymbolTable::default()`.
    pub const BUILTIN: SymbolId = SymbolId(0);
    pub const EQUAL: SymbolId = SymbolId(1);
    pub const ERROR: SymbolId = SymbolId(2);
    pub const FALSE: SymbolId = SymbolId(3);
    pub const FUNCTION: SymbolId = SymbolId(4);
    pub const GREATER: SymbolId = SymbolId(5);
    pub const INT: SymbolId = SymbolId(6);
    pub const LESS: SymbolId = SymbolId(7);
    pub const LIST: SymbolId = SymbolId(8);
    pub const MAIN: SymbolId = SymbolId(9);
    pub const NOTHING: SymbolId = SymbolId(10);
    pub const OK: SymbolId = SymbolId(11);
    pub const RECEIVE_PORT: SymbolId = SymbolId(12);
    pub const RETURN_CHANNEL: SymbolId = SymbolId(13);
    pub const SEND_PORT: SymbolId = SymbolId(14);
    pub const STDIN: SymbolId = SymbolId(15);
    pub const STDOUT: SymbolId = SymbolId(16);
    pub const STRUCT: SymbolId = SymbolId(17);
    pub const TAG: SymbolId = SymbolId(18);
    pub const TEXT: SymbolId = SymbolId(19);
    pub const TRUE: SymbolId = SymbolId(20);

    pub fn value(self) -> usize {
        self.0
    }
}

pub trait DisplayWithSymbolTable {
    fn to_string(&self, symbol_table: &SymbolTable) -> String {
        let mut buffer = String::new();
        let mut formatter = fmt::Formatter::new(&mut buffer);
        self.fmt(&mut formatter, symbol_table).unwrap();
        buffer
    }
    fn fmt(&self, f: &mut Formatter, symbol_table: &SymbolTable) -> fmt::Result;
}
impl<T: Display> DisplayWithSymbolTable for T {
    fn fmt(&self, f: &mut Formatter, _symbol_table: &SymbolTable) -> fmt::Result {
        Display::fmt(&self, f)
    }
}

pub trait OrdWithSymbolTable {
    fn cmp(&self, symbol_table: &SymbolTable, other: &Self) -> Ordering;
}
impl<T: OrdWithSymbolTable> OrdWithSymbolTable for Option<T> {
    fn cmp(&self, symbol_table: &SymbolTable, other: &Self) -> Ordering {
        match (self, other) {
            (Option::None, Option::None) => Ordering::Equal,
            (Option::Some(this), Option::Some(other)) => this.cmp(symbol_table, other),
            _ => intrinsics::discriminant_value(self).cmp(&intrinsics::discriminant_value(other)),
        }
    }
}
impl<T0: OrdWithSymbolTable, T1: OrdWithSymbolTable> OrdWithSymbolTable for (T0, T1) {
    fn cmp(&self, symbol_table: &SymbolTable, other: &Self) -> Ordering {
        self.0
            .cmp(symbol_table, &other.0)
            .then_with(|| self.1.cmp(symbol_table, &other.1))
    }
}
impl<T: OrdWithSymbolTable> OrdWithSymbolTable for [T] {
    fn cmp(&self, symbol_table: &SymbolTable, other: &Self) -> Ordering {
        let l = cmp::min(self.len(), other.len());

        // Slice to the loop iteration range to enable bound check
        // elimination in the compiler
        let lhs = &self[..l];
        let rhs = &other[..l];

        for i in 0..l {
            match lhs[i].cmp(symbol_table, &rhs[i]) {
                Ordering::Equal => (),
                non_eq => return non_eq,
            }
        }

        self.len().cmp(&other.len())
    }
}

macro_rules! impl_ops_with_symbol_table_via_ops {
    ($type:ty) => {
        impl crate::heap::OrdWithSymbolTable for $type {
            fn cmp(
                &self,
                _symbol_table: &crate::heap::SymbolTable,
                other: &Self,
            ) -> std::cmp::Ordering {
                Ord::cmp(self, other)
            }
        }
    };
}
pub(crate) use impl_ops_with_symbol_table_via_ops;
use rand::Rng;
