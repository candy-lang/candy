use derive_more::From;
use rand::Rng;
use std::{
    borrow::Cow,
    cmp::{self, Ordering},
    fmt::{self, Debug, Display, Formatter},
    intrinsics,
};

#[derive(Clone, Debug)]
pub struct SymbolTable {
    symbols: Vec<String>,
}

impl SymbolTable {
    #[must_use]
    pub fn get(&self, id: SymbolId) -> &str {
        &self.symbols[id.0]
    }
    #[must_use]
    pub fn find_or_add(&mut self, symbol: impl Into<Cow<str>>) -> SymbolId {
        let symbol: Cow<str> = symbol.into();
        if let Some(index) = self.symbols.iter().position(|it| it == symbol.as_ref()) {
            return SymbolId(index);
        }
        let id = SymbolId(self.symbols.len());
        self.symbols.push(symbol.into_owned());
        id
    }

    #[must_use]
    pub fn symbols(&self) -> &[String] {
        &self.symbols
    }
    pub fn ids_and_symbols(&self) -> impl Iterator<Item = (SymbolId, &str)> {
        self.symbols
            .iter()
            .enumerate()
            .map(|(index, it)| (SymbolId(index), it.as_str()))
    }
    #[must_use]
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

#[derive(Copy, Clone, Eq, From, Hash, Ord, PartialEq, PartialOrd)]
pub struct SymbolId(usize);
impl SymbolId {
    // These symbols are created by built-in functions or used for starting the
    // program (main and environment keys). They have a fixed ID so that they
    // can be used in the VM without lookups.
    //
    // Sorted alphabetically and must be kept in sync with
    // `SymbolTable::default()`.
    pub const BUILTIN: Self = Self(0);
    pub const EQUAL: Self = Self(1);
    pub const ERROR: Self = Self(2);
    pub const FALSE: Self = Self(3);
    pub const FUNCTION: Self = Self(4);
    pub const GREATER: Self = Self(5);
    pub const INT: Self = Self(6);
    pub const LESS: Self = Self(7);
    pub const LIST: Self = Self(8);
    pub const MAIN: Self = Self(9);
    pub const NOTHING: Self = Self(10);
    pub const OK: Self = Self(11);
    pub const STDIN: Self = Self(12);
    pub const STDOUT: Self = Self(13);
    pub const STRUCT: Self = Self(14);
    pub const TAG: Self = Self(15);
    pub const TEXT: Self = Self(16);
    pub const TRUE: Self = Self(17);

    #[must_use]
    pub const fn value(self) -> usize {
        self.0
    }
}

impl Debug for SymbolId {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "<symbol-id {}>", self.0)
    }
}
impl DisplayWithSymbolTable for SymbolId {
    fn fmt(&self, f: &mut Formatter, symbol_table: &SymbolTable) -> fmt::Result {
        write!(f, "{}", symbol_table.get(*self))
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
            (None, None) => Ordering::Equal,
            (Some(this), Some(other)) => this.cmp(symbol_table, other),
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

macro_rules! impl_ord_with_symbol_table_via_ord {
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
pub(crate) use impl_ord_with_symbol_table_via_ord;
