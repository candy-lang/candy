use super::{super::utils::JoinWithCommasAndAnd, utils::id_to_end_of_line, Hint, HintKind};
use crate::{
    compiler::{
        ast_to_hir::AstToHir,
        hir::{Expression, HirDb, Id, Lambda},
    },
    database::Database,
    fuzzer::{Fuzzer, Status},
    module::Module,
    vm::{
        context::{DbUseProvider, RunLimitedNumberOfInstructions},
        tracer::{Event, InFiberEvent},
        Heap, Pointer,
    },
};
use itertools::Itertools;
use rand::{prelude::SliceRandom, thread_rng};
use std::collections::HashMap;
use tracing::{error, trace};

#[derive(Default)]
pub struct FuzzerManager {
    fuzzers: HashMap<Module, HashMap<Id, Fuzzer>>,
}

impl FuzzerManager {
    pub fn update_module(
        &mut self,
        module: Module,
        heap: &Heap,
        fuzzable_closures: &[(Id, Pointer)],
    ) {
        let fuzzers = fuzzable_closures
            .iter()
            .map(|(id, closure)| (id.clone(), Fuzzer::new(heap, *closure, id.clone())))
            .collect();
        self.fuzzers.insert(module, fuzzers);
    }

    pub fn remove_module(&mut self, module: Module) {
        self.fuzzers.remove(&module).unwrap();
    }

    pub fn run(&mut self, db: &Database) -> Option<Module> {
        let mut running_fuzzers = self
            .fuzzers
            .values_mut()
            .flat_map(|fuzzers| fuzzers.values_mut())
            .filter(|fuzzer| matches!(fuzzer.status(), Status::StillFuzzing { .. }))
            .collect_vec();
        trace!(
            "Fuzzer running. {} fuzzers for relevant closures are running.",
            running_fuzzers.len(),
        );

        let fuzzer = running_fuzzers.choose_mut(&mut thread_rng())?;
        fuzzer.run(
            &mut DbUseProvider { db },
            &mut RunLimitedNumberOfInstructions::new(100),
        );

        match &fuzzer.status() {
            Status::StillFuzzing { .. } => None,
            Status::PanickedForArguments { .. } => Some(fuzzer.closure_id.module.clone()),
        }
    }

    pub fn get_hints(&self, db: &Database, module: &Module) -> Vec<Vec<Hint>> {
        let mut hints = vec![];

        for fuzzer in self.fuzzers[module].values() {
            if let Status::PanickedForArguments {
                arguments,
                reason,
                tracer,
            } = fuzzer.status()
            {
                let id = fuzzer.closure_id.clone();
                let first_hint = {
                    let parameter_names = match db.find_expression(id.clone()) {
                        Some(Expression::Lambda(Lambda { parameters, .. })) => parameters
                            .into_iter()
                            .map(|parameter| parameter.keys.last().unwrap().to_string())
                            .collect_vec(),
                        Some(_) => panic!("Looks like we fuzzed a non-closure. That's weird."),
                        None => {
                            error!("Using fuzzing, we found an error in a generated closure.");
                            continue;
                        }
                    };
                    Hint {
                        kind: HintKind::Fuzz,
                        text: format!(
                            "If this is called with {},",
                            parameter_names
                                .iter()
                                .zip(arguments.iter())
                                .map(|(name, argument)| format!("`{name} = {argument:?}`"))
                                .collect_vec()
                                .join_with_commas_and_and(),
                        ),
                        position: id_to_end_of_line(db, id.clone()).unwrap(),
                    }
                };

                let second_hint = {
                    let panicking_inner_call = tracer
                        .events
                        .iter()
                        .rev()
                        // Find the innermost panicking call that is in the
                        // function.
                        .filter_map(|event| match &event.event {
                            Event::InFiber { event, .. } => Some(event),
                            _ => None,
                        })
                        .find(|event| {
                            let innermost_panicking_call_id = match &event {
                                InFiberEvent::CallStarted { id, .. } => id,
                                InFiberEvent::NeedsStarted { id, .. } => id,
                                _ => return false,
                            };
                            id.is_same_module_and_any_parent_of(innermost_panicking_call_id)
                                && db.hir_to_cst_id(id.clone()).is_some()
                        });
                    let panicking_inner_call = match panicking_inner_call {
                        Some(panicking_inner_call) => panicking_inner_call,
                        None => {
                            // We found a panicking function without an inner
                            // panicking needs. This indicates an error during
                            // compilation within a function body.
                            continue;
                        }
                    };
                    let (call_id, name, arguments) = match &panicking_inner_call {
                        InFiberEvent::CallStarted { id, closure, args } => {
                            (id.clone(), closure.format(&tracer.heap), args.clone())
                        }
                        InFiberEvent::NeedsStarted {
                            id,
                            condition,
                            reason,
                        } => (id.clone(), "needs".to_string(), vec![*condition, *reason]),
                        _ => unreachable!(),
                    };
                    Hint {
                        kind: HintKind::Fuzz,
                        text: format!(
                            "then `{name} {}` panics because {reason}.",
                            arguments
                                .iter()
                                .map(|arg| arg.format(&tracer.heap))
                                .join(" "),
                        ),
                        position: id_to_end_of_line(db, call_id).unwrap(),
                    }
                };

                hints.push(vec![first_hint, second_hint]);
            }
        }

        hints
    }
}
