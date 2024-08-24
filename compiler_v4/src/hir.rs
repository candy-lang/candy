use crate::{id::CountableId, impl_countable_id};
use derive_more::Deref;
use itertools::Itertools;
use rustc_hash::FxHashMap;
use std::fmt::{self, Display, Formatter};
use strum::VariantArray;

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
    pub assignments: Box<[(Id, Box<str>, Assignment)]>,
    pub functions: Box<[(Id, Box<str>, Function)]>,
    pub main_function_id: Id,
}
impl Hir {
    #[must_use]
    pub fn get_assignment(&self, id: Id) -> (&str, &Assignment) {
        let (_, name, assignment) = self.assignments.iter().find(|(i, _, _)| *i == id).unwrap();
        (name, assignment)
    }
    #[must_use]
    pub fn get_function(&self, id: Id) -> (&str, &Function) {
        let (_, name, function) = self.functions.iter().find(|(i, _, _)| *i == id).unwrap();
        (name, function)
    }
    // /// `None` means the ID belongs to a parameter.
    // #[must_use]
    // pub fn get(&self, id: Id) -> Option<&Assignment> {
    //     self.assignments.iter().find_map(|(i, _, assignment)| {
    //         if i == &id {
    //             Some(assignment)
    //         } else {
    //             assignment.get(id)
    //         }
    //     })
    // }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum TypeDeclaration {
    Struct {
        type_parameters: Box<[TypeParameter]>,
        fields: Box<[(Box<str>, Type)]>,
    },
    Enum {
        type_parameters: Box<[TypeParameter]>,
        variants: Box<[(Box<str>, Option<Type>)]>,
    },
}
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct TypeParameter {
    pub id: TypeParameterId,
    pub name: Box<str>,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum Type {
    Named {
        name: Box<str>,
        type_arguments: Box<[Type]>,
    },
    Parameter {
        name: Box<str>,
        id: TypeParameterId,
    },
    // TODO: `Self` type
    Error,
}
impl Type {
    // Builtin types
    #[must_use]
    pub fn int() -> Self {
        Self::Named {
            name: "Int".into(),
            type_arguments: [].into(),
        }
    }
    #[must_use]
    pub fn text() -> Self {
        Self::Named {
            name: "Text".into(),
            type_arguments: [].into(),
        }
    }

    // Standard library types
    #[must_use]
    pub fn nothing() -> Self {
        Self::Named {
            name: "Nothing".into(),
            type_arguments: [].into(),
        }
    }
    #[must_use]
    pub fn never() -> Self {
        Self::Named {
            name: "Never".into(),
            type_arguments: [].into(),
        }
    }
    #[must_use]
    pub fn ordering() -> Self {
        Self::Named {
            name: "Ordering".into(),
            type_arguments: [].into(),
        }
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
            Self::Named { name, type_arguments } => Self::Named {
                name: name.clone(),
                type_arguments: type_arguments.iter().map(|it| it.substitute(environment)).collect(),
            },
            Self::Parameter { name, id } => environment.get(id).unwrap_or_else(|| panic!("Missing substitution for type parameter {name} (environment: {environment:?})")).clone(),
            Self::Error => Self::Error,
        }
    }
}
impl Display for Type {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match &self {
            Self::Named {
                name,
                type_arguments,
            } => {
                write!(f, "{name}")?;
                if !type_arguments.is_empty() {
                    write!(f, "[{}]", type_arguments.iter().join(", "))?;
                }
                Ok(())
            }
            Self::Parameter { name, id: _ } => write!(f, "{name}"),
            Self::Error => write!(f, "<error>"),
        }
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Assignment {
    pub type_: Type,
    pub body: Body,
}
// impl Definition {
//     #[must_use]
//     pub fn type_of(&self, id: Id) -> Option<Cow<Type>> {
//         match self {
//             Definition::Value { value, .. } => value.type_of(id),
//             Definition::Function { body, .. } => match body {
//                 BodyOrBuiltin::Body(body) => body.type_of(id),
//                 BodyOrBuiltin::Builtin(_) => None,
//             },
//         }
//     }
//     // #[must_use]
//     // pub fn get(&self, id: Id) -> Option<&Assignment> {
//     //     match self {
//     //         Self::Value { value,.. } => value.get(id),
//     //         Self::Function { body, .. } => body.get(id),
//     //     }
//     // }
// }

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Function {
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
    // #[must_use]
    // pub fn type_of(&self, id: Id) -> Option<Cow<Type>> {
    //     self.expressions
    //         .iter()
    //         .find_map(|(i, _, expression, type_)| {
    //             if i == &id {
    //                 Some(Cow::Borrowed(type_))
    //             } else {
    //                 expression.type_of(id)
    //             }
    //         })
    // }
    // #[must_use]
    // pub fn get(&self, id: Id) -> Option<&Assignment> {
    //     match self {
    //         Body::Builtin(_) => None,
    //         Body::Written { expressions } => {
    //             expressions.iter().find_map(|(i, _, expression, _)| {
    //                 if i == &id {
    //                     Some(expression)
    //                 } else {
    //                     expression.get(id)
    //                 }
    //             })
    //         }
    //     }
    // }

    // fn collect_defined_and_referenced_ids(
    //     &self,
    //     defined_ids: &mut FxHashSet<Id>,
    //     referenced_ids: &mut FxHashSet<Id>,
    // ) {
    //     for (id, _, expression, _) in &self.expressions {
    //         defined_ids.insert(*id);
    //         match expression {
    //             Expression::Int(_)
    //             | Expression::Text(_)
    //             | Expression::Tag { .. }
    //             | Expression::Struct(_)
    //             | Expression::StructAccess { .. }
    //             | Expression::ValueWithTypeAnnotation { .. }
    //             | Expression::Reference(_)
    //             | Expression::Call { .. }
    //             | Expression::Or { .. }
    //             | Expression::CreateOrVariant { .. }
    //             | Expression::Type(_)
    //             | Expression::Error => {}
    //             Expression::Lambda(Lambda { parameters, body }) => {
    //                 defined_ids.extend(parameters.iter().map(|it| it.id));
    //                 body.collect_defined_and_referenced_ids(defined_ids, referenced_ids);
    //             }
    //         }
    //     }
    // }
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
// impl Expression {
//     #[must_use]
//     pub fn type_of(&self, id: Id) -> Option<Cow<Type>> {
//         match self {
//             Expression::Int(_)
//             | Expression::Text(_)
//             | Expression::Tag { .. }
//             | Expression::Struct(_)
//             | Expression::StructAccess { .. }
//             | Expression::ValueWithTypeAnnotation { .. }
//             | Expression::Reference(_)
//             | Expression::Call { .. }
//             | Expression::Or { .. }
//             | Expression::CreateOrVariant { .. }
//             | Expression::Type(_)
//             | Expression::Error => None,
//             Expression::Lambda(Lambda { parameters, body }) => {
//                 if let Some(parameter) = parameters.iter().find(|it| it.id == id) {
//                     return Some(Cow::Borrowed(&parameter.type_));
//                 }

//                 body.type_of(id)
//             }
//         }
//     }
// }

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
                parameters: [("a".into(), Type::int()), ("b".into(), Type::int())].into(),
                return_type: Type::int(),
            },
            Self::IntCompareTo => BuiltinFunctionSignature {
                name: "compareTo".into(),
                type_parameters: Box::default(),
                parameters: [("a".into(), Type::int()), ("b".into(), Type::int())].into(),
                return_type: Type::ordering(),
            },
            Self::IntSubtract => BuiltinFunctionSignature {
                name: "subtract".into(),
                type_parameters: Box::default(),
                parameters: [("a".into(), Type::int()), ("b".into(), Type::int())].into(),
                return_type: Type::int(),
            },
            Self::IntToText => BuiltinFunctionSignature {
                name: "toText".into(),
                type_parameters: Box::default(),
                parameters: [("int".into(), Type::int())].into(),
                return_type: Type::text(),
            },
            Self::Panic => BuiltinFunctionSignature {
                name: "panic".into(),
                type_parameters: Box::default(),
                parameters: [("message".into(), Type::text())].into(),
                return_type: Type::never(),
            },
            Self::Print => BuiltinFunctionSignature {
                name: "print".into(),
                type_parameters: Box::default(),
                parameters: [("message".into(), Type::text())].into(),
                return_type: Type::nothing(),
            },
            Self::TextConcat => BuiltinFunctionSignature {
                name: "concat".into(),
                type_parameters: Box::default(),
                parameters: [("a".into(), Type::text()), ("b".into(), Type::text())].into(),
                return_type: Type::text(),
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
