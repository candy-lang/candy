use super::flow_value::FlowValue;
use crate::{
    impl_display_via_richir,
    mir::Id,
    rich_ir::{RichIrBuilder, ToRichIr},
    utils::ArcImHashSet,
};
use enumset::EnumSet;
use rustc_hash::FxHashMap;
use std::{
    fmt::Debug,
    mem,
    ops::{BitAnd, BitAndAssign, BitOr},
};

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum Timeline {
    Value { id: Id, value: FlowValue },
    And(ArcImHashSet<Timeline>),
    Or(ArcImHashSet<Timeline>),
}

impl Timeline {
    pub fn map_ids(&mut self, mapping: &FxHashMap<Id, Id>) {
        match self {
            Timeline::Value { id, value } => {
                *id = mapping[&*id];
                value.map_ids(mapping);
            }
            Timeline::And(timelines) => {
                *timelines = mem::take(timelines)
                    .into_iter()
                    .map(|mut it| {
                        it.map_ids(mapping);
                        it
                    })
                    .collect();
            }
            Timeline::Or(timelines) => {
                *timelines = mem::take(timelines)
                    .into_iter()
                    .map(|mut it| {
                        it.map_ids(mapping);
                        it
                    })
                    .collect();
            }
        }
    }
}

impl Default for Timeline {
    fn default() -> Self {
        Timeline::And(ArcImHashSet::default())
    }
}

impl BitAnd for Timeline {
    type Output = Timeline;

    fn bitand(self, rhs: Self) -> Self::Output {
        match (self, rhs) {
            #[allow(clippy::suspicious_arithmetic_impl)]
            (Timeline::And(lhs), Timeline::And(rhs)) => Timeline::And(lhs + rhs),
            (Timeline::And(mut timelines), other) | (other, Timeline::And(mut timelines)) => {
                timelines.insert(other);
                Timeline::And(timelines)
            }
            (lhs, rhs) => Timeline::And(ArcImHashSet::from_iter([lhs, rhs])),
        }
    }
}
impl BitAndAssign<Self> for Timeline {
    fn bitand_assign(&mut self, rhs: Self) {
        match (self, rhs) {
            (Timeline::And(lhs), Timeline::And(rhs)) => lhs.extend(rhs),
            (Timeline::And(lhs), rhs) => {
                lhs.insert(rhs);
            }
            (lhs, Timeline::And(mut rhs)) => {
                rhs.insert(mem::take(lhs));
                *lhs = Timeline::And(rhs);
            }
            (lhs, rhs) => {
                *lhs = Timeline::And(ArcImHashSet::from_iter([mem::take(lhs), rhs]));
            }
        }
    }
}
impl BitOr for Timeline {
    type Output = Timeline;

    fn bitor(self, rhs: Self) -> Self::Output {
        match (self, rhs) {
            #[allow(clippy::suspicious_arithmetic_impl)]
            (Timeline::Or(lhs), Timeline::Or(rhs)) => Timeline::Or(lhs + rhs),
            (Timeline::Or(mut timelines), other) | (other, Timeline::Or(mut timelines)) => {
                timelines.insert(other);
                Timeline::Or(timelines)
            }
            (lhs, rhs) => Timeline::Or(ArcImHashSet::from_iter([lhs, rhs])),
        }
    }
}

impl_display_via_richir!(Timeline);
impl ToRichIr for Timeline {
    fn build_rich_ir(&self, builder: &mut RichIrBuilder) {
        match self {
            Timeline::Value { id, value } => {
                id.build_rich_ir(builder);
                builder.push(" = ", None, EnumSet::empty());
                value.build_rich_ir(builder);
            }
            Timeline::And(timelines) => {
                builder.push("And", None, EnumSet::empty());
                builder.push_children_multiline(timelines)
            }
            Timeline::Or(timelines) => {
                builder.push("Or", None, EnumSet::empty());
                builder.push_children_multiline(timelines)
            }
        }
    }
}
