use self::{flow_value::FlowValue, timeline::Timeline};
use super::current_expression::CurrentExpression;
use crate::{
    impl_display_via_richir,
    mir::{Expression, Id},
    rich_ir::{RichIrBuilder, ToRichIr},
};
use enumset::EnumSet;
use rustc_hash::FxHashMap;
use std::fmt::Debug;

mod flow_value;
mod timeline;

#[derive(Debug, Default, Eq, PartialEq)]
pub struct DataFlowInsights {
    panics: Vec<PanickingTimeline>,
    timeline: MainTimeline,
}
impl_display_via_richir!(DataFlowInsights);
impl ToRichIr for DataFlowInsights {
    fn build_rich_ir(&self, builder: &mut RichIrBuilder) {
        if !self.panics.is_empty() {
            builder.push("Panicking cases:", None, EnumSet::empty());
            builder.push_children_multiline(&self.panics);
            builder.push_newline();

            builder.push("Otherwise:", None, EnumSet::empty());
            builder.indent();
            builder.push_newline();
        }

        self.timeline.build_rich_ir(builder);

        if !self.panics.is_empty() {
            builder.dedent();
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PanickingTimeline {
    timeline: Timeline,
    reason: Id,
    responsible: Id,
}
impl PanickingTimeline {
    pub fn map_ids(&mut self, mapping: &FxHashMap<Id, Id>) {
        self.timeline.map_ids(mapping);
        self.reason = mapping[&self.reason];
        self.responsible = mapping[&self.responsible];
    }
}
impl_display_via_richir!(PanickingTimeline);
impl ToRichIr for PanickingTimeline {
    fn build_rich_ir(&self, builder: &mut RichIrBuilder) {
        Expression::Panic {
            reason: self.reason,
            responsible: self.responsible,
        }
        .build_rich_ir(builder);
        builder.indent();
        builder.push_newline();
        self.timeline.build_rich_ir(builder);
        builder.dedent();
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum MainTimeline {
    NoPanic(Timeline),
    Panic(PanickingTimeline),
}
impl MainTimeline {
    pub fn map_ids(&mut self, mapping: &FxHashMap<Id, Id>) {
        match self {
            MainTimeline::NoPanic(timeline) => timeline.map_ids(mapping),
            MainTimeline::Panic(timeline) => timeline.map_ids(mapping),
        }
    }
}
impl Default for MainTimeline {
    fn default() -> Self {
        MainTimeline::NoPanic(Timeline::default())
    }
}
impl_display_via_richir!(MainTimeline);
impl ToRichIr for MainTimeline {
    fn build_rich_ir(&self, builder: &mut RichIrBuilder) {
        match self {
            MainTimeline::NoPanic(timeline) => timeline.build_rich_ir(builder),
            MainTimeline::Panic(timeline) => timeline.build_rich_ir(builder),
        }
    }
}

impl DataFlowInsights {
    pub fn visit_optimized(&mut self, id: Id, expression: &CurrentExpression) {
        // We already know that the code panics and all code after that can be
        // ignored/removed since it never runs.
        let timeline = self.require_no_panic_mut();

        let value = match &**expression {
            Expression::Int(int) => FlowValue::Int(int.to_owned()),
            Expression::Text(text) => FlowValue::Text(text.to_owned()),
            Expression::Tag { symbol, value } => FlowValue::Tag {
                symbol: symbol.to_owned(),
                value: value.map(|it| Box::new(FlowValue::Reference(it))),
            },
            Expression::Builtin(builtin) => FlowValue::Builtin(*builtin),
            Expression::List(list) => {
                FlowValue::List(list.iter().copied().map(FlowValue::Reference).collect())
            }
            Expression::Struct(struct_) => FlowValue::Struct(
                struct_
                    .iter()
                    .map(|(key, value)| (FlowValue::Reference(*key), FlowValue::Reference(*value)))
                    .collect(),
            ),
            Expression::Reference(id) => FlowValue::Reference(*id),
            Expression::HirId(_) => {
                // HIR IDs are not normal parameters (except for `needs`) and can't
                // be accessed by the user.
                return;
            }
            Expression::Function { .. } => {
                // FIXME
                FlowValue::AnyFunction
            }
            Expression::Parameter => FlowValue::Any,
            Expression::Call { .. } => {
                FlowValue::Any
                // TODO
            }
            Expression::UseModule { .. } => {
                panic!("`Expression::UseModule` should have been inlined.")
            }
            Expression::Panic {
                reason,
                responsible,
            } => {
                self.timeline = MainTimeline::Panic(PanickingTimeline {
                    // TODO: Filter timeline to only include `reason`, `responsible`, and their dependencies.
                    timeline: timeline.to_owned(),
                    reason: *reason,
                    responsible: *responsible,
                });
                return;
            }
            // These expressions are lowered to instructions that don't actually
            // put anything on the stack. In the MIR, the result of these is
            // guaranteed to never be used afterwards.
            Expression::TraceCallStarts { .. }
            | Expression::TraceCallEnds { .. }
            | Expression::TraceExpressionEvaluated { .. }
            | Expression::TraceFoundFuzzableFunction { .. } => {
                // Tracing instructions are not referenced by anything else, so
                // we don't have to keep track of their return value (which,
                // conceptually, is `Nothing`).
                return;
            }
        };
        *timeline &= Timeline::Value { id, value };
    }

    pub(super) fn enter_function(&mut self, parameters: &[Id], responsible_parameter: Id) {
        // TODO
    }
    pub(super) fn on_normalize_ids(&mut self, mapping: &FxHashMap<Id, Id>) {
        for timeline in self.panics.iter_mut() {
            timeline.map_ids(mapping);
        }
        self.timeline.map_ids(mapping);
    }
    pub(super) fn include(&mut self, other: &DataFlowInsights, mapping: &FxHashMap<Id, Id>) {
        assert!(
            other.panics.is_empty(),
            "Modules can't panic conditionally.",
        );

        match &other.timeline {
            MainTimeline::NoPanic(timeline) => {
                let mut timeline = timeline.to_owned();
                timeline.map_ids(mapping);
                *self.require_no_panic_mut() &= timeline;
            }
            MainTimeline::Panic(timeline) => {
                assert!(matches!(self.timeline, MainTimeline::NoPanic(_)));
                let mut timeline = timeline.to_owned();
                timeline.map_ids(mapping);
                self.timeline = MainTimeline::Panic(timeline);
            }
        }
    }

    fn require_no_panic_mut(&mut self) -> &mut Timeline {
        match &mut self.timeline {
            MainTimeline::NoPanic(timeline) => timeline,
            MainTimeline::Panic(_) => panic!("Tried to continue data flow analysis after panic"),
        }
    }
}