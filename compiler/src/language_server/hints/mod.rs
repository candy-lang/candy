//! Unlike other language server features, hints are not generated on-demand
//! with the usual request-response model. Instead, a hints server runs in the
//! background all the time. That way, the hints can progressively get better.
//! For example, when opening a long file, the hints may appear from top to
//! bottom as more code is evaluated. Then, the individual closures could get
//! fuzzed with ever-more-complex inputs, resulting in some error cases to be
//! displayed over time.
//!
//! While doing all that, we can pause regularly between executing instructions
//! so that we don't occupy a single CPU at 100 %.

mod constant_evaluator;
mod fuzzer;
mod utils;

use self::{constant_evaluator::ConstantEvaluator, fuzzer::FuzzerManager};
use crate::{database::Database, module::Module, vm::Heap};
use itertools::Itertools;
use lsp_types::{notification::Notification, Position};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, time::Duration, vec};
use tokio::{
    sync::mpsc::{error::TryRecvError, Receiver, Sender},
    time::sleep,
};
use tracing::{trace, warn};

pub enum Event {
    UpdateModule(Module, Vec<u8>),
    CloseModule(Module),
    Shutdown,
}

#[derive(PartialEq, Eq, Debug, Clone, Serialize, Deserialize)]
pub struct Hint {
    kind: HintKind,
    text: String,
    position: Position,
}
#[derive(PartialEq, Eq, Debug, Clone, Serialize, Deserialize, PartialOrd, Ord, Copy)]
#[serde(rename_all = "camelCase")]
pub enum HintKind {
    Value,
    Fuzz,
    Panic,
}

#[derive(Serialize, Deserialize)]
pub struct HintsNotification {
    pub uri: String,
    pub hints: Vec<Hint>,
}
impl Notification for HintsNotification {
    const METHOD: &'static str = "candy/textDocument/publishHints";

    type Params = Self;
}

pub async fn run_server(
    mut incoming_events: Receiver<Event>,
    outgoing_hints: Sender<(Module, Vec<Hint>)>,
) {
    let mut db = Database::default();
    let mut constant_evaluator = ConstantEvaluator::default();
    let mut fuzzer = FuzzerManager::default();
    let mut outgoing_hints = OutgoingHints::new(outgoing_hints);

    'server_loop: loop {
        trace!("Hints server is running.");
        sleep(Duration::from_millis(100)).await;

        loop {
            let event = match incoming_events.try_recv() {
                Ok(event) => event,
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => break 'server_loop,
            };
            match event {
                Event::UpdateModule(module, content) => {
                    db.did_change_module(&module, content);
                    outgoing_hints.report_hints(module.clone(), vec![]).await;
                    constant_evaluator.update_module(&db, module.clone());
                    fuzzer.update_module(module, &Heap::default(), &[]);
                }
                Event::CloseModule(module) => {
                    db.did_close_module(&module);
                    constant_evaluator.remove_module(module.clone());
                    fuzzer.remove_module(module);
                }
                Event::Shutdown => {
                    incoming_events.close();
                }
            }
        }

        // First, try to constant-evaluate opened modules – that has a higher
        // priority. When constant evaluation is done, we try fuzzing the
        // functions we found.
        let module_with_new_insight = 'new_insight: {
            if let Some(module) = constant_evaluator.run(&db) {
                let (heap, closures) = constant_evaluator.get_fuzzable_closures(&module);
                fuzzer.update_module(module.clone(), heap, &closures);
                break 'new_insight Some(module);
            }
            if let Some(module) = fuzzer.run(&db) {
                warn!("Fuzzer found a problem!");
                break 'new_insight Some(module);
            }
            None
        };

        if let Some(module) = module_with_new_insight {
            let hints = constant_evaluator
                .get_hints(&db, &module)
                .into_iter()
                // The fuzzer returns groups of related hints.
                .map(|hint| vec![hint])
                .chain(fuzzer.get_hints(&db, &module).into_iter())
                // Make hints look like comments.
                .map(|mut hint_group| {
                    for hint in &mut hint_group {
                        hint.text = format!("{}# {}", quasi_spaces(2), hint.text);
                    }
                    hint_group
                })
                // Show related hints at the same indentation.
                .flat_map(|mut hint_group| {
                    hint_group.align_hint_columns();
                    hint_group
                })
                .sorted_by_key(|hint| hint.position)
                .collect_vec();

            module.dump_associated_debug_file(
                "hints",
                &hints.iter().map(|hint| format!("{hint:?}")).join("\n"),
            );

            // Only show the most important hint per line.
            let hints = hints
                .into_iter()
                .group_by(|hint| hint.position.line)
                .into_iter()
                .map(|(_, hints)| hints.max_by_key(|hint| hint.kind).unwrap())
                .collect_vec();

            outgoing_hints.report_hints(module, hints).await;
        }
    }
}

struct OutgoingHints {
    sender: Sender<(Module, Vec<Hint>)>,
    last_sent: HashMap<Module, Vec<Hint>>,
}
impl OutgoingHints {
    fn new(sender: Sender<(Module, Vec<Hint>)>) -> Self {
        Self {
            sender,
            last_sent: HashMap::new(),
        }
    }

    async fn report_hints(&mut self, module: Module, hints: Vec<Hint>) {
        if self.last_sent.get(&module) != Some(&hints) {
            self.last_sent.insert(module.clone(), hints.clone());
            self.sender.send((module, hints)).await.unwrap();
        }
    }
}

/// VSCode trims multiple leading spaces to one. That's why we use an
/// [em quad](https://en.wikipedia.org/wiki/Quad_(typography)) instead, which
/// seems to have the same width as a normal space in VSCode.
fn quasi_spaces(n: usize) -> String {
    " ".repeat(n)
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
                quasi_spaces(additional_indentation as usize),
                hint.text
            );
        }
    }
}
