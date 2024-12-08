use crate::{hir::BuiltinFunction, impl_countable_id};
use derive_more::Deref;
use rustc_hash::{FxHashMap, FxHashSet};
use std::fmt::{self, Display, Formatter};

#[derive(Clone, Copy, Debug, Default, Deref, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Id(usize);
impl_countable_id!(Id);
impl Display for Id {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "_{}", self.0)
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Mono {
    pub type_declarations: FxHashMap<Box<str>, TypeDeclaration>,
    pub assignments: FxHashMap<Box<str>, Assignment>,
    pub assignment_initialization_order: Box<[Box<str>]>,
    pub functions: FxHashMap<Box<str>, Function>,
    pub main_function: Box<str>,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum TypeDeclaration {
    Builtin {
        name: Box<str>,
        type_arguments: Box<[Box<str>]>,
    },
    Struct {
        fields: Box<[(Box<str>, Box<str>)]>,
    },
    Enum {
        variants: Box<[EnumVariant]>,
    },
    Function {
        parameter_types: Box<[Box<str>]>,
        return_type: Box<str>,
    },
}
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct EnumVariant {
    pub name: Box<str>,
    pub value_type: Option<Box<str>>,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Assignment {
    pub type_: Box<str>,
    pub body: Body,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Function {
    pub parameters: Box<[Parameter]>,
    pub return_type: Box<str>,
    pub body: BodyOrBuiltin,
}
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Parameter {
    pub id: Id,
    pub name: Box<str>,
    pub type_: Box<str>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BodyOrBuiltin {
    Body(Body),
    Builtin {
        builtin_function: BuiltinFunction,
        substitutions: FxHashMap<Box<str>, Box<str>>,
    },
}
#[derive(Clone, Debug, Default, Eq, Hash, PartialEq)]
pub struct Body {
    pub expressions: Vec<(Id, Option<Box<str>>, Expression)>,
}
impl Body {
    #[must_use]
    pub fn return_value_id(&self) -> Id {
        self.expressions.last().unwrap().0
    }
    #[must_use]
    pub fn return_type(&self) -> &str {
        &self.expressions.last().unwrap().2.type_
    }

    fn collect_defined_and_referenced_ids(
        &self,
        defined_ids: &mut FxHashSet<Id>,
        referenced_ids: &mut FxHashSet<Id>,
    ) {
        for (id, _, expression) in &self.expressions {
            defined_ids.insert(*id);
            match &expression.kind {
                ExpressionKind::Int(_) | ExpressionKind::Text(_) => {}
                ExpressionKind::CreateStruct { fields, .. } => {
                    defined_ids.extend(fields.iter());
                }
                ExpressionKind::StructAccess { struct_, .. } => {
                    referenced_ids.insert(*struct_);
                }
                ExpressionKind::CreateEnum { value, .. } => {
                    referenced_ids.extend(value.iter());
                }
                ExpressionKind::GlobalAssignmentReference(_) => {}
                ExpressionKind::LocalReference(referenced_id) => {
                    referenced_ids.insert(*referenced_id);
                }
                ExpressionKind::CallFunction { arguments, .. } => {
                    referenced_ids.extend(arguments.iter());
                }
                ExpressionKind::CallLambda {
                    lambda, arguments, ..
                } => {
                    referenced_ids.insert(*lambda);
                    referenced_ids.extend(arguments.iter());
                }
                ExpressionKind::Switch { value, .. } => {
                    referenced_ids.insert(*value);
                }
                ExpressionKind::Lambda(Lambda { parameters, body }) => {
                    defined_ids.extend(parameters.iter().map(|it| it.id));
                    body.collect_defined_and_referenced_ids(defined_ids, referenced_ids);
                }
            }
        }
    }
    #[must_use]
    pub fn find_expression(&self, id: Id) -> Option<&Expression> {
        self.expressions.iter().find_map(|(it_id, _, expression)| {
            if *it_id == id {
                return Some(expression);
            }

            match &expression.kind {
                ExpressionKind::Switch { cases, .. } => {
                    cases.iter().find_map(|it| it.body.find_expression(id))
                }
                ExpressionKind::Lambda(Lambda { body, .. }) => body.find_expression(id),
                _ => None,
            }
        })
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Expression {
    pub kind: ExpressionKind,
    pub type_: Box<str>,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum ExpressionKind {
    Int(i64),
    Text(Box<str>),
    CreateStruct {
        struct_: Box<str>,
        fields: Box<[Id]>,
    },
    StructAccess {
        struct_: Id,
        field: Box<str>,
    },
    CreateEnum {
        enum_: Box<str>,
        variant: Box<str>,
        value: Option<Id>,
    },
    GlobalAssignmentReference(Box<str>),
    LocalReference(Id),
    CallFunction {
        function: Box<str>,
        arguments: Box<[Id]>,
    },
    CallLambda {
        lambda: Id,
        arguments: Box<[Id]>,
    },
    Switch {
        value: Id,
        enum_: Box<str>,
        cases: Box<[SwitchCase]>,
    },
    Lambda(Lambda),
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct SwitchCase {
    pub variant: Box<str>,
    pub value_id: Option<Id>,
    pub body: Body,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Lambda {
    pub parameters: Box<[Parameter]>,
    pub body: Body,
}
impl Lambda {
    #[must_use]
    pub fn closure_with_types(&self, function_body: &Body) -> FxHashMap<Id, Box<str>> {
        self.closure()
            .into_iter()
            .map(|id| (id, function_body.find_expression(id).unwrap().type_.clone()))
            .collect()
    }
    #[must_use]
    pub fn closure(&self) -> FxHashSet<Id> {
        let mut defined_ids = self.parameters.iter().map(|it| it.id).collect();
        let mut referenced_ids = FxHashSet::default();
        self.body
            .collect_defined_and_referenced_ids(&mut defined_ids, &mut referenced_ids);
        referenced_ids.retain(|id| !defined_ids.contains(id));
        referenced_ids
    }
}
