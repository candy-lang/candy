use super::{super::utils::JoinWithCommasAndAnd, utils::id_to_end_of_line, Hint, HintKind};
use crate::{
    compiler::{
        ast_to_hir::AstToHir,
        hir::{Expression, HirDb, Id, Lambda},
        TracingConfig,
    },
    database::Database,
    fuzzer::{Fuzzer, Status},
    module::Module,
    vm::{
        context::{DbUseProvider, RunLimitedNumberOfInstructions},
        Heap, Pointer,
    },
};
use itertools::Itertools;
use rand::{prelude::SliceRandom, thread_rng};
use std::collections::HashMap;
use tracing::{debug, error};

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

        let fuzzer = running_fuzzers.choose_mut(&mut thread_rng())?;
        fuzzer.run(
            &mut DbUseProvider {
                db,
                tracing: TracingConfig::off(),
            },
            &mut RunLimitedNumberOfInstructions::new(1000),
        );

        match &fuzzer.status() {
            Status::StillFuzzing { .. } => None,
            Status::PanickedForArguments { .. } => Some(fuzzer.closure_id.module.clone()),
        }
    }

    pub fn get_hints(&self, db: &Database, module: &Module) -> Vec<Vec<Hint>> {
        let mut hints = vec![];

        debug!("There are {} fuzzers.", self.fuzzers.len());

        for fuzzer in self.fuzzers[module].values() {
            let Status::PanickedForArguments {
                arguments,
                reason,
                responsible,
                ..
            } = fuzzer.status() else { continue; };

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
                if &responsible.module != module {
                    // The function panics internally for an input, but it's the
                    // fault of an inner function that's in another module.
                    // TODO: The fuzz case should instead be highlighted in the
                    // used function directly. We don't do that right now
                    // because we assume the fuzzer will find the panic when
                    // fuzzing the faulty function, but we should save the
                    // panicking case (or something like that) in the future.
                    continue;
                }
                if db.hir_to_cst_id(id.clone()).is_none() {
                    panic!(
                        "It looks like the generated code {responsible} is at fault for a panic."
                    );
                }

                // TODO: In the future, re-run only the failing case with
                // tracing enabled and also show the arguments to the failing
                // function in the hint.
                Hint {
                    kind: HintKind::Fuzz,
                    text: format!("then {responsible} panics: {reason}"),
                    position: id_to_end_of_line(db, responsible.clone()).unwrap(),
                }
            };

            hints.push(vec![first_hint, second_hint]);
        }

        hints
    }
}
