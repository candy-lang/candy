use super::insights::DataFlowInsights;
use crate::{
    builtin_functions::BuiltinFunction,
    mir::Id,
    rich_ir::{ReferenceKey, RichIrBuilder, ToRichIr, TokenType},
};
use derive_more::From;
use enumset::EnumSet;
use itertools::Itertools;
use num_bigint::BigInt;
use rustc_hash::FxHashMap;
use std::{collections::BTreeSet, mem};

#[derive(Clone, Debug, Eq, From, Hash, Ord, PartialEq, PartialOrd)]
pub enum FlowValue {
    Any,

    Not(BTreeSet<FlowValue>),

    #[from]
    Builtin(BuiltinFunction),

    AnyInt,
    #[from]
    Int(BigInt),

    AnyFunction,
    #[from]
    Function(DataFlowInsights),

    AnyList,
    #[from]
    List(Vec<FlowValue>),

    #[from]
    Reference(Id),

    AnyStruct,
    #[from]
    Struct(Vec<(FlowValue, FlowValue)>),

    AnyTag,
    Tag {
        symbol: String,
        value: Option<Box<FlowValue>>,
    },

    AnyText,
    #[from]
    Text(String),
}

impl FlowValue {
    pub fn visit_referenced_ids(&self, visit: &mut impl FnMut(Id)) {
        match self {
            Self::Any => {}
            Self::Not(variants) => {
                for variant in variants {
                    variant.visit_referenced_ids(visit);
                }
            }
            Self::Builtin(_) => {}
            Self::AnyInt | Self::Int(_) => {}
            Self::AnyFunction => {}
            Self::Function(function) => function.visit_referenced_ids(visit),
            Self::AnyList => {}
            Self::List(items) => {
                for item in items {
                    item.visit_referenced_ids(visit);
                }
            }
            Self::Reference(id) => visit(*id),
            Self::AnyStruct => {}
            Self::Struct(struct_) => {
                for (key, value) in struct_ {
                    key.visit_referenced_ids(visit);
                    value.visit_referenced_ids(visit);
                }
            }
            Self::AnyTag => {}
            Self::Tag { symbol: _, value } => {
                if let Some(value) = value {
                    value.visit_referenced_ids(visit);
                }
            }
            Self::AnyText | Self::Text(_) => {}
        }
    }
    pub fn map_ids(&mut self, mapping: &FxHashMap<Id, Id>) {
        match self {
            Self::Any => {}
            Self::Not(variants) => {
                *variants = mem::take(variants)
                    .into_iter()
                    .map(|mut it| {
                        it.map_ids(mapping);
                        it
                    })
                    .collect();
            }
            Self::Builtin(_) => {}
            Self::AnyInt | Self::Int(_) => {}
            Self::AnyFunction => {}
            Self::Function(function) => function.map_ids(mapping),
            Self::AnyList => {}
            Self::List(items) => {
                for item in items {
                    item.map_ids(mapping);
                }
            }
            Self::Reference(id) => *id = mapping[&*id],
            Self::AnyStruct => {}
            Self::Struct(struct_) => {
                for (key, value) in struct_ {
                    key.map_ids(mapping);
                    value.map_ids(mapping);
                }
            }
            Self::AnyTag => {}
            Self::Tag { symbol: _, value } => {
                if let Some(value) = value {
                    value.map_ids(mapping);
                }
            }
            Self::AnyText | Self::Text(_) => {}
        }
    }
}

impl ToRichIr for FlowValue {
    fn build_rich_ir(&self, builder: &mut RichIrBuilder) {
        match self {
            Self::Any => {
                builder.push("<Any>", TokenType::Type, EnumSet::empty());
            }
            Self::Not(values) => {
                Self::Any.build_rich_ir(builder);
                for value in values {
                    builder.push(" - ", None, EnumSet::empty());
                    value.build_rich_ir(builder);
                }
            }
            Self::Builtin(builtin) => {
                builtin.build_rich_ir(builder);
            }
            Self::AnyInt => {
                builder.push("<Int>", TokenType::Type, EnumSet::empty());
            }
            Self::Int(int) => {
                let range = builder.push(int.to_string(), TokenType::Int, EnumSet::empty());
                builder.push_reference(int.clone(), range);
            }
            Self::AnyFunction => {
                builder.push("<Function>", TokenType::Type, EnumSet::empty());
            }
            Self::Function(function) => function.build_rich_ir(builder),
            Self::AnyList => {
                builder.push("<List>", TokenType::Type, EnumSet::empty());
            }
            Self::List(items) => {
                builder.push("(", None, EnumSet::empty());
                builder.push_children(items, ", ");
                builder.push(")", None, EnumSet::empty());
            }
            Self::Reference(id) => id.build_rich_ir(builder),
            Self::AnyStruct => {
                builder.push("<Struct>", TokenType::Type, EnumSet::empty());
            }
            Self::Struct(fields) => {
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
            Self::AnyTag => {
                builder.push("<Tag>", TokenType::Type, EnumSet::empty());
            }
            Self::Tag { symbol, value } => {
                let range = builder.push(symbol, TokenType::Symbol, EnumSet::empty());
                builder.push_reference(ReferenceKey::Symbol(symbol.clone()), range);
                if let Some(value) = value {
                    builder.push(" ", None, EnumSet::empty());
                    value.build_rich_ir(builder);
                }
            }
            Self::AnyText => {
                builder.push("<Text>", TokenType::Type, EnumSet::empty());
            }
            Self::Text(text) => {
                let range =
                    builder.push(format!(r#""{}""#, text), TokenType::Text, EnumSet::empty());
                builder.push_reference(text.clone(), range);
            }
        }
    }
}
