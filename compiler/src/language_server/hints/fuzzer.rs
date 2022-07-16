use super::{utils::id_to_end_of_line, Hint, HintKind};
use crate::{
    compiler::hir::Id,
    database::Database,
    fuzzer,
    input::Input,
    vm::{
        tracer::Tracer,
        value::{Closure, Value},
    },
};
use itertools::Itertools;
use rand::{prelude::SliceRandom, thread_rng};
use std::collections::HashMap;

#[derive(Default)]
pub struct Fuzzer {
    fuzzable_closures: HashMap<Input, Vec<(Id, Closure)>>,
    found_panics: HashMap<Closure, Panic>,
}
struct Panic {
    arguments: Vec<Value>,
    message: Value,
    tracer: Tracer,
}

impl Fuzzer {
    pub fn update_input(&mut self, input: Input, fuzzable_closures: Vec<(Id, Closure)>) {
        self.fuzzable_closures
            .insert(input.clone(), fuzzable_closures);
    }

    pub fn remove_input(&mut self, input: Input) {
        self.fuzzable_closures.remove(&input).unwrap();
    }

    pub fn run(&mut self, db: &Database) -> Option<Input> {
        let mut non_panicked_closures = self
            .fuzzable_closures
            .iter()
            .map(|(input, closures)| {
                closures
                    .iter()
                    .map(|(id, closure)| (input.clone(), id.clone(), closure.clone()))
                    .collect_vec()
            })
            .flatten()
            .filter(|(_, _, closure)| !self.found_panics.contains_key(closure))
            .collect_vec();
        log::trace!(
            "Fuzzer running. {} non-panicked closures, {} in total.",
            non_panicked_closures.len(),
            self.fuzzable_closures.len(),
        );

        non_panicked_closures.shuffle(&mut thread_rng());
        let (input, closure_id, closure) = non_panicked_closures.pop()?;
        match fuzzer::fuzz_closure(db, &input, closure.clone(), &closure_id, 20) {
            fuzzer::ClosureFuzzResult::NoProblemFound => None,
            fuzzer::ClosureFuzzResult::PanickedForArguments {
                arguments,
                message,
                tracer,
            } => {
                self.found_panics.insert(
                    closure,
                    Panic {
                        arguments,
                        message,
                        tracer,
                    },
                );
                Some(input)
            }
        }
    }

    pub fn get_hints(&self, db: &Database, input: &Input) -> Vec<Hint> {
        let mut hints = vec![];

        for (id, closure) in &self.fuzzable_closures[input] {
            if let Some(Panic {
                arguments,
                message,
                tracer,
            }) = self.found_panics.get(closure)
            {
                hints.push(Hint {
                    kind: HintKind::Fuzz,
                    text: format!(
                        " # If this is called with the arguments {}, it panics because {message}.",
                        arguments
                            .iter()
                            .map(|argument| format!("{argument}"))
                            .join(" ")
                    ),
                    position: id_to_end_of_line(&db, id.clone()).unwrap(),
                });
            }
        }

        hints
    }
}
