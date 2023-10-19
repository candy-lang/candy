use super::timeline::Timeline;
use crate::{
    impl_display_via_richir,
    mir::{Expression, Id},
    rich_ir::{RichIrBuilder, ToRichIr},
};
use enumset::EnumSet;
use rustc_hash::{FxHashMap, FxHashSet};

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Operation {
    /// The operation only applies if this timeline can be fulfilled by the
    /// parameters.
    pub timeline: Timeline,
    pub kind: OperationKind,
}
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum OperationKind {
    Panic(Panic),
    Impure,
}

impl Operation {
    pub fn visit_referenced_ids(&self, visit: &mut impl FnMut(Id)) {
        self.timeline.visit_referenced_ids(visit);
        match &self.kind {
            OperationKind::Panic(panic) => visit(panic.reason),
            OperationKind::Impure => todo!(),
        }
    }
    pub fn map_ids(&mut self, mapping: &FxHashMap<Id, Id>) {
        self.timeline.map_ids(mapping);
        match &mut self.kind {
            OperationKind::Panic(panic) => {
                panic.reason = mapping[&panic.reason];
                panic.responsible = mapping[&panic.responsible];
            }
            OperationKind::Impure => todo!(),
        }
    }

    pub fn reduce(&mut self, parameters: FxHashSet<Id>) {
        let return_value = match &self.kind {
            OperationKind::Panic(panic) => Some(panic.reason),
            OperationKind::Impure => None,
        };
        self.timeline.reduce(parameters, return_value);
    }
}

impl_display_via_richir!(Operation);
impl ToRichIr for Operation {
    fn build_rich_ir(&self, builder: &mut RichIrBuilder) {
        match &self.kind {
            OperationKind::Panic(panic) => {
                panic.build_rich_ir(builder);
            }
            OperationKind::Impure => {
                builder.push("impure", None, EnumSet::default());
            }
        }
        builder.push(" if:", None, EnumSet::default());
        builder.indent();
        builder.push_newline();
        self.timeline.build_rich_ir(builder);
        builder.dedent();
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Panic {
    pub reason: Id,
    pub responsible: Id,
}

impl_display_via_richir!(Panic);
impl ToRichIr for Panic {
    fn build_rich_ir(&self, builder: &mut RichIrBuilder) {
        Expression::Panic {
            reason: self.reason,
            responsible: self.responsible,
        }
        .build_rich_ir(builder);
    }
}
