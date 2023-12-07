use super::BodyId;
use crate::{
    builtin_functions::BuiltinFunction,
    hir,
    id::CountableId,
    impl_countable_id, impl_display_via_richir,
    rich_ir::{ReferenceKey, RichIrBuilder, ToRichIr, TokenType},
};
use derive_more::{From, TryInto};
use enumset::EnumSet;
use itertools::Itertools;
use num_bigint::BigInt;
use rustc_hash::FxHashMap;
use std::fmt::{self, Debug, Display, Formatter};
use strum_macros::EnumIs;

// ID

#[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ConstantId(usize);

impl_countable_id!(ConstantId);

impl Debug for ConstantId {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "%{}", self.0)
    }
}
impl Display for ConstantId {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "%{}", self.0)
    }
}
impl ToRichIr for ConstantId {
    fn build_rich_ir(&self, builder: &mut RichIrBuilder) {
        let range = builder.push(self.to_string(), TokenType::Constant, EnumSet::empty());
        builder.push_reference(*self, range);
    }
}

// Constants

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Constants(Vec<Constant>);

impl Constants {
    #[must_use]
    pub fn get(&self, id: ConstantId) -> &Constant {
        &self.0[id.to_usize()]
    }
    pub fn push(&mut self, constant: impl Into<Constant>) -> ConstantId {
        let id = ConstantId::from_usize(self.0.len());
        self.0.push(constant.into());
        id
    }

    pub fn ids_and_constants(&self) -> impl Iterator<Item = (ConstantId, &Constant)> {
        self.0
            .iter()
            .enumerate()
            .map(|(index, it)| (ConstantId(index), it))
    }
}
impl ToRichIr for Constants {
    fn build_rich_ir(&self, builder: &mut RichIrBuilder) {
        builder.push_custom_multiline(self.ids_and_constants(), |builder, (id, constant)| {
            let range = builder.push(id.to_string(), TokenType::Constant, EnumSet::empty());
            builder.push_definition(*id, range);
            builder.push(" = ", None, EnumSet::empty());
            constant.build_rich_ir(builder);
        });
    }
}

// Constant

// TODO: `impl Hash for Constant`
#[derive(Clone, Debug, EnumIs, Eq, From, PartialEq, TryInto)]
pub enum Constant {
    Int(BigInt),
    Text(String),
    Tag {
        symbol: String,
        value: Option<ConstantId>,
    },
    Builtin(BuiltinFunction),
    List(Vec<ConstantId>),
    Struct(FxHashMap<ConstantId, ConstantId>),
    HirId(hir::Id),
    Function(BodyId),
}

impl Constant {
    pub fn build_rich_ir_with_constants(
        &self,
        builder: &mut RichIrBuilder,
        constants: impl Into<Option<&Constants>>,
    ) {
        let constants = constants.into();
        let build_constant = |builder: &mut RichIrBuilder, id: ConstantId| {
            if let Some(constants) = constants {
                constants
                    .get(id)
                    .build_rich_ir_with_constants(builder, constants);
            } else {
                id.build_rich_ir(builder);
            }
        };

        match self {
            Self::Int(int) => {
                int.build_rich_ir(builder);
            }
            Self::Text(text) => {
                let range =
                    builder.push(format!(r#""{}""#, text), TokenType::Text, EnumSet::empty());
                builder.push_reference(text.clone(), range);
            }
            Self::Tag { symbol, value } => {
                let range = builder.push(symbol, TokenType::Symbol, EnumSet::empty());
                builder.push_reference(ReferenceKey::Symbol(symbol.clone()), range);
                if let Some(value) = value {
                    builder.push(" ", None, EnumSet::empty());
                    build_constant(builder, *value);
                }
            }
            Self::Builtin(builtin) => {
                builtin.build_rich_ir(builder);
            }
            Self::List(items) => {
                builder.push("(", None, EnumSet::empty());
                builder.push_children(items, ", ");
                if items.len() <= 1 {
                    builder.push(",", None, EnumSet::empty());
                }
                builder.push(")", None, EnumSet::empty());
            }
            Self::Struct(fields) => {
                builder.push("[", None, EnumSet::empty());
                builder.push_children_custom(
                    fields.iter().collect_vec(),
                    |builder, (key, value)| {
                        build_constant(builder, **key);
                        builder.push(": ", None, EnumSet::empty());
                        build_constant(builder, **value);
                    },
                    ", ",
                );
                builder.push("]", None, EnumSet::empty());
            }
            Self::HirId(id) => {
                let range = builder.push(id.to_string(), TokenType::Symbol, EnumSet::empty());
                builder.push_reference(id.clone(), range);
            }
            Self::Function(body_id) => {
                builder.push("{ ", None, EnumSet::empty());
                body_id.build_rich_ir(builder);
                builder.push(" }", None, EnumSet::empty());
            }
        }
    }
}

impl_display_via_richir!(Constant);
impl ToRichIr for Constant {
    fn build_rich_ir(&self, builder: &mut RichIrBuilder) {
        self.build_rich_ir_with_constants(builder, None)
    }
}
