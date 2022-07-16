use super::{super::utils::JoinWithCommasAndAnd, utils::id_to_end_of_line, Hint, HintKind};
use crate::{
    compiler::{
        ast_to_hir::AstToHir,
        hir::{Expression, HirDb, Id, Lambda},
    },
    database::Database,
    fuzzer,
    input::Input,
    vm::{
        tracer::{TraceEntry, Tracer},
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
        match fuzzer::fuzz_closure(db, &input, closure.clone(), &closure_id, 100) {
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
                let first_hint = {
                    let parameter_names = match db.find_expression(id.clone()) {
                        Some(Expression::Lambda(Lambda { parameters, .. })) => parameters
                            .into_iter()
                            .map(|parameter| parameter.keys.last().unwrap().to_string())
                            .collect_vec(),
                        Some(_) => {
                            log::warn!("Looks like we fuzzed a non-closure. That's weird.");
                            continue;
                        }
                        None => {
                            log::warn!(
                                "Using fuzzing, we found a possible error in a generated closure."
                            );
                            continue;
                        }
                    };
                    Hint {
                        kind: HintKind::Fuzz,
                        text: format!(
                            " # If this is called with {},",
                            parameter_names
                                .iter()
                                .zip(arguments.iter())
                                .map(|(name, argument)| format!("`{name} = {argument}`"))
                                .collect_vec()
                                .join_with_commas_and_and(),
                        ),
                        position: id_to_end_of_line(&db, id.clone()).unwrap(),
                    }
                };

                let second_hint = {
                    let panicking_inner_call = tracer
                        .log()
                        .iter()
                        .rev()
                        .filter(|entry| {
                            let inner_call_id = match entry {
                                TraceEntry::CallStarted { id, .. } => id,
                                TraceEntry::NeedsStarted { id, .. } => id,
                                _ => return false,
                            };
                            // Make sure the entry comes from the same file and is not generated code.
                            id.is_same_module_parent_of(inner_call_id)
                                && db.hir_to_cst_id(id.clone()).is_some()
                        })
                        .next()
                        .expect(
                            "Fuzzer found a panicking function without an inner panicking needs",
                        );
                    let (call_id, name, arguments) = match panicking_inner_call {
                        TraceEntry::CallStarted { id, closure, args } => {
                            (id.clone(), format!("{closure}"), args.clone())
                        }
                        TraceEntry::NeedsStarted {
                            id,
                            condition,
                            message,
                        } => (
                            id.clone(),
                            "needs".to_string(),
                            vec![condition.clone(), message.clone()],
                        ),
                        _ => unreachable!(),
                    };
                    Hint {
                        kind: HintKind::Fuzz,
                        text: format!(
                            " # then `{name} {}` panics because {}.",
                            arguments.iter().map(|arg| format!("{arg}")).join(" "),
                            if let Value::Text(message) = message {
                                message.to_string()
                            } else {
                                format!("{message}")
                            }
                        ),
                        position: id_to_end_of_line(db, call_id).unwrap(),
                    }
                };

                let mut panic_hints = vec![first_hint, second_hint];
                panic_hints.align_hint_columns();
                hints.extend(panic_hints);
            }
        }

        hints
    }
}

trait AlignHints {
    fn align_hint_columns(&mut self);
}
impl AlignHints for Vec<Hint> {
    fn align_hint_columns(&mut self) {
        assert!(!self.is_empty());
        let max_indentation = self.iter().map(|it| it.position.character).max().unwrap();
        for hint in self {
            let additional_indentation = max_indentation - hint.position.character;
            hint.text = format!(
                "{}{}",
                " ".repeat(additional_indentation as usize),
                hint.text
            );
        }
    }
}
