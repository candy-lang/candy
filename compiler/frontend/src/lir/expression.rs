use super::{BodyId, ConstantId, Id};
use crate::{
    impl_display_via_richir,
    rich_ir::{ReferenceKey, RichIrBuilder, ToRichIr, TokenType},
};
use derive_more::From;
use enumset::EnumSet;
use itertools::Itertools;

#[derive(Clone, Debug, Eq, From, PartialEq)]
pub enum Expression {
    CreateTag {
        symbol: String,
        value: Id,
    },

    #[from]
    CreateList(Vec<Id>),

    #[from]
    CreateStruct(Vec<(Id, Id)>),

    CreateFunction {
        captured: Vec<Id>,
        body_id: BodyId,
    },

    #[from]
    Constant(ConstantId),

    #[from]
    Reference(Id),

    /// Increase the reference count of the given value.
    Dup {
        id: Id,
        amount: usize,
    },

    /// Decrease the reference count of the given value.
    ///
    /// If the reference count reaches zero, the value is freed.
    Drop(Id),

    Call {
        function: Id,
        arguments: Vec<Id>,
        responsible: Id,
    },

    Panic {
        reason: Id,
        responsible: Id,
    },

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
    pub fn replace_ids(&mut self, mut replacer: impl FnMut(Id) -> Id) {
        match self {
            Self::CreateTag { symbol: _, value } => {
                *value = replacer(*value);
            }
            Self::CreateList(items) => {
                for item in items {
                    *item = replacer(*item);
                }
            }
            Self::CreateStruct(fields) => {
                for (key, value) in fields {
                    *key = replacer(*key);
                    *value = replacer(*value);
                }
            }
            Self::CreateFunction {
                captured,
                body_id: _,
            } => {
                for captured in captured {
                    *captured = replacer(*captured);
                }
            }
            Self::Constant(_) => {}
            Self::Reference(id) => {
                *id = replacer(*id);
            }
            Self::Dup { id, amount: _ } => {
                *id = replacer(*id);
            }
            Self::Drop(id) => {
                *id = replacer(*id);
            }
            Self::Call {
                function,
                arguments,
                responsible,
            } => {
                *function = replacer(*function);
                for argument in arguments {
                    *argument = replacer(*argument);
                }
                *responsible = replacer(*responsible);
            }
            Self::Panic {
                reason,
                responsible,
            } => {
                *reason = replacer(*reason);
                *responsible = replacer(*responsible);
            }
            Self::TraceCallStarts {
                hir_call,
                function,
                arguments,
                responsible,
            } => {
                *hir_call = replacer(*hir_call);
                *function = replacer(*function);
                for argument in arguments {
                    *argument = replacer(*argument);
                }
                *responsible = replacer(*responsible);
            }
            Self::TraceCallEnds { return_value } => {
                *return_value = replacer(*return_value);
            }
            Self::TraceExpressionEvaluated {
                hir_expression,
                value,
            } => {
                *hir_expression = replacer(*hir_expression);
                *value = replacer(*value);
            }
            Self::TraceFoundFuzzableFunction {
                hir_definition,
                function,
            } => {
                *hir_definition = replacer(*hir_definition);
                *function = replacer(*function);
            }
        }
    }
}

impl_display_via_richir!(Expression);
impl ToRichIr for Expression {
    fn build_rich_ir(&self, builder: &mut RichIrBuilder) {
        match self {
            Self::CreateTag { symbol, value } => {
                let range = builder.push(symbol, TokenType::Symbol, EnumSet::empty());
                builder.push_reference(ReferenceKey::Symbol(symbol.clone()), range);
                builder.push(" ", None, EnumSet::empty());
                value.build_rich_ir(builder);
            }
            Self::CreateList(items) => {
                builder.push("(", None, EnumSet::empty());
                builder.push_children(items, ", ");
                if items.len() <= 1 {
                    builder.push(",", None, EnumSet::empty());
                }
                builder.push(")", None, EnumSet::empty());
            }
            Self::CreateStruct(fields) => {
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
            Self::CreateFunction { captured, body_id } => {
                builder.push("{ ", None, EnumSet::empty());
                body_id.build_rich_ir(builder);
                builder.push(" capturing ", None, EnumSet::empty());
                if captured.is_empty() {
                    builder.push("nothing", None, EnumSet::empty());
                } else {
                    builder.push_children(captured, ", ");
                }

                builder.push(" }", None, EnumSet::empty());
            }
            Self::Constant(id) => id.build_rich_ir(builder),
            Self::Reference(id) => id.build_rich_ir(builder),
            Self::Dup { id, amount } => {
                builder.push("dup ", None, EnumSet::empty());
                id.build_rich_ir(builder);
                builder.push(" by ", None, EnumSet::empty());
                builder.push(amount.to_string(), None, EnumSet::empty());
            }
            Self::Drop(id) => {
                builder.push("drop ", None, EnumSet::empty());
                id.build_rich_ir(builder);
            }
            Self::Call {
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
            Self::Panic {
                reason,
                responsible,
            } => {
                builder.push("panicking because ", None, EnumSet::empty());
                reason.build_rich_ir(builder);
                builder.push(" (", None, EnumSet::empty());
                responsible.build_rich_ir(builder);
                builder.push(" is at fault)", None, EnumSet::empty());
            }
            Self::TraceCallStarts {
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
            Self::TraceCallEnds { return_value } => {
                builder.push(
                    "trace: end of call with return value ",
                    None,
                    EnumSet::empty(),
                );
                return_value.build_rich_ir(builder);
            }
            Self::TraceExpressionEvaluated {
                hir_expression,
                value,
            } => {
                builder.push("trace: expression ", None, EnumSet::empty());
                hir_expression.build_rich_ir(builder);
                builder.push(" evaluated to ", None, EnumSet::empty());
                value.build_rich_ir(builder);
            }
            Self::TraceFoundFuzzableFunction {
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
