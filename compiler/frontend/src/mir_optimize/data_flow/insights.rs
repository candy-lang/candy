use super::{
    operation::{Operation, Panic},
    timeline::Timeline,
};
use crate::{
    mir::Id,
    rich_ir::{RichIrBuilder, ToRichIr},
};
use derive_more::From;
use enumset::EnumSet;
use rustc_hash::FxHashMap;

#[derive(Clone, Debug, Eq, From, Hash, Ord, PartialEq, PartialOrd)]
pub struct DataFlowInsights {
    pub parameters: Vec<Id>,
    pub operations: Vec<Operation>,
    pub timeline: Timeline,
    pub result: Result<Id, Panic>,
}

impl DataFlowInsights {
    pub fn new(
        parameters: Vec<Id>,
        operations: Vec<Operation>,
        timeline: Timeline,
        result: Result<Id, Panic>,
    ) -> Self {
        Self {
            parameters,
            operations,
            timeline,
            result,
        }
    }

    pub fn visit_referenced_ids(&self, visit: &mut impl FnMut(Id)) {
        for operation in &self.operations {
            operation.visit_referenced_ids(visit);
        }
        self.timeline.visit_referenced_ids(visit);
        match &self.result {
            Ok(return_value) => visit(*return_value),
            Err(panic) => {
                visit(panic.reason);
            }
        }
    }
    pub fn map_ids(&mut self, mapping: &FxHashMap<Id, Id>) {
        for parameter in &mut self.parameters {
            *parameter = mapping[&*parameter];
        }
        for operation in &mut self.operations {
            operation.map_ids(mapping);
        }
        self.timeline.map_ids(mapping);
        self.result = match self.result {
            Ok(return_value) => Ok(mapping[&return_value]),
            Err(Panic {
                reason,
                responsible,
            }) => Err(Panic {
                reason: mapping[&reason],
                responsible: mapping[&responsible],
            }),
        }
    }

    pub fn reduce(&mut self) {
        self.timeline.reduce(
            self.parameters.iter().copied().collect(),
            self.result.as_ref().copied().unwrap_or_else(|it| it.reason),
        );
    }
}

impl ToRichIr for DataFlowInsights {
    fn build_rich_ir(&self, builder: &mut RichIrBuilder) {
        builder.push("{", None, EnumSet::empty());

        for parameter in &self.parameters {
            builder.push(" ", None, EnumSet::empty());
            parameter.build_rich_ir(builder);
        }
        if !self.parameters.is_empty() {
            builder.push(" ->", None, EnumSet::empty());
        }

        builder.indent();
        builder.push_newline();

        if !self.operations.is_empty() {
            builder.push("Operations:", None, EnumSet::empty());
            builder.push_children_multiline(&self.operations);
            builder.push_newline();

            builder.push("Otherwise:", None, EnumSet::empty());
            builder.indent();
            builder.push_newline();
        }

        self.timeline.build_rich_ir(builder);
        match &self.result {
            Ok(return_value) => {
                builder.push("returns ", None, EnumSet::empty());
                return_value.build_rich_ir(builder);
            }
            Err(panic) => panic.build_rich_ir(builder),
        }

        if !self.operations.is_empty() {
            builder.dedent();
        }

        builder.dedent();
        builder.push_newline();
        builder.push("}", None, EnumSet::empty());
    }
}
