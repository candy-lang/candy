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

use self::module_analyzer::ModuleAnalyzer;
use super::AnalyzerClient;
use crate::database::Database;
use candy_frontend::module::{Module, MutableModuleProviderOwner, PackagesPath};
use lsp_types::{notification::Notification, Position, Url};
use rand::{seq::IteratorRandom, thread_rng};
use rustc_hash::FxHashMap;
use serde::{Deserialize, Serialize};
use std::{fmt, future::Future, time::Duration, vec};
use tokio::{
    sync::mpsc::{self, error::TryRecvError},
    time::sleep,
};
use tracing::debug;

mod code_lens;
pub mod insights;
mod module_analyzer;
mod utils;

pub enum Message {
    UpdateModule(Module, Vec<u8>),
    CloseModule(Module),
    Shutdown,
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

#[derive(PartialEq, Eq, Debug, Clone, Serialize, Deserialize)]
pub struct Hint {
    pub kind: HintKind,
    pub text: String,
    pub position: Position,
}
#[derive(PartialEq, Eq, Debug, Clone, Serialize, Deserialize, PartialOrd, Ord, Copy)]
#[serde(rename_all = "camelCase")]
pub enum HintKind {
    Value,
    Panic,
    FuzzingStatus,
}

#[tokio::main(worker_threads = 1)]
#[allow(unused_must_use)]
pub async fn run_server(
    packages_path: PackagesPath,
    mut incoming_events: mpsc::Receiver<Message>,
    client: AnalyzerClient,
) {
    // PERF: Stop this loop when we don't have any updates.
    let client_clone = client.clone();
    tokio::spawn(async move {
        sleep(Duration::from_millis(1000)).await;
        loop {
            sleep(Duration::from_millis(500)).await;
            client_clone.code_lenses_updated().await;
        }
    });

    let mut db = Database::new_with_file_system_module_provider(packages_path);
    let mut analyzers: FxHashMap<Module, ModuleAnalyzer> = FxHashMap::default();
    let client_ref = &client;
    let mut outgoing_diagnostics = OutgoingCache::new(move |module, diagnostics| {
        client_ref.update_diagnostics(module, diagnostics)
    });
    let mut outgoing_hints =
        OutgoingCache::new(move |module, hints| client_ref.update_hints(module, hints));

    'server_loop: loop {
        sleep(Duration::from_millis(100)).await;

        loop {
            let event = match incoming_events.try_recv() {
                Ok(event) => event,
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => break 'server_loop,
            };
            match event {
                Message::UpdateModule(module, content) => {
                    db.did_change_module(&module, content);
                    outgoing_hints.send(module.clone(), vec![]).await;
                    analyzers
                        .entry(module.clone())
                        .and_modify(|it| it.module_changed())
                        .or_insert_with(|| ModuleAnalyzer::for_module(module.clone()));
                }
                Message::CloseModule(module) => {
                    db.did_close_module(&module);
                    analyzers.remove(&module);
                }
                Message::Shutdown => {
                    incoming_events.close();
                }
            }
        }

        let Some(module) = analyzers.keys().choose(&mut thread_rng()).cloned() else {
            client.update_status(None);
            continue;
        };
        let analyzer = analyzers.get_mut(&module).unwrap();

        analyzer.run(&db, &client).await;

        let (mut hints, diagnostics) = analyzer.insights(&db);
        hints.sort_by_key(|hint| hint.position);

        outgoing_diagnostics.send(module.clone(), diagnostics).await;
        outgoing_hints.send(module, hints).await;
    }
}

struct OutgoingCache<T, R: Fn(Module, T) -> F, F: Future> {
    sender: R,
    last_sent: FxHashMap<Module, T>,
}
impl<T: Clone + fmt::Debug + Eq, R: Fn(Module, T) -> F, F: Future> OutgoingCache<T, R, F> {
    fn new(sender: R) -> Self {
        Self {
            sender,
            last_sent: FxHashMap::default(),
        }
    }

    async fn send(&mut self, module: Module, value: T) {
        if self.last_sent.get(&module) != Some(&value) {
            debug!("Reporting for {}: {value:?}", module);
            self.last_sent.insert(module.clone(), value.clone());
            (self.sender)(module, value).await;
        }
    }
}
