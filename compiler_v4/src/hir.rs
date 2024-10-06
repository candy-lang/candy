use crate::{
    id::CountableId,
    impl_countable_id,
    to_text::{TextBuilder, ToText},
};
use derive_more::{Deref, From};
use extension_trait::extension_trait;
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
impl ToText for Id {
    fn build_text(&self, builder: &mut TextBuilder) {
        builder.push(format!("${self}"));
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
impl ToText for Hir {
    fn build_text(&self, builder: &mut TextBuilder) {
        for (name, declaration) in self
            .type_declarations
            .iter()
            .sorted_by_key(|(name, _)| *name)
        {
            match &declaration.kind {
                TypeDeclarationKind::Struct { fields } => {
                    builder.push(format!("struct {name}"));
                    declaration.type_parameters.build_text(builder);
                    builder.push(" {");
                    builder.push_children_custom_multiline(
                        fields.iter(),
                        |builder, (name, type_)| {
                            builder.push(format!("{name}: {type_},"));
                        },
                    );
                    if !fields.is_empty() {
                        builder.push_newline();
                    }
                    builder.push("}");
                }
                TypeDeclarationKind::Enum { variants } => {
                    builder.push(format!("enum {name}"));
                    declaration.type_parameters.build_text(builder);
                    builder.push(" {");
                    builder.push_children_custom_multiline(
                        variants.iter(),
                        |builder, (name, type_)| {
                            builder.push(name);
                            if let Some(type_) = type_ {
                                builder.push(format!(": {type_}"));
                            }
                            builder.push(",");
                        },
                    );
                    if !variants.is_empty() {
                        builder.push_newline();
                    }
                    builder.push("}");
                }
                TypeDeclarationKind::Trait { functions } => {
                    builder.push(format!("trait {name}"));
                    declaration.type_parameters.build_text(builder);
                    builder.push(" {");
                    builder.push_children_custom_multiline(
                        functions
                            .iter()
                            .sorted_by_key(|(_, it)| it.signature.name.clone()),
                        |builder, (id, function)| (*id, *function).build_text(builder),
                    );
                    builder.push("}");
                }
            }
            builder.push_newline();
        }
        for impl_ in self.impls.iter() {
            impl_.build_text(builder);
            builder.push_newline();
        }

        for (id, assignment) in self
            .assignments
            .iter()
            .sorted_by_key(|(_, it)| it.name.clone())
        {
            (id, assignment).build_text(builder);
            builder.push_newline();
        }
        for (id, function) in self
            .functions
            .iter()
            .sorted_by_key(|(_, it)| it.signature.name.clone())
        {
            (id, function).build_text(builder);
            builder.push_newline();
        }

        builder.push(format!("main: ${}", self.main_function_id));
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypeDeclaration {
    pub type_parameters: Box<[TypeParameter]>,
    pub kind: TypeDeclarationKind,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TypeDeclarationKind {
    Struct {
        fields: Box<[(Box<str>, Type)]>,
    },
    Enum {
        variants: Box<[(Box<str>, Option<Type>)]>,
    },
    Trait {
        functions: FxHashMap<Id, TraitFunction>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TraitFunction {
    pub signature: FunctionSignature,
    pub body: Option<BodyOrBuiltin>,
}
impl ToText for (&Id, &TraitFunction) {
    fn build_text(&self, builder: &mut TextBuilder) {
        builder.push(format!("{}@", self.0));
        self.1.signature.build_text(builder);
        if let Some(body) = &self.1.body {
            builder.push(" ");
            match body {
                BodyOrBuiltin::Body(body) => body.build_text(builder),
                BodyOrBuiltin::Builtin(builtin_function) => {
                    builder.push(format!("= {builtin_function:?}"));
                }
            }
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Impl {
    pub type_parameters: Box<[TypeParameter]>,
    pub type_: Type,
    pub trait_: Type,
    pub functions: FxHashMap<Id, Function>,
}
impl ToText for Impl {
    fn build_text(&self, builder: &mut TextBuilder) {
        builder.push("impl");
        self.type_parameters.build_text(builder);
        builder.push(format!(" {}: {} {{", self.type_, self.trait_));
        builder.push_children_custom_multiline(
            self.functions
                .iter()
                .sorted_by_key(|(_, it)| it.signature.name.clone()),
            |builder, (id, function)| {
                (*id, *function).build_text(builder);
            },
        );
        builder.push("}");
    }
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
#[extension_trait]
pub impl SliceOfTypeParameter for [TypeParameter] {
    #[must_use]
    fn type_(&self) -> Box<[Type]> {
        self.iter().map(|it| it.type_().into()).collect()
    }
}
impl ToText for [TypeParameter] {
    fn build_text(&self, builder: &mut TextBuilder) {
        if self.is_empty() {
            return;
        }

        builder.push("[");
        builder.push_children(self, ", ");
        builder.push("]");
    }
}
impl ToText for TypeParameter {
    fn build_text(&self, builder: &mut TextBuilder) {
        builder.push(format!("{}@{}", self.id, self.name));
        if let Some(upper_bound) = self.upper_bound.as_ref() {
            builder.push(format!(": {upper_bound}"));
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
    pub fn nothing() -> Self {
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Assignment {
    pub name: Box<str>,
    pub type_: Type,
    pub body: Body,
}
impl ToText for (&Id, &Assignment) {
    fn build_text(&self, builder: &mut TextBuilder) {
        builder.push(format!("{}@{}: {} = ", self.0, self.1.name, self.1.type_));
        self.1.body.build_text(builder);
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Function {
    pub signature: FunctionSignature,
    pub body: BodyOrBuiltin,
}
impl ToText for (&Id, &Function) {
    fn build_text(&self, builder: &mut TextBuilder) {
        builder.push(format!("{}@", self.0));
        self.1.signature.build_text(builder);
        builder.push(" ");
        match &self.1.body {
            BodyOrBuiltin::Body(body) => body.build_text(builder),
            BodyOrBuiltin::Builtin(builtin_function) => {
                builder.push(format!("= {builtin_function:?}"));
            }
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FunctionSignature {
    pub name: Box<str>,
    pub type_parameters: Box<[TypeParameter]>,
    pub parameters: Box<[Parameter]>,
    pub return_type: Type,
}
impl ToText for (&Id, &FunctionSignature) {
    fn build_text(&self, builder: &mut TextBuilder) {
        builder.push(format!("{}@", self.0));
        self.1.build_text(builder);
    }
}
impl ToText for FunctionSignature {
    fn build_text(&self, builder: &mut TextBuilder) {
        builder.push(format!("{}", self.name));
        self.type_parameters.build_text(builder);
        builder.push("(");
        builder.push_children_custom(
            self.parameters.iter(),
            |builder, parameter| {
                builder.push(format!("{}: {}", parameter.name, parameter.type_));
            },
            ", ",
        );
        builder.push(format!(") {}", self.return_type));
    }
}
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Parameter {
    pub id: Id,
    pub name: Box<str>,
    pub type_: Type,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BodyOrBuiltin {
    Body(Body),
    Builtin(BuiltinFunction),
}
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Body {
    pub expressions: Vec<(Id, Option<Box<str>>, Expression)>,
}
impl Body {
    pub fn return_value_id(&self) -> Id {
        self.expressions.last().unwrap().0
    }
}
impl ToText for Body {
    fn build_text(&self, builder: &mut TextBuilder) {
        builder.push("{");
        builder.push_children_custom_multiline(
            self.expressions.iter(),
            |builder, (id, name, expression)| {
                id.build_text(builder);
                builder.push(format!(": {} = ", expression.type_));
                if let Some(name) = name {
                    builder.push(format!("{name} = "));
                }
                expression.kind.build_text(builder);
            },
        );
        builder.push_newline();
        builder.push("}");
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
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

#[derive(Clone, Debug, Eq, PartialEq)]
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
        substitutions: FxHashMap<TypeParameterId, Type>,
        arguments: Box<[Id]>,
    },
    Switch {
        value: Id,
        enum_: Type,
        cases: Box<[SwitchCase]>,
    },
    Error,
}
impl ToText for ExpressionKind {
    fn build_text(&self, builder: &mut TextBuilder) {
        match self {
            Self::Int(value) => builder.push(value.to_string()),
            Self::Text(value) => builder.push(format!("\"{value}\"")),
            Self::CreateStruct { struct_, fields } => {
                builder.push(format!("{struct_} {{"));
                builder.push_children(fields.iter(), ", ");
                builder.push("}");
            }
            Self::StructAccess { struct_, field } => {
                builder.push(format!("{struct_}.{field}"));
            }
            Self::CreateEnum {
                enum_,
                variant,
                value,
            } => {
                builder.push(format!("{enum_}.{variant}"));
                if let Some(value) = value {
                    builder.push(format!("({value})"));
                }
            }
            Self::Reference(id) => builder.push(format!("${id}")),
            Self::Call {
                function,
                substitutions,
                arguments,
            } => {
                function.build_text(builder);
                if !substitutions.is_empty() {
                    builder.push("[");
                    builder.push_children_custom(
                        substitutions.iter(),
                        |builder, (id, type_)| {
                            builder.push(format!("{id}: {type_}"));
                        },
                        ", ",
                    );
                    builder.push("]");
                }
                builder.push("(");
                builder.push_children(arguments.iter(), ", ");
                builder.push(")");
            }
            Self::Switch {
                value,
                enum_,
                cases,
            } => {
                builder.push("switch ");
                value.build_text(builder);
                builder.push(format!(": {enum_} {{"));
                builder.push_children_custom_multiline(cases.iter(), |builder, case| {
                    builder.push(format!("case {}", case.variant));
                    if let Some(value_id) = case.value_id {
                        builder.push(format!("({value_id})"));
                    }
                    builder.push(" {");
                    builder.push_children_custom_multiline(
                        case.body.expressions.iter(),
                        |builder, (id, name, expression)| {
                            id.build_text(builder);
                            builder.push(format!(": {} = ", expression.type_));
                            if let Some(name) = name {
                                builder.push(format!("{name} = "));
                            }
                            expression.kind.build_text(builder);
                        },
                    );
                    builder.push("}");
                });
                builder.push("}");
            }
            Self::Error => builder.push("<error>"),
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
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
