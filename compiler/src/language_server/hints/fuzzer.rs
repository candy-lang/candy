use super::{super::utils::JoinWithCommasAndAnd, utils::id_to_end_of_line, Hint, HintKind};
use crate::{
    compiler::{
        ast_to_hir::AstToHir,
        hir::{Expression, HirDb, Id, Lambda},
    },
    database::Database,
    fuzzer::{Fuzzer, Status},
    input::Input,
    vm::{tracer::TraceEntry, value::Closure},
};
use itertools::Itertools;
use rand::{prelude::SliceRandom, thread_rng};
use std::collections::HashMap;

#[derive(Default)]
pub struct FuzzerManager {
    fuzzable_closures: HashMap<Input, HashMap<Id, Closure>>,
    fuzzers: HashMap<Closure, Fuzzer>,
}

impl FuzzerManager {
    pub fn update_input(
        &mut self,
        db: &Database,
        input: Input,
        fuzzable_closures: Vec<(Id, Closure)>,
    ) {
        let closures = self
            .fuzzable_closures
            .entry(input)
            .or_insert_with(HashMap::new);

        for (id, new_closure) in fuzzable_closures {
            let old_closure = closures.insert(id.clone(), new_closure.clone());
            self.fuzzers
                .entry(new_closure.clone())
                .or_insert_with(|| Fuzzer::new(db, new_closure.clone(), id));
            if let Some(old_closure) = old_closure && old_closure != new_closure {
                self.fuzzers.remove(&old_closure);
            }
        }
    }

    pub fn remove_input(&mut self, input: Input) {
        self.fuzzable_closures.remove(&input).unwrap();
    }

    pub fn run(&mut self, db: &Database) -> Option<Input> {
        let mut running_fuzzers = self
            .fuzzers
            .values_mut()
            .filter(|fuzzer| matches!(fuzzer.status(), Status::StillFuzzing { .. }))
            .collect_vec();
        log::trace!(
            "Fuzzer running. {} fuzzers for relevant closures are running.",
            running_fuzzers.len(),
        );

        let fuzzer = running_fuzzers.choose_mut(&mut thread_rng())?;
        fuzzer.run(db, 100);

        match &fuzzer.status() {
            Status::StillFuzzing { .. } => None,
            Status::PanickedForArguments { .. } => Some(fuzzer.closure_id.input.clone()),
        }
    }

    pub fn get_hints(&self, db: &Database, input: &Input) -> Vec<Vec<Hint>> {
        let relevant_fuzzers = self.fuzzable_closures[input]
            .iter()
            .map(|(_, closure)| &self.fuzzers[closure])
            .collect_vec();
        let mut hints = vec![];

        for fuzzer in &relevant_fuzzers {
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
                            log::error!("Using fuzzing, we found an error in a generated closure.");
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
                                .map(|(name, argument)| format!("`{name} = {argument}`"))
                                .collect_vec()
                                .join_with_commas_and_and(),
                        ),
                        position: id_to_end_of_line(db, id.clone()).unwrap(),
                    }
                };

                let second_hint = {
                    let panicking_inner_call = tracer
                        .log()
                        .iter()
                        .rev()
                        // Find the innermost panicking call that is in the
                        // function.
                        .find(|entry| {
                            let innermost_panicking_call_id = match entry {
                                TraceEntry::CallStarted { id, .. } => id,
                                TraceEntry::NeedsStarted { id, .. } => id,
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
                    let (call_id, name, arguments) = match panicking_inner_call {
                        TraceEntry::CallStarted { id, closure, args } => {
                            (id.clone(), format!("{closure}"), args.clone())
                        }
                        TraceEntry::NeedsStarted {
                            id,
                            condition,
                            reason,
                        } => (
                            id.clone(),
                            "needs".to_string(),
                            vec![condition.clone(), reason.clone()],
                        ),
                        _ => unreachable!(),
                    };
                    Hint {
                        kind: HintKind::Fuzz,
                        text: format!(
                            "then `{name} {}` panics because {reason}.",
                            arguments.iter().map(|arg| format!("{arg}")).join(" "),
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
