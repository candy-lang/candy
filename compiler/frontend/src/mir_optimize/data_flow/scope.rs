use super::{flow_value::FlowValue, timeline::Timeline};
use crate::{
    impl_display_via_richir,
    mir::{Expression, Id},
    rich_ir::{RichIrBuilder, ToRichIr},
};
use enumset::EnumSet;
use rustc_hash::{FxHashMap, FxHashSet};
use std::{fmt::Debug, mem};
use strum_macros::EnumIs;

#[derive(Debug, Default, Eq, PartialEq)]
pub struct DataFlowScope {
    pub locals: FxHashSet<Id>,
    pub panics: Vec<PanickingTimeline>,
    pub timeline: MainTimeline,
}
impl_display_via_richir!(DataFlowScope);
impl ToRichIr for DataFlowScope {
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

    pub fn reduce(&mut self, parameters: &[Id]) {
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

#[derive(Debug, EnumIs, Eq, PartialEq)]
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

    pub fn reduce(&mut self, parameters: &[Id], return_value: Id) {
        match self {
            MainTimeline::NoPanic(timeline) => timeline.reduce(parameters, return_value),
            MainTimeline::Panic(timeline) => timeline.reduce(parameters),
        }
    }

    pub fn timeline_mut(&mut self) -> &mut Timeline {
        match self {
            MainTimeline::NoPanic(timeline) => timeline,
            MainTimeline::Panic(timeline) => &mut timeline.timeline,
        }
    }

    pub fn require_no_panic(&self) -> &Timeline {
        match self {
            MainTimeline::NoPanic(timeline) => timeline,
            MainTimeline::Panic(_) => panic!("Main timeline panics!"),
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

impl DataFlowScope {
    pub fn new(mut timeline: Timeline, parameters: &[Id]) -> Self {
        for parameter in parameters {
            assert!(timeline.values.insert(*parameter, FlowValue::Any).is_none());
        }
        Self {
            locals: FxHashSet::default(),
            panics: vec![],
            timeline: MainTimeline::NoPanic(timeline),
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
                // FIXME
                FlowValue::AnyFunction
            }
            Expression::Parameter => FlowValue::Any,
            Expression::Call { .. } => {
                // FIXME
                FlowValue::Any
            }
            Expression::UseModule { .. } => {
                // FIXME
                FlowValue::Any
            }
            Expression::Panic {
                reason,
                responsible,
            } => {
                *reference_counts.get_mut(reason).unwrap() += 1;
                *reference_counts.get_mut(responsible).unwrap() += 1;
                self.timeline = MainTimeline::Panic(PanickingTimeline {
                    // TODO: Filter timeline to only include `reason`, `responsible`, and their dependencies.
                    timeline: mem::take(timeline),
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
        timeline.values.insert(id, value);
        assert!(self.locals.insert(id));
    }

    pub fn require_no_panic_mut(&mut self) -> &mut Timeline {
        match &mut self.timeline {
            MainTimeline::NoPanic(timeline) => timeline,
            MainTimeline::Panic(_) => panic!("Tried to continue data flow analysis after panic"),
        }
    }

    pub fn reduce(&mut self, parameters: &[Id], return_value: Id) {
        for panicking_timeline in &mut self.panics {
            panicking_timeline.reduce(parameters);
        }
        self.timeline.reduce(parameters, return_value);
    }
    // pub fn tree_shake(&mut self, parameters: &[Id], return_value: Id) {
    //     match self.timeline {
    //         MainTimeline::NoPanic(timeline) => timeline.tree_shake(),
    //         MainTimeline::Panic(_) => todo!(),
    //     }
    // }
}
