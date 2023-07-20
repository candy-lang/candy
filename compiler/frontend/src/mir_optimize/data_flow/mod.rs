use self::{scope::DataFlowScope, timeline::Timeline};
use super::utils::ReferenceCounts;
use crate::{
    mir::{Body, Expression, Id},
    mir_optimize::data_flow::scope::MainTimeline,
    rich_ir::{RichIr, ToRichIr},
};
use rustc_hash::FxHashMap;
use std::{collections::hash_map::Entry, fmt::Debug};
use tracing::info;

mod flow_value;
mod scope;
mod timeline;

// TODO: Split off a struct for actual results after the whole module was visited.
#[derive(Debug, Default, Eq, PartialEq)]
pub struct DataFlowInsights {
    reference_counts: FxHashMap<Id, usize>,
    scopes: Vec<DataFlowScope>,
}

impl DataFlowInsights {
    pub(super) fn new(body: &Body) -> Self {
        let mut reference_counts = body.reference_counts();
        assert!(reference_counts.insert(body.return_value(), 1).is_none());
        Self {
            reference_counts,
            scopes: vec![DataFlowScope::new_top_level(body.return_value())],
        }
    }

    pub(super) fn enter_function(&mut self, parameters: Vec<Id>, return_value: Id) {
        info!("Entering function {:?} -> {:?}", parameters, return_value);
        for parameter in parameters.iter() {
            *self.reference_counts.entry(*parameter).or_default() += 1;
        }
        assert!(self.reference_counts.insert(return_value, 1).is_none());

        let timeline: Timeline = self
            .innermost_scope()
            .timeline
            .require_no_panic()
            .to_owned();
        self.scopes
            .push(DataFlowScope::new(timeline, parameters, return_value));
    }

    pub(super) fn is_unconditional_panic(&self) -> bool {
        self.innermost_scope().timeline.is_panic()
    }

    pub(super) fn visit_optimized(
        &mut self,
        id: Id,
        expression: &Expression,
        original_reference_counts: &FxHashMap<Id, usize>,
    ) {
        let scope = self.scopes.last_mut().unwrap();
        scope.visit_optimized(id, expression, &mut self.reference_counts);
        self.on_expression_passed(id, original_reference_counts);
    }

    /// Called after we're done optimizing an expression.
    ///
    /// Reduces our internal reference counts by what was initially referenced by the expression
    /// (before it was optimized) so that we can drop information about values that are no longer
    /// used.
    pub(super) fn on_expression_passed(
        &mut self,
        id: Id,
        original_reference_counts: &FxHashMap<Id, usize>,
    ) {
        let scope = self.scopes.last_mut().unwrap();
        for (&id, &reference_count) in original_reference_counts {
            let Entry::Occupied(mut entry) = self.reference_counts.entry(id) else {
                // The referenced ID was defined inside the body of the current
                // expression.
                continue;
            };
            if *entry.get() == reference_count {
                entry.remove();
                scope.timeline.timeline_mut().remove(id);
            } else {
                *entry.get_mut() -= reference_count;
            }
        }

        if !self.reference_counts.contains_key(&id) {
            scope.timeline.timeline_mut().remove(id);
        }
    }

    pub(super) fn exit_function(&mut self, id: Id, parameters: &[Id], return_value: Id) {
        info!("Exiting function {:?} -> {:?}", parameters, return_value);
        let function = self.scopes.pop().unwrap().finalize();
        self.innermost_scope_mut().insert_value(id, function);

        for parameter in parameters {
            // Might have been removed already if the reference count dropped to
            // 0.
            self.reference_counts.remove(parameter);
        }
        self.reference_counts.remove(&return_value).unwrap();
    }
    pub(super) fn on_normalize_ids(&mut self, mapping: &FxHashMap<Id, Id>) {
        let root_scope = self.require_only_root_mut();
        for timeline in root_scope.panics.iter_mut() {
            timeline.map_ids(mapping);
        }
        root_scope.timeline.map_ids(mapping);
    }
    pub(super) fn on_constants_lifted(&mut self, lifted_constants: impl IntoIterator<Item = Id>) {
        let [.., outer_scope, inner_scope] = self.scopes.as_mut_slice() else { panic!(); };
        for constant in lifted_constants {
            assert!(inner_scope.locals.remove(&constant));
            assert!(outer_scope.locals.insert(constant));
        }
    }
    pub(super) fn on_module_folded(
        &mut self,
        id: Id,
        other: &DataFlowInsights,
        mapping: &FxHashMap<Id, Id>,
    ) {
        let this = self.require_only_root_mut();
        let other = other.require_only_root();
        assert!(
            other.panics.is_empty(),
            "Modules can't panic conditionally.",
        );

        match &other.timeline {
            MainTimeline::NoPanic {
                timeline,
                return_value,
            } => {
                let mut timeline = timeline.to_owned();
                timeline.map_ids(mapping);
                *this.require_no_panic_mut() &= timeline;

                let scope = self.scopes.last_mut().unwrap();
                scope
                    .timeline
                    .require_no_panic_mut()
                    .overwrite_value(id, *return_value);
            }
            MainTimeline::Panic(timeline) => {
                assert!(matches!(this.timeline, MainTimeline::NoPanic { .. }));
                let mut timeline = timeline.to_owned();
                timeline.map_ids(mapping);
                this.timeline = MainTimeline::Panic(timeline);
            }
        }
    }

    pub fn innermost_scope_to_rich_ir(&self) -> RichIr {
        self.innermost_scope().to_rich_ir()
    }
    fn innermost_scope(&self) -> &DataFlowScope {
        self.scopes.last().unwrap()
    }
    fn innermost_scope_mut(&mut self) -> &mut DataFlowScope {
        self.scopes.last_mut().unwrap()
    }

    fn require_only_root(&self) -> &DataFlowScope {
        match self.scopes.as_slice() {
            [root_scope] => root_scope,
            _ => panic!(),
        }
    }
    fn require_only_root_mut(&mut self) -> &mut DataFlowScope {
        match self.scopes.as_mut_slice() {
            [root_scope] => root_scope,
            _ => panic!(),
        }
    }
}
