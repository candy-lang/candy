use crate::{id::CountableId, impl_countable_id};
use derive_more::{Deref, From};
use itertools::Itertools;
use rustc_hash::FxHashMap;
use std::fmt::{self, Display, Formatter};
use strum::VariantArray;

// TODO: split assignment/function and expression IDs
#[derive(Clone, Copy, Debug, Default, Deref, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Id(usize);
impl_countable_id!(Id);
impl Display for Id {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "_{}", self.0)
    }
}

#[derive(Clone, Copy, Debug, Default, Deref, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TypeParameterId(usize);
impl_countable_id!(TypeParameterId);
impl Display for TypeParameterId {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "_{}", self.0)
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Hir {
    pub type_declarations: FxHashMap<Box<str>, TypeDeclaration>,
    pub impls: Box<[Impl]>,
    pub assignments: FxHashMap<Id, Assignment>,
    pub functions: FxHashMap<Id, Function>,
    pub main_function_id: Id,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct TypeDeclaration {
    pub type_parameters: Box<[TypeParameter]>,
    pub kind: TypeDeclarationKind,
}
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum TypeDeclarationKind {
    Struct {
        fields: Box<[(Box<str>, Type)]>,
    },
    Enum {
        variants: Box<[(Box<str>, Option<Type>)]>,
    },
    Trait {
        functions: Box<[(Id, Box<str>, Function)]>,
    },
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Impl {
    pub type_parameters: Box<[TypeParameter]>,
    pub type_: Type,
    pub trait_: Type,
    pub functions: Box<[Function]>,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct TypeParameter {
    pub id: TypeParameterId,
    pub name: Box<str>,
    pub upper_bound: Option<Box<Type>>,
}
impl TypeParameter {
    #[must_use]
    pub fn type_(&self) -> ParameterType {
        ParameterType {
            name: self.name.clone(),
            id: self.id,
        }
    }
}

#[derive(Clone, Debug, Eq, From, Hash, PartialEq)]
pub enum Type {
    // TODO: encode ADT, trait, or builtin type here
    #[from]
    Named(NamedType),
    #[from]
    Parameter(ParameterType),
    Self_ {
        base_type: NamedType,
    },
    Error,
}
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct NamedType {
    pub name: Box<str>,
    pub type_arguments: Box<[Type]>,
}
impl NamedType {
    // Builtin types
    #[must_use]
    pub fn int() -> Self {
        Self {
            name: "Int".into(),
            type_arguments: [].into(),
        }
    }
    #[must_use]
    pub fn text() -> Self {
        Self {
            name: "Text".into(),
            type_arguments: [].into(),
        }
    }

    // Standard library types
    #[must_use]
    pub fn nothing() -> Self {
        Self {
            name: "Nothing".into(),
            type_arguments: [].into(),
        }
    }
    #[must_use]
    pub fn never() -> Self {
        Self {
            name: "Never".into(),
            type_arguments: [].into(),
        }
    }
    #[must_use]
    pub fn ordering() -> Self {
        Self {
            name: "Ordering".into(),
            type_arguments: [].into(),
        }
    }
}
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct ParameterType {
    pub name: Box<str>,
    pub id: TypeParameterId,
}
impl Display for ParameterType {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "{}", self.name)
    }
}
impl Type {
    pub fn nothing() -> Type {
        NamedType::nothing().into()
    }

    #[must_use]
    pub fn build_environment(
        type_parameters: &[TypeParameter],
        type_arguments: &[Self],
    ) -> FxHashMap<TypeParameterId, Self> {
        type_parameters
            .iter()
            .map(|it| it.id)
            .zip_eq(type_arguments.iter().cloned())
            .collect()
    }
    #[must_use]
    pub fn substitute(&self, environment: &FxHashMap<TypeParameterId, Self>) -> Self {
        match self {
            Self::Named(NamedType { name, type_arguments }) => Self::Named(NamedType {
                name: name.clone(),
                type_arguments: type_arguments.iter().map(|it| it.substitute(environment)).collect(),
            }),
            Self::Parameter (ParameterType{ name, id }) => environment.get(id).unwrap_or_else(|| panic!("Missing substitution for type parameter {name} (environment: {environment:?})")).clone(),
            Self::Self_ { base_type } => Self::Self_ { base_type: base_type.clone() },
            Self::Error => Self::Error,
        }
    }
}
impl Display for Type {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match &self {
            Self::Named(type_) => write!(f, "{type_}"),
            Self::Parameter(ParameterType { name, id: _ }) => write!(f, "{name}"),
            Self::Self_ { base_type } => write!(f, "Self<{base_type}>"),
            Self::Error => write!(f, "<error>"),
        }
    }
}
impl Display for NamedType {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name)?;
        if !self.type_arguments.is_empty() {
            write!(f, "[{}]", self.type_arguments.iter().join(", "))?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Assignment {
    pub name: Box<str>,
    pub type_: Type,
    pub body: Body,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Function {
    pub name: Box<str>,
    pub type_parameters: Box<[TypeParameter]>,
    pub parameters: Box<[Parameter]>,
    pub return_type: Type,
    pub body: BodyOrBuiltin,
}
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Parameter {
    pub id: Id,
    pub name: Box<str>,
    pub type_: Type,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum BodyOrBuiltin {
    Body(Body),
    Builtin(BuiltinFunction),
}
#[derive(Clone, Debug, Default, Eq, Hash, PartialEq)]
pub struct Body {
    pub expressions: Vec<(Id, Option<Box<str>>, Expression)>,
}
impl Body {
    pub fn return_value_id(&self) -> Id {
        self.expressions.last().unwrap().0
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Expression {
    pub kind: ExpressionKind,
    pub type_: Type,
}
impl Expression {
    #[must_use]
    pub fn nothing() -> Self {
        Self {
            kind: ExpressionKind::CreateStruct {
                struct_: Type::nothing(),
                fields: [].into(),
            },
            type_: Type::nothing(),
        }
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum ExpressionKind {
    Int(i64),
    Text(Box<str>),
    CreateStruct {
        struct_: Type,
        fields: Box<[Id]>,
    },
    StructAccess {
        struct_: Id,
        field: Box<str>,
    },
    CreateEnum {
        enum_: Type,
        variant: Box<str>,
        value: Option<Id>,
    },
    Reference(Id),
    Call {
        function: Id,
        type_arguments: Box<[Type]>,
        arguments: Box<[Id]>,
    },
    Switch {
        value: Id,
        enum_: Type,
        cases: Box<[SwitchCase]>,
    },
    Error,
}
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct SwitchCase {
    pub variant: Box<str>,
    pub value_id: Option<Id>,
    pub body: Body,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, VariantArray)]
#[strum(serialize_all = "camelCase")]
pub enum BuiltinFunction {
    IntAdd,
    IntCompareTo,
    IntSubtract,
    IntToText,
    Panic,
    Print,
    TextConcat,
}
impl BuiltinFunction {
    #[must_use]
    pub fn id(self) -> Id {
        Id::from_usize(self as usize)
    }

    #[must_use]
    pub fn signature(self) -> BuiltinFunctionSignature {
        match self {
            Self::IntAdd => BuiltinFunctionSignature {
                name: "add".into(),
                type_parameters: Box::default(),
                parameters: [
                    ("a".into(), NamedType::int().into()),
                    ("b".into(), NamedType::int().into()),
                ]
                .into(),
                return_type: NamedType::int().into(),
            },
            Self::IntCompareTo => BuiltinFunctionSignature {
                name: "compareTo".into(),
                type_parameters: Box::default(),
                parameters: [
                    ("a".into(), NamedType::int().into()),
                    ("b".into(), NamedType::int().into()),
                ]
                .into(),
                return_type: NamedType::ordering().into(),
            },
            Self::IntSubtract => BuiltinFunctionSignature {
                name: "subtract".into(),
                type_parameters: Box::default(),
                parameters: [
                    ("a".into(), NamedType::int().into()),
                    ("b".into(), NamedType::int().into()),
                ]
                .into(),
                return_type: NamedType::int().into(),
            },
            Self::IntToText => BuiltinFunctionSignature {
                name: "toText".into(),
                type_parameters: Box::default(),
                parameters: [("int".into(), NamedType::int().into())].into(),
                return_type: NamedType::text().into(),
            },
            Self::Panic => BuiltinFunctionSignature {
                name: "panic".into(),
                type_parameters: Box::default(),
                parameters: [("message".into(), NamedType::text().into())].into(),
                return_type: NamedType::never().into(),
            },
            Self::Print => BuiltinFunctionSignature {
                name: "print".into(),
                type_parameters: Box::default(),
                parameters: [("message".into(), NamedType::text().into())].into(),
                return_type: NamedType::nothing().into(),
            },
            Self::TextConcat => BuiltinFunctionSignature {
                name: "concat".into(),
                type_parameters: Box::default(),
                parameters: [
                    ("a".into(), NamedType::text().into()),
                    ("b".into(), NamedType::text().into()),
                ]
                .into(),
                return_type: NamedType::text().into(),
            },
        }
    }
}
#[derive(Debug)]
pub struct BuiltinFunctionSignature {
    pub name: Box<str>,
    pub type_parameters: Box<[Box<str>]>,
    pub parameters: Box<[(Box<str>, Type)]>,
    pub return_type: Type,
}
