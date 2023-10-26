use self::{insights::DataFlowInsights, scope::DataFlowScope};
use super::utils::ReferenceCounts;
use crate::{
    mir::{Body, Expression, Id},
    mir_optimize::data_flow::operation::OperationKind,
    rich_ir::{RichIr, ToRichIr},
    utils::{HashMapExtension, HashSetExtension},
};
use rustc_hash::FxHashMap;
use std::{collections::hash_map::Entry, fmt::Debug};
use tracing::info;

pub mod flow_value;
pub mod insights;
pub mod operation;
mod scope;
pub mod timeline;

// TODO: Split off a struct for actual results after the whole module was visited.
#[derive(Debug, Default, Eq, PartialEq)]
pub struct DataFlow {
    reference_counts: FxHashMap<Id, usize>,
    scopes: Vec<DataFlowScope>,
}

impl DataFlow {
    pub(super) fn new(body: &Body) -> Self {
        let mut reference_counts = body.reference_counts();
        reference_counts.force_insert(body.return_value(), 1);
        Self {
            reference_counts,
            scopes: vec![DataFlowScope::new_top_level(body.return_value())],
        }
    }

    pub(super) fn enter_function(&mut self, parameters: Vec<Id>, return_value: Id) {
        info!("Entering function {:?} -> {:?}", parameters, return_value);
        for parameter in &parameters {
            *self.reference_counts.entry(*parameter).or_default() += 1;
        }
        self.reference_counts.force_insert(return_value, 1);

        let timeline = self.innermost_scope().state.timeline.clone();
        self.scopes
            .push(DataFlowScope::new(timeline, parameters, return_value));
    }

    pub(super) fn is_unconditional_panic(&self) -> bool {
        self.innermost_scope().state.result.is_err()
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
                scope.state.timeline.remove(id);
            } else {
                *entry.get_mut() -= reference_count;
            }
        }

        if !self.reference_counts.contains_key(&id) {
            scope.state.timeline.remove(id);
        }
    }

    pub(super) fn exit_function(&mut self, id: Id, parameters: &[Id], return_value: Id) {
        info!("Exiting function {:?} -> {:?}", parameters, return_value);
        let function = self.scopes.pop().unwrap().finalize();
        self.innermost_scope_mut().insert(id, function);

        for parameter in parameters {
            // Might have been removed already if the reference count dropped to
            // 0.
            self.reference_counts.remove(parameter);
        }
        self.reference_counts.force_remove(&return_value);
    }
    pub(super) fn finalize(mut self, mapping: &FxHashMap<Id, Id>) -> DataFlowInsights {
        let root_scope = self.scopes.pop().unwrap();
        assert!(self.scopes.is_empty());

        let mut insights = root_scope.finalize();
        insights.map_ids(mapping);
        insights
    }
    pub(super) fn on_constants_lifted(&mut self, lifted_constants: impl IntoIterator<Item = Id>) {
        let [.., outer_scope, inner_scope] = self.scopes.as_mut_slice() else {
            panic!();
        };
        for constant in lifted_constants {
            inner_scope.locals.force_remove(&constant);
            outer_scope.locals.force_insert(constant);
        }
    }
    pub(super) fn on_call_inlined(
        &mut self,
        call_id: Id,
        function: Id,
        mapping: &FxHashMap<Id, Id>,
    ) {
        self.innermost_scope_mut()
            .on_call_inlined(call_id, function, mapping);
    }
    pub(super) fn on_module_folded(
        &mut self,
        id: Id,
        other: &DataFlowInsights,
        mapping: &FxHashMap<Id, Id>,
    ) {
        let this = self.require_only_root_mut();
        assert!(
            !other
                .operations
                .iter()
                .any(|it| matches!(it.kind, OperationKind::Panic(_))),
            "Modules can't panic conditionally.",
        );

        let mut timeline = other.timeline.clone();
        timeline.map_ids(mapping);
        match &other.result {
            Ok(return_value) => {
                this.state.timeline &= timeline;

                self.innermost_scope_mut()
                    .state
                    .timeline
                    .replace(id, *return_value);
            }
            Err(panic) => {
                assert!(this.state.result.is_ok());
                this.state.timeline = timeline;
                this.state.result = Err(panic.clone());
            }
        }
    }

    pub fn innermost_scope_to_rich_ir(&self) -> RichIr {
        self.innermost_scope().to_rich_ir(false)
    }
    fn innermost_scope(&self) -> &DataFlowScope {
        self.scopes.last().unwrap()
    }
    fn innermost_scope_mut(&mut self) -> &mut DataFlowScope {
        self.scopes.last_mut().unwrap()
    }

    fn require_only_root_mut(&mut self) -> &mut DataFlowScope {
        match self.scopes.as_mut_slice() {
            [root_scope] => root_scope,
            _ => panic!(),
        }
    }
}
