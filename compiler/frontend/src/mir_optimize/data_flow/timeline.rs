use super::flow_value::FlowValue;
use crate::{
    impl_display_via_richir,
    mir::Id,
    rich_ir::{RichIrBuilder, ToRichIr},
    utils::ImHashSet,
};
use enumset::EnumSet;
use std::{
    fmt::Debug,
    mem,
    ops::{BitAnd, BitAndAssign, BitOr},
};

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum Timeline {
    Value { id: Id, value: FlowValue },
    And(ImHashSet<Timeline>),
    Or(ImHashSet<Timeline>),
}

impl Default for Timeline {
    fn default() -> Self {
        Timeline::And(ImHashSet::default())
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
            (lhs, rhs) => Timeline::And(ImHashSet::from_iter([lhs, rhs])),
        }
    }
}
// impl BitAndAssign<&Timeline> for Timeline {
//     fn bitand_assign(&mut self, rhs: &Timeline) {
//         match (self, rhs) {
//             (Timeline::And(lhs), Timeline::And(rhs)) => lhs.extend(rhs.to_owned()),
//             (Timeline::And(lhs), rhs) => {
//                 lhs.insert(rhs.to_owned());
//             }
//             (lhs, Timeline::And(rhs)) => {
//                 let mut timelines = rhs.to_owned();
//                 timelines.insert(mem::take(lhs));
//                 *lhs = Timeline::And(timelines);
//             }
//             (lhs, rhs) => {
//                 *lhs = Timeline::And(ImHashSet::from_iter([mem::take(lhs), rhs.to_owned()]));
//             }
//         }
//     }
// }
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
                *lhs = Timeline::And(ImHashSet::from_iter([mem::take(lhs), rhs]));
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
            (lhs, rhs) => Timeline::Or(ImHashSet::from_iter([lhs, rhs])),
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
