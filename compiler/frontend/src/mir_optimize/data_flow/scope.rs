use super::{
    flow_value::FlowValue, insights::DataFlowInsights, operation::Panic, timeline::Timeline,
};
use crate::{
    impl_display_via_richir,
    mir::{Expression, Id},
    rich_ir::{RichIrBuilder, ToRichIr},
    utils::{ArcImHashMapExtension, HashSetExtension},
};
use rustc_hash::{FxHashMap, FxHashSet};
use std::fmt::Debug;

#[derive(Debug, Eq, PartialEq)]
pub struct DataFlowScope {
    pub locals: FxHashSet<Id>,
    pub(super) state: DataFlowInsights,
}
impl_display_via_richir!(DataFlowScope);
impl ToRichIr for DataFlowScope {
    fn build_rich_ir(&self, builder: &mut RichIrBuilder) {
        self.state.build_rich_ir(builder);
    }
}

impl DataFlowScope {
    pub fn new_top_level(return_value: Id) -> Self {
        Self::new(Timeline::default(), vec![], return_value)
    }
    pub fn new(mut timeline: Timeline, parameters: Vec<Id>, return_value: Id) -> Self {
        for parameter in &parameters {
            timeline.values.force_insert(*parameter, FlowValue::Any);
        }
        Self {
            locals: FxHashSet::default(),
            state: DataFlowInsights {
                parameters,
                operations: vec![],
                timeline,
                result: Ok(return_value),
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
        assert!(self.state.result.is_ok());

        let value = match expression {
            Expression::Int(int) => FlowValue::Int(int.clone()),
            Expression::Text(text) => FlowValue::Text(text.clone()),
            Expression::Tag { symbol, value } => FlowValue::Tag {
                symbol: symbol.clone(),
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
                self.locals.force_insert(id);
                return;
            }
            Expression::Function { .. } => {
                // Functions get added by [DataFlowInsights::exit_function].
                assert!(self.state.timeline.values.contains_key(&id));
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

                self.state
                    .timeline
                    .reduce(self.state.parameters.iter().copied().collect(), *reason);
                self.state.result = Err(Panic {
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
        self.insert(id, value);
    }

    pub(super) fn on_call_inlined(
        &mut self,
        call_id: Id,
        function: Id,
        mapping: &FxHashMap<Id, Id>,
    ) {
        let FlowValue::Function(mut function) = self.state.timeline.values[&function].clone()
        else {
            panic!("Inlined function is not a function.");
        };

        let mut function_timeline = function.timeline.clone();
        function_timeline.map_ids(mapping);

        self.state.operations.append(&mut function.operations);

        match function.result {
            Ok(return_value) => {
                self.state.timeline &= function_timeline;
                self.state.timeline.replace(call_id, return_value);
            }
            Err(panic) => {
                assert!(self.state.result.is_ok());
                self.state.timeline = function_timeline;
                self.state.result = Err(panic);
            }
        }
    }

    pub(super) fn insert(&mut self, id: Id, value: impl Into<FlowValue>) {
        self.locals.force_insert(id);
        self.state.timeline.insert(id, value);
    }

    pub fn finalize(mut self) -> DataFlowInsights {
        self.state.reduce();
        self.state
    }
}
