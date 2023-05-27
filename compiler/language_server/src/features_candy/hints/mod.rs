//! Unlike other language server features, hints are not generated on-demand
//! with the usual request-response model. Instead, a hints server runs in the
//! background all the time. That way, the hints can progressively get better.
//! For example, when opening a long file, the hints may appear from top to
//! bottom as more code is evaluated. Then, the individual functions could get
//! fuzzed with ever-more-complex inputs, resulting in some error cases to be
//! displayed over time.
//!
//! While doing all that, we can pause regularly between executing instructions
//! so that we don't occupy a single CPU at 100 %.

use self::hints_finder::HintsFinder;
use crate::database::Database;
use candy_frontend::{
    module::{Module, MutableModuleProviderOwner, PackagesPath},
    rich_ir::ToRichIr,
};
use extension_trait::extension_trait;
use itertools::Itertools;
use lsp_types::{notification::Notification, Position, Url};
use rand::{seq::IteratorRandom, thread_rng};
use rustc_hash::FxHashMap;
use serde::{Deserialize, Serialize};
use std::{collections::hash_map, time::Duration, vec};
use tokio::{
    sync::mpsc::{error::TryRecvError, Receiver, Sender},
    time::sleep,
};
use tracing::debug;

mod hints_finder;
mod utils;

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
    pub uri: Url,
    pub hints: Vec<Hint>,
}
impl Notification for HintsNotification {
    const METHOD: &'static str = "candy/textDocument/publishHints";

    type Params = Self;
}

#[tokio::main(worker_threads = 1)]
#[allow(unused_must_use)]
pub async fn run_server(
    packages_path: PackagesPath,
    mut incoming_events: Receiver<Event>,
    outgoing_hints: Sender<(Module, Vec<Hint>)>,
) {
    let mut db = Database::new_with_file_system_module_provider(packages_path);
    let mut hints_finders: FxHashMap<Module, HintsFinder> = FxHashMap::default();
    let mut outgoing_hints = OutgoingHints::new(outgoing_hints);

    'server_loop: loop {
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
                    match hints_finders.entry(module.clone()) {
                        hash_map::Entry::Occupied(mut entry) => entry.get_mut().module_changed(),
                        hash_map::Entry::Vacant(entry) => {
                            entry.insert(HintsFinder::for_module(module.clone()));
                        }
                    };
                }
                Event::CloseModule(module) => {
                    db.did_close_module(&module);
                    hints_finders.remove(&module);
                }
                Event::Shutdown => {
                    incoming_events.close();
                }
            }
        }

        let Some(module) = hints_finders.keys().choose(&mut thread_rng()).cloned() else { continue; };
        let hints_finder = hints_finders.get_mut(&module).unwrap();

        hints_finder.run(&db);
        let hints = hints_finder
            .hints(&db, &module)
            .into_iter()
            // Make hints look like comments.
            .map(|mut hint_group| {
                for hint in &mut hint_group {
                    hint.text =
                        format!("{}# {}", quasi_spaces(2), hint.text.replace('\n', r#"\n"#));
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

        outgoing_hints.report_hints(module, hints).await;
    }
}

struct OutgoingHints {
    sender: Sender<(Module, Vec<Hint>)>,
    last_sent: FxHashMap<Module, Vec<Hint>>,
}
impl OutgoingHints {
    fn new(sender: Sender<(Module, Vec<Hint>)>) -> Self {
        Self {
            sender,
            last_sent: FxHashMap::default(),
        }
    }

    async fn report_hints(&mut self, module: Module, hints: Vec<Hint>) {
        if self.last_sent.get(&module) != Some(&hints) {
            debug!("Reporting hints for {}: {hints:?}", module.to_rich_ir());
            self.last_sent.insert(module.clone(), hints.clone());
            self.sender.send((module, hints)).await.unwrap();
        }
    }
}

/// VSCode trims multiple leading spaces to one. That's why we use an
/// [em quad](https://en.wikipedia.org/wiki/Quad_(typography)) instead, which
/// seems to have the same width as a normal space in VSCode.
fn quasi_spaces(n: usize) -> String {
    format!(" {}", " ".repeat(n))
}

#[extension_trait]
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
