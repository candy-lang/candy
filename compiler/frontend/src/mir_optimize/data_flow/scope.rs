use super::{
    flow_value::{FlowValue, FunctionFlowValue},
    timeline::Timeline,
};
use crate::{
    impl_display_via_richir,
    mir::{Expression, Id},
    rich_ir::{RichIrBuilder, ToRichIr},
};
use enumset::EnumSet;
use rustc_hash::{FxHashMap, FxHashSet};
use std::{fmt::Debug, mem};
use strum_macros::EnumIs;

#[derive(Debug, Eq, PartialEq)]
pub struct DataFlowScope {
    parameters: Vec<Id>,
    pub locals: FxHashSet<Id>,
    pub panics: Vec<PanickingTimeline>,
    pub timeline: MainTimeline,
}
impl_display_via_richir!(DataFlowScope);
impl ToRichIr for DataFlowScope {
    fn build_rich_ir(&self, builder: &mut RichIrBuilder) {
        build_rich_ir_for_timelines(builder, &self.panics, &self.timeline)
    }
}

pub(super) fn build_rich_ir_for_timelines(
    builder: &mut RichIrBuilder,
    panics: &[PanickingTimeline],
    timeline: &MainTimeline,
) {
    if !panics.is_empty() {
        builder.push("Panicking cases:", None, EnumSet::empty());
        builder.push_children_multiline(panics);
        builder.push_newline();

        builder.push("Otherwise:", None, EnumSet::empty());
        builder.indent();
        builder.push_newline();
    }

    timeline.build_rich_ir(builder);

    if !panics.is_empty() {
        builder.dedent();
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PanickingTimeline {
    timeline: Timeline,
    reason: Id,
    responsible: Id,
}
impl PanickingTimeline {
    pub fn visit_referenced_ids(&self, visit: &mut impl FnMut(Id)) {
        self.timeline.visit_referenced_ids(visit);
        visit(self.reason);
    }
    pub fn map_ids(&mut self, mapping: &FxHashMap<Id, Id>) {
        self.timeline.map_ids(mapping);
        self.reason = mapping[&self.reason];
        self.responsible = mapping[&self.responsible];
    }

    pub fn reduce(&mut self, parameters: FxHashSet<Id>) {
        self.timeline.reduce(parameters, self.reason)
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

#[derive(Clone, Debug, EnumIs, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum MainTimeline {
    NoPanic {
        timeline: Timeline,
        return_value: Id,
    },
    Panic(PanickingTimeline),
}
impl MainTimeline {
    pub fn visit_referenced_ids(&self, visit: &mut impl FnMut(Id)) {
        match self {
            MainTimeline::NoPanic {
                timeline,
                return_value,
            } => {
                timeline.visit_referenced_ids(visit);
                visit(*return_value);
            }
            MainTimeline::Panic(timeline) => timeline.visit_referenced_ids(visit),
        }
    }
    pub fn map_ids(&mut self, mapping: &FxHashMap<Id, Id>) {
        match self {
            MainTimeline::NoPanic { timeline, .. } => timeline.map_ids(mapping),
            MainTimeline::Panic(timeline) => timeline.map_ids(mapping),
        }
    }

    pub fn reduce(&mut self, parameters: FxHashSet<Id>) {
        match self {
            MainTimeline::NoPanic {
                timeline,
                return_value,
            } => timeline.reduce(parameters, *return_value),
            MainTimeline::Panic(timeline) => timeline.reduce(parameters),
        }
    }

    pub fn timeline_mut(&mut self) -> &mut Timeline {
        match self {
            MainTimeline::NoPanic { timeline, .. } => timeline,
            MainTimeline::Panic(timeline) => &mut timeline.timeline,
        }
    }

    pub fn require_no_panic(&self) -> &Timeline {
        match self {
            MainTimeline::NoPanic { timeline, .. } => timeline,
            MainTimeline::Panic(_) => panic!("Main timeline panics!"),
        }
    }
    pub fn require_no_panic_mut(&mut self) -> &mut Timeline {
        match self {
            MainTimeline::NoPanic { timeline, .. } => timeline,
            MainTimeline::Panic(_) => panic!("Main timeline panics!"),
        }
    }
}
impl_display_via_richir!(MainTimeline);
impl ToRichIr for MainTimeline {
    fn build_rich_ir(&self, builder: &mut RichIrBuilder) {
        match self {
            MainTimeline::NoPanic {
                timeline,
                return_value,
            } => {
                timeline.build_rich_ir(builder);
                builder.push("returns ", None, EnumSet::empty());
                return_value.build_rich_ir(builder);
            }
            MainTimeline::Panic(timeline) => timeline.build_rich_ir(builder),
        }
    }
}

impl DataFlowScope {
    pub fn new_top_level(return_value: Id) -> Self {
        Self::new(Timeline::default(), vec![], return_value)
    }
    pub fn new(mut timeline: Timeline, parameters: Vec<Id>, return_value: Id) -> Self {
        for parameter in parameters.iter() {
            assert!(timeline.values.insert(*parameter, FlowValue::Any).is_none());
        }
        Self {
            parameters,
            locals: FxHashSet::default(),
            panics: vec![],
            timeline: MainTimeline::NoPanic {
                timeline,
                return_value,
            },
        }
    }

    pub fn visit_optimized(
        &mut self,
        id: Id,
        expression: &Expression,
        reference_counts: &mut FxHashMap<Id, usize>,
    ) {
        // We already know that the code panics and all code after that can be
        // ignored/removed since it never runs.
        let timeline = self.require_no_panic_mut();

        let value = match expression {
            Expression::Int(int) => FlowValue::Int(int.to_owned()),
            Expression::Text(text) => FlowValue::Text(text.to_owned()),
            Expression::Tag { symbol, value } => FlowValue::Tag {
                symbol: symbol.to_owned(),
                value: value.map(|it| {
                    *reference_counts.get_mut(&it).unwrap() += 1;
                    Box::new(FlowValue::Reference(it))
                }),
            },
            Expression::Builtin(builtin) => FlowValue::Builtin(*builtin),
            Expression::List(list) => FlowValue::List(
                list.iter()
                    .map(|it| {
                        *reference_counts.get_mut(it).unwrap() += 1;
                        FlowValue::Reference(*it)
                    })
                    .collect(),
            ),
            Expression::Struct(struct_) => FlowValue::Struct(
                struct_
                    .iter()
                    .map(|(key, value)| {
                        *reference_counts.get_mut(key).unwrap() += 1;
                        *reference_counts.get_mut(value).unwrap() += 1;
                        (FlowValue::Reference(*key), FlowValue::Reference(*value))
                    })
                    .collect(),
            ),
            Expression::Reference(id) => {
                *reference_counts.get_mut(id).unwrap() += 1;
                FlowValue::Reference(*id)
            }
            Expression::HirId(_) => {
                // HIR IDs are not normal parameters (except for `needs`) and
                // can't be accessed by the user. Hence, we don't have to track
                // their value.
                assert!(self.locals.insert(id));
                return;
            }
            Expression::Function { .. } => {
                // Functions get added by [DataFlowInsights::exit_function].
                assert!(self.timeline.require_no_panic().values.contains_key(&id));
                return;
            }
            Expression::Parameter => FlowValue::Any,
            Expression::Call { .. } => {
                // FIXME
                FlowValue::Any
            }
            Expression::UseModule { .. } => {
                // Will be overridden by [DataFlowInsights::on_module_folded].
                FlowValue::Any
            }
            Expression::Panic {
                reason,
                responsible,
            } => {
                *reference_counts.get_mut(reason).unwrap() += 1;
                *reference_counts.get_mut(responsible).unwrap() += 1;

                let mut timeline = PanickingTimeline {
                    timeline: mem::take(timeline),
                    reason: *reason,
                    responsible: *responsible,
                };
                timeline.reduce(self.parameters.iter().copied().collect());
                self.timeline = MainTimeline::Panic(timeline);
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
        self.insert_value(id, value);
    }
    pub(super) fn insert_value(&mut self, id: Id, value: impl Into<FlowValue>) {
        self.require_no_panic_mut().insert_value(id, value);
        assert!(self.locals.insert(id));
    }

    pub fn require_no_panic_mut(&mut self) -> &mut Timeline {
        match &mut self.timeline {
            MainTimeline::NoPanic { timeline, .. } => timeline,
            MainTimeline::Panic(_) => panic!("Tried to continue data flow analysis after panic"),
        }
    }

    pub fn finalize(mut self) -> FunctionFlowValue {
        self.timeline
            .reduce(self.parameters.iter().copied().collect());
        FunctionFlowValue::new(self.parameters, self.panics, self.timeline)
    }
}
