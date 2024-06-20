use crate::ast::AstString;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Hir {
    pub assignments: Vec<(Box<str>, Assignment)>,
}
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum Assignment {
    Value {
        value: Expression,
        type_: Type,
    },
    Function {
        parameters: Box<[Parameter]>,
        return_type: Type,
        body: Body,
    },
}
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Parameter {
    pub name: Box<str>,
    pub type_: Type,
}

#[derive(Clone, Debug, Default, Eq, Hash, PartialEq)]
pub struct Body {
    pub expressions: Vec<(Option<Box<str>>, Expression, Type)>,
}
impl Body {
    pub fn return_type(&self) -> &Type {
        self.expressions.last().map_or(&Type::Error, |it| &it.2)
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum Expression {
    Symbol(Box<str>),
    Int(i64),
    Text(Box<str>),
    Struct(Box<[(Box<str>, Expression)]>),
    StructAccess {
        struct_: Box<Expression>,
        field: Box<str>,
    },
    ValueWithTypeAnnotation {
        value: Box<Expression>,
        type_: Type,
    },
    ParameterReference(Box<str>),
    Lambda {
        parameters: Box<[Parameter]>,
        body: Body,
    },
    // Reference(Id),
    Call {
        receiver: Box<Expression>,
        arguments: Box<[Expression]>,
    },
    BuiltinEquals,
    BuiltinPrint,
    BuiltinTextConcat,
    BuiltinToDebugText,
    Type(Type),
    Error,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum Type {
    Type,
    Symbol, // TODO: specific symbol as separate type
    Int,
    Text,
    Struct(Box<[(Box<str>, Type)]>),
    Function {
        parameter_types: Box<[Type]>,
        return_type: Box<Type>,
    },
    Error,
}
