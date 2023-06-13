use super::{body::Body, id::Id};
use crate::{
    builtin_functions::BuiltinFunction,
    hir, impl_display_via_richir,
    module::Module,
    rich_ir::{ReferenceKey, RichIrBuilder, ToRichIr, TokenModifier, TokenType},
};
use core::mem;
use derive_more::{From, TryInto};
use enumset::EnumSet;
use itertools::Itertools;
use num_bigint::BigInt;
use rustc_hash::{FxHashSet, FxHasher};
use std::{
    cmp::Ordering,
    hash::{Hash, Hasher},
};

#[derive(Clone, Debug, Eq, From, PartialEq, TryInto)]
pub enum Expression {
    #[from]
    #[try_into]
    Int(BigInt),
    #[from]
    #[try_into]
    Text(String),
    Tag {
        symbol: String,
        value: Option<Id>,
    },
    #[from]
    Builtin(BuiltinFunction),
    #[from]
    List(Vec<Id>),
    Struct(Vec<(Id, Id)>),
    Reference(Id),
    /// A HIR ID that can be used to refer to code in the HIR.
    #[from]
    HirId(hir::Id),
    /// In the MIR, responsibilities are explicitly tracked. All functions take
    /// a responsible HIR ID as an extra parameter. Based on whether the
    /// function is fuzzable or not, this parameter may be used to dynamically
    /// determine who's at fault if some `needs` is not fulfilled.
    Function {
        original_hirs: FxHashSet<hir::Id>,
        parameters: Vec<Id>,
        responsible_parameter: Id,
        body: Body,
    },
    /// This expression is never contained in an actual MIR body, but when
    /// dealing with expressions, its easier to not special-case IDs referring
    /// to parameters.
    Parameter,
    Call {
        function: Id,
        arguments: Vec<Id>,
        responsible: Id,
    },
    UseModule {
        current_module: Module,
        relative_path: Id,
        responsible: Id,
    },
    /// This expression indicates that the code will panic. It's created in the
    /// generated `needs` function or if the compiler can statically determine
    /// that some expression will always panic.
    Panic {
        reason: Id,
        responsible: Id,
    },

    /// For convenience when writing optimization passes, this expression allows
    /// storing multiple inner expressions in a single expression. The expansion
    /// back into multiple expressions happens in the [multiple flattening]
    /// optimization.
    ///
    /// [multiple flattening]: crate::mir_optimize::multiple_flattening
    #[from]
    Multiple(Body),

    TraceCallStarts {
        hir_call: Id,
        function: Id,
        arguments: Vec<Id>,
        responsible: Id,
    },
    TraceCallEnds {
        return_value: Id,
    },
    TraceExpressionEvaluated {
        hir_expression: Id,
        value: Id,
    },
    TraceFoundFuzzableFunction {
        hir_definition: Id,
        function: Id,
    },
}

impl Expression {
    pub fn tag(symbol: String) -> Self {
        Expression::Tag {
            symbol,
            value: None,
        }
    }
    pub fn nothing() -> Self {
        Self::tag("Nothing".to_string())
    }
}

// Int
impl From<i32> for Expression {
    fn from(value: i32) -> Self {
        Self::Int(value.into())
    }
}
impl From<u64> for Expression {
    fn from(value: u64) -> Self {
        Self::Int(value.into())
    }
}
impl From<usize> for Expression {
    fn from(value: usize) -> Self {
        Self::Int(value.into())
    }
}
impl<'a> TryInto<&'a BigInt> for &'a Expression {
    type Error = ();

    fn try_into(self) -> Result<&'a BigInt, ()> {
        let Expression::Int(int) = self else { return Err(()); };
        Ok(int)
    }
}
// Text
impl<'a> From<&'a str> for Expression {
    fn from(value: &'a str) -> Self {
        Self::Text(value.to_string())
    }
}
impl<'a> TryInto<&'a str> for &'a Expression {
    type Error = ();

    fn try_into(self) -> Result<&'a str, ()> {
        let Expression::Text(text) = self else { return Err(()); };
        Ok(text)
    }
}
// Tag
impl From<bool> for Expression {
    fn from(value: bool) -> Self {
        Self::tag(if value { "True" } else { "False" }.to_string())
    }
}
impl TryInto<bool> for &Expression {
    type Error = ();

    fn try_into(self) -> Result<bool, ()> {
        let Expression::Tag { symbol, .. } = self else { return Err(()); };
        match symbol.as_str() {
            "True" => Ok(true),
            "False" => Ok(false),
            _ => Err(()),
        }
    }
}
impl From<Ordering> for Expression {
    fn from(value: Ordering) -> Self {
        let symbol = match value {
            Ordering::Less => "Less",
            Ordering::Equal => "Equal",
            Ordering::Greater => "Greater",
        };
        Self::tag(symbol.to_string())
    }
}
impl From<Result<Id, Id>> for Expression {
    fn from(value: Result<Id, Id>) -> Self {
        let (symbol, value) = match value {
            Ok(it) => ("Ok", it),
            Err(it) => ("Error", it),
        };
        Expression::Tag {
            symbol: symbol.to_string(),
            value: Some(value),
        }
    }
}
// Referene
impl From<&Id> for Expression {
    fn from(value: &Id) -> Self {
        Self::Reference(*value)
    }
}

#[allow(clippy::derived_hash_with_manual_eq)]
impl Hash for Expression {
    fn hash<H: Hasher>(&self, state: &mut H) {
        mem::discriminant(self).hash(state);
        match self {
            Expression::Int(int) => int.hash(state),
            Expression::Text(text) => text.hash(state),
            Expression::Tag { symbol, value } => {
                symbol.hash(state);
                value.hash(state);
            }
            Expression::Builtin(builtin) => builtin.hash(state),
            Expression::List(items) => items.hash(state),
            Expression::Struct(fields) => fields.len().hash(state),
            Expression::Reference(id) => id.hash(state),
            Expression::HirId(id) => id.hash(state),
            Expression::Function {
                original_hirs,
                parameters,
                responsible_parameter,
                body,
            } => {
                {
                    let mut hash = 0;
                    for id in original_hirs {
                        let mut state = FxHasher::default();
                        id.hash(&mut state);
                        hash ^= state.finish();
                    }
                    hash.hash(state);
                }
                parameters.hash(state);
                responsible_parameter.hash(state);
                body.hash(state);
            }
            Expression::Parameter => {}
            Expression::Call {
                function,
                arguments,
                responsible,
            } => {
                function.hash(state);
                arguments.hash(state);
                responsible.hash(state);
            }
            Expression::UseModule {
                current_module,
                relative_path,
                responsible,
            } => {
                current_module.hash(state);
                relative_path.hash(state);
                responsible.hash(state);
            }
            Expression::Panic {
                reason,
                responsible,
            } => {
                reason.hash(state);
                responsible.hash(state);
            }
            Expression::Multiple(body) => body.hash(state),
            Expression::TraceCallStarts {
                hir_call,
                function,
                arguments,
                responsible,
            } => {
                hir_call.hash(state);
                function.hash(state);
                arguments.hash(state);
                responsible.hash(state);
            }
            Expression::TraceCallEnds { return_value } => return_value.hash(state),
            Expression::TraceExpressionEvaluated {
                hir_expression,
                value,
            } => {
                hir_expression.hash(state);
                value.hash(state);
            }
            Expression::TraceFoundFuzzableFunction {
                hir_definition,
                function,
            } => {
                hir_definition.hash(state);
                function.hash(state);
            }
        }
    }
}
impl_display_via_richir!(Expression);
impl ToRichIr for Expression {
    fn build_rich_ir(&self, builder: &mut RichIrBuilder) {
        match self {
            Expression::Int(int) => {
                let range = builder.push(int.to_string(), TokenType::Int, EnumSet::empty());
                builder.push_reference(int.to_owned(), range);
            }
            Expression::Text(text) => {
                let range =
                    builder.push(format!(r#""{}""#, text), TokenType::Text, EnumSet::empty());
                builder.push_reference(text.to_owned(), range);
            }
            Expression::Tag { symbol, value } => {
                let range = builder.push(symbol, TokenType::Symbol, EnumSet::empty());
                builder.push_reference(ReferenceKey::Symbol(symbol.to_owned()), range);
                if let Some(value) = value {
                    builder.push(" ", None, EnumSet::empty());
                    value.build_rich_ir(builder);
                }
            }
            Expression::Builtin(builtin) => {
                let range = builder.push(
                    format!("builtin{builtin:?}"),
                    TokenType::Function,
                    EnumSet::only(TokenModifier::Builtin),
                );
                builder.push_reference(*builtin, range);
            }
            Expression::List(items) => {
                builder.push("(", None, EnumSet::empty());
                builder.push_children(items, ", ");
                builder.push(")", None, EnumSet::empty());
            }
            Expression::Struct(fields) => {
                builder.push("[", None, EnumSet::empty());
                builder.push_children_custom(
                    fields.iter().collect_vec(),
                    |builder, (key, value)| {
                        key.build_rich_ir(builder);
                        builder.push(": ", None, EnumSet::empty());
                        value.build_rich_ir(builder);
                    },
                    ", ",
                );
                builder.push("]", None, EnumSet::empty());
            }
            Expression::Reference(id) => id.build_rich_ir(builder),
            Expression::HirId(id) => {
                let range = builder.push(id.to_string(), TokenType::Symbol, EnumSet::empty());
                builder.push_reference(id.to_owned(), range);
            }
            Expression::Function {
                // IDs are displayed by the body before the entire expression
                // assignment.
                original_hirs: _,
                parameters,
                responsible_parameter,
                body,
            } => {
                builder.push("{ ", None, EnumSet::empty());
                builder.push_children_custom(
                    parameters,
                    |builder, parameter| {
                        let range = builder.push(
                            parameter.to_short_debug_string(),
                            TokenType::Parameter,
                            EnumSet::empty(),
                        );
                        builder.push_definition(*parameter, range);
                    },
                    " ",
                );
                builder.push(
                    if parameters.is_empty() {
                        "(responsible "
                    } else {
                        " (+ responsible "
                    },
                    None,
                    EnumSet::empty(),
                );
                let range = builder.push(
                    responsible_parameter.to_short_debug_string(),
                    TokenType::Parameter,
                    EnumSet::empty(),
                );
                builder.push_definition(*responsible_parameter, range);
                builder.push(") ->", None, EnumSet::empty());
                builder.push_foldable(|builder| {
                    builder.indent();
                    builder.push_newline();
                    body.build_rich_ir(builder);
                    builder.dedent();
                    builder.push_newline();
                });
                builder.push("}", None, EnumSet::empty());
            }
            Expression::Parameter => {
                builder.push("parameter", None, EnumSet::empty());
            }
            Expression::Call {
                function,
                arguments,
                responsible,
            } => {
                builder.push("call ", None, EnumSet::empty());
                function.build_rich_ir(builder);
                builder.push(" with ", None, EnumSet::empty());
                if arguments.is_empty() {
                    builder.push("no arguments", None, EnumSet::empty());
                } else {
                    builder.push_children(arguments, " ");
                }
                builder.push(" (", None, EnumSet::empty());
                responsible.build_rich_ir(builder);
                builder.push(" is responsible)", None, EnumSet::empty());
            }
            Expression::UseModule {
                current_module,
                relative_path,
                responsible,
            } => {
                builder.push("use ", None, EnumSet::empty());
                relative_path.build_rich_ir(builder);
                builder.push(" (relative to ", None, EnumSet::empty());
                current_module.build_rich_ir(builder);
                builder.push("; ", None, EnumSet::empty());
                responsible.build_rich_ir(builder);
                builder.push(" is responsible)", None, EnumSet::empty());
            }
            Expression::Panic {
                reason,
                responsible,
            } => {
                builder.push("panicking because ", None, EnumSet::empty());
                reason.build_rich_ir(builder);
                builder.push(" (", None, EnumSet::empty());
                responsible.build_rich_ir(builder);
                builder.push(" is at fault)", None, EnumSet::empty());
            }
            Expression::Multiple(body) => {
                builder.indent();
                builder.push_newline();
                body.build_rich_ir(builder);
                builder.dedent();
            }
            Expression::TraceCallStarts {
                hir_call,
                function,
                arguments,
                responsible,
            } => {
                builder.push("trace: start of call of ", None, EnumSet::empty());
                function.build_rich_ir(builder);
                builder.push(" with ", None, EnumSet::empty());
                builder.push_children(arguments, " ");
                builder.push(" (", None, EnumSet::empty());
                responsible.build_rich_ir(builder);
                builder.push(" is responsible, code is at ", None, EnumSet::empty());
                hir_call.build_rich_ir(builder);
                builder.push(")", None, EnumSet::empty());
            }
            Expression::TraceCallEnds { return_value } => {
                builder.push(
                    "trace: end of call with return value ",
                    None,
                    EnumSet::empty(),
                );
                return_value.build_rich_ir(builder);
            }
            Expression::TraceExpressionEvaluated {
                hir_expression,
                value,
            } => {
                builder.push("trace: expression ", None, EnumSet::empty());
                hir_expression.build_rich_ir(builder);
                builder.push(" evaluated to ", None, EnumSet::empty());
                value.build_rich_ir(builder);
            }
            Expression::TraceFoundFuzzableFunction {
                hir_definition,
                function,
            } => {
                builder.push("trace: found fuzzable function ", None, EnumSet::empty());
                function.build_rich_ir(builder);
                builder.push(" defined at ", None, EnumSet::empty());
                hir_definition.build_rich_ir(builder);
            }
        }
    }
}
