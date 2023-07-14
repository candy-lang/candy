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
    Function {
        return_value: Box<FlowValue>, // TODO
    },
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
            FlowValue::Any => {}
            FlowValue::Not(variants) => {
                for variant in variants {
                    variant.visit_referenced_ids(visit);
                }
            }
            FlowValue::Builtin(_) => {}
            FlowValue::AnyInt | FlowValue::Int(_) => {}
            FlowValue::AnyFunction => {}
            FlowValue::Function { return_value } => return_value.visit_referenced_ids(visit),
            FlowValue::AnyList => {}
            FlowValue::List(items) => {
                for item in items {
                    item.visit_referenced_ids(visit);
                }
            }
            FlowValue::Reference(id) => visit(*id),
            FlowValue::AnyStruct => {}
            FlowValue::Struct(struct_) => {
                for (key, value) in struct_ {
                    key.visit_referenced_ids(visit);
                    value.visit_referenced_ids(visit);
                }
            }
            FlowValue::AnyTag => {}
            FlowValue::Tag { symbol: _, value } => {
                if let Some(value) = value {
                    value.visit_referenced_ids(visit);
                }
            }
            FlowValue::AnyText | FlowValue::Text(_) => {}
        }
    }
    pub fn map_ids(&mut self, mapping: &FxHashMap<Id, Id>) {
        match self {
            FlowValue::Any => {}
            FlowValue::Not(variants) => {
                *variants = mem::take(variants)
                    .into_iter()
                    .map(|mut it| {
                        it.map_ids(mapping);
                        it
                    })
                    .collect();
            }
            FlowValue::Builtin(_) => {}
            FlowValue::AnyInt | FlowValue::Int(_) => {}
            FlowValue::AnyFunction => {}
            FlowValue::Function { return_value } => return_value.as_mut().map_ids(mapping),
            FlowValue::AnyList => {}
            FlowValue::List(items) => {
                for item in items {
                    item.map_ids(mapping);
                }
            }
            FlowValue::Reference(id) => *id = mapping[&*id],
            FlowValue::AnyStruct => {}
            FlowValue::Struct(struct_) => {
                for (key, value) in struct_ {
                    key.map_ids(mapping);
                    value.map_ids(mapping);
                }
            }
            FlowValue::AnyTag => {}
            FlowValue::Tag { symbol: _, value } => {
                if let Some(value) = value {
                    value.map_ids(mapping);
                }
            }
            FlowValue::AnyText | FlowValue::Text(_) => {}
        }
    }
}

impl ToRichIr for FlowValue {
    fn build_rich_ir(&self, builder: &mut RichIrBuilder) {
        match self {
            FlowValue::Any => {
                builder.push("<Any>", TokenType::Type, EnumSet::empty());
            }
            FlowValue::Not(values) => {
                FlowValue::Any.build_rich_ir(builder);
                for value in values {
                    builder.push(" - ", None, EnumSet::empty());
                    value.build_rich_ir(builder);
                }
            }
            FlowValue::Builtin(builtin) => {
                builtin.build_rich_ir(builder);
            }
            FlowValue::AnyInt => {
                builder.push("<Int>", TokenType::Type, EnumSet::empty());
            }
            FlowValue::Int(int) => {
                let range = builder.push(int.to_string(), TokenType::Int, EnumSet::empty());
                builder.push_reference(int.to_owned(), range);
            }
            FlowValue::AnyFunction => {
                builder.push("<Function>", TokenType::Type, EnumSet::empty());
            }
            FlowValue::Function { return_value } => {
                builder.push("{ ", None, EnumSet::empty());
                return_value.build_rich_ir(builder);
                builder.push(" }", None, EnumSet::empty());
            }
            FlowValue::AnyList => {
                builder.push("<List>", TokenType::Type, EnumSet::empty());
            }
            FlowValue::List(items) => {
                builder.push("(", None, EnumSet::empty());
                builder.push_children(items, ", ");
                builder.push(")", None, EnumSet::empty());
            }
            FlowValue::Reference(id) => id.build_rich_ir(builder),
            FlowValue::AnyStruct => {
                builder.push("<Struct>", TokenType::Type, EnumSet::empty());
            }
            FlowValue::Struct(fields) => {
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
            FlowValue::AnyTag => {
                builder.push("<Tag>", TokenType::Type, EnumSet::empty());
            }
            FlowValue::Tag { symbol, value } => {
                let range = builder.push(symbol, TokenType::Symbol, EnumSet::empty());
                builder.push_reference(ReferenceKey::Symbol(symbol.to_owned()), range);
                if let Some(value) = value {
                    builder.push(" ", None, EnumSet::empty());
                    value.build_rich_ir(builder);
                }
            }
            FlowValue::AnyText => {
                builder.push("<Text>", TokenType::Type, EnumSet::empty());
            }
            FlowValue::Text(text) => {
                let range =
                    builder.push(format!(r#""{}""#, text), TokenType::Text, EnumSet::empty());
                builder.push_reference(text.to_owned(), range);
            }
            FlowValue::Text(text) => {
                let range =
                    builder.push(format!(r#""{}""#, text), TokenType::Text, EnumSet::empty());
                builder.push_reference(text.to_owned(), range);
            }
        }
    }
}
