use super::flow_value::FlowValue;
use crate::{
    impl_display_via_richir,
    mir::Id,
    rich_ir::{RichIrBuilder, ToRichIr},
    utils::ArcImHashMap,
};
use enumset::EnumSet;
use itertools::Itertools;
use rustc_hash::{FxHashMap, FxHashSet};
use std::{fmt::Debug, mem, ops::BitAndAssign};

#[derive(Clone, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Timeline {
    pub values: ArcImHashMap<Id, FlowValue>,
    pub variants: Vec<Vec<Timeline>>,
}

impl Timeline {
    pub fn insert(&mut self, id: Id, value: impl Into<FlowValue>) {
        assert!(self.values.insert(id, value.into()).is_none());
    }
    pub fn replace(&mut self, id: Id, value: impl Into<FlowValue>) {
        assert!(self.values.insert(id, value.into()).is_some());
    }
    pub fn remove(&mut self, id: Id) {
        self.values.remove(&id);
        for variant in &mut self.variants.iter_mut().flatten() {
            variant.remove(id);
        }
    }

    pub fn visit_referenced_ids(&self, visit: &mut impl FnMut(Id)) {
        for value in self.values.values() {
            value.visit_referenced_ids(visit);
        }
        for variants in &self.variants {
            for variant in variants {
                variant.visit_referenced_ids(visit);
            }
        }
    }
    pub fn map_ids(&mut self, mapping: &FxHashMap<Id, Id>) {
        self.values = mem::take(&mut self.values)
            .into_iter()
            .filter_map(|(id, value)| {
                // If the mapping doesn't contain `id`, it means that the
                // corresponding expression got removed by tree shaking.
                mapping.get(&id).map(|&new_id| (new_id, value))
            })
            .map(|(id, mut value)| {
                value.map_ids(mapping);
                (id, value)
            })
            .collect();
        for variant in &mut self.variants.iter_mut().flatten() {
            variant.map_ids(mapping);
        }
    }

    pub fn reduce(&mut self, parameters: FxHashSet<Id>, return_value: impl Into<Option<Id>>) {
        let mut to_visit = vec![];
        let mut referenced = parameters;
        if let Some(return_value) = return_value.into() {
            to_visit.push(return_value);
            referenced.insert(return_value);
        }

        while let Some(current) = to_visit.pop() {
            self.collect_referenced_for_reduction(current, &mut |id| {
                if referenced.insert(id) {
                    to_visit.push(id);
                }
            });
        }

        self.retain(&referenced);
    }
    fn collect_referenced_for_reduction(&self, current: Id, add: &mut impl FnMut(Id)) -> bool {
        if let Some(value) = &self.values.get(&current) {
            value.visit_referenced_ids(add);
            true
        } else {
            let mut was_found = false;
            for variants in &self.variants {
                for variant in variants {
                    was_found |= variant.collect_referenced_for_reduction(current, add);
                }
                if was_found {
                    return true;
                }
            }
            false
        }
    }
    fn retain(&mut self, to_retain: &FxHashSet<Id>) {
        self.values.retain(|id, _| to_retain.contains(id));
        for variant in self.variants.iter_mut().flatten() {
            variant.retain(to_retain);
        }
    }
}

// impl BitAnd for Timeline {
//     type Output = Timeline;

//     fn bitand(self, rhs: Self) -> Self::Output {
//         match (self, rhs) {
//             #[allow(clippy::suspicious_arithmetic_impl)]
//             (Timeline::And(lhs), Timeline::And(rhs)) => Timeline::And(lhs + rhs),
//             (Timeline::And(mut timelines), other) | (other, Timeline::And(mut timelines)) => {
//                 timelines.insert(other);
//                 Timeline::And(timelines)
//             }
//             (lhs, rhs) => Timeline::And(ArcImHashSet::from_iter([lhs, rhs])),
//         }
//     }
// }
impl BitAndAssign<Self> for Timeline {
    fn bitand_assign(&mut self, rhs: Self) {
        self.values.extend(rhs.values);
        self.variants.extend(rhs.variants);
    }
}
// impl BitOr for Timeline {
//     type Output = Timeline;

//     fn bitor(self, rhs: Self) -> Self::Output {
//         match (self, rhs) {
//             #[allow(clippy::suspicious_arithmetic_impl)]
//             (Timeline::Or(lhs), Timeline::Or(rhs)) => Timeline::Or(lhs + rhs),
//             (Timeline::Or(mut timelines), other) | (other, Timeline::Or(mut timelines)) => {
//                 timelines.insert(other);
//                 Timeline::Or(timelines)
//             }
//             (lhs, rhs) => Timeline::Or(ArcImHashSet::from_iter([lhs, rhs])),
//         }
//     }
// }

impl_display_via_richir!(Timeline);
impl ToRichIr for Timeline {
    fn build_rich_ir(&self, builder: &mut RichIrBuilder) {
        for (id, value) in self.values.iter().sorted_by_key(|(id, _)| **id) {
            id.build_rich_ir(builder);
            builder.push(" = ", None, EnumSet::empty());
            value.build_rich_ir(builder);
            builder.push_newline();
        }

        for variants in &self.variants {
            builder.push("Or", None, EnumSet::empty());
            builder.push_children_multiline(variants);
        }
    }
}
