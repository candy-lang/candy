//! Unlike the usual language server features, hints are not generated on-demand
//! with the usual request-response model. Instead, a hints server runs in the
//! background all the time. That way, the hints can progressively get better.
//! For example, when opening a long file, the hints may appear from top to
//! bottom as more code is evaluated. Then, the individual closures could get
//! fuzzed with ever-more-complex inputs, resulting in some error cases to be
//! displayed over time.
//! While doing all that, we can pause regularly between executing instructions
//! so that we don't occupy a single CPU at 100%.

mod constant_evaluator;
mod fuzzer;
mod utils;

use self::{constant_evaluator::ConstantEvaluator, fuzzer::Fuzzer};
use crate::{database::Database, input::Input, CloneWithExtension};
use itertools::Itertools;
use lsp_types::{notification::Notification, Position};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fs, time::Duration};
use tokio::{
    sync::mpsc::{error::TryRecvError, Receiver, Sender},
    time::sleep,
};

pub enum Event {
    UpdateModule(Input),
    CloseModule(Input),
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
    outgoing_hints: Sender<(Input, Vec<Hint>)>,
) {
    let db = Database::default();
    let mut constant_evaluator = ConstantEvaluator::default();
    let mut fuzzer = Fuzzer::default();
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
                Event::UpdateModule(input) => {
                    constant_evaluator.update_input(&db, input.clone());
                    fuzzer.update_input(input, vec![]);
                }
                Event::CloseModule(input) => {
                    constant_evaluator.remove_input(input.clone());
                    fuzzer.remove_input(input);
                }
                Event::Shutdown => {
                    incoming_events.close();
                }
            }
        }

        // First, try to constant-evaluate some input – that has a higher
        // priority. When constant evaluation is done, we try fuzzing the
        // functions we found.
        let input_with_new_insight = 'new_insight: {
            if let Some(input) = constant_evaluator.run(&db) {
                fuzzer.update_input(
                    input.clone(),
                    constant_evaluator.get_fuzzable_closures(&input),
                );
                break 'new_insight Some(input);
            }
            if let Some(input) = fuzzer.run(&db) {
                break 'new_insight Some(input);
            }
            None
        };

        if let Some(input) = input_with_new_insight {
            let mut hints = constant_evaluator
                .get_hints(&db, &input)
                .into_iter()
                .chain(fuzzer.get_hints(&db, &input).into_iter())
                .collect_vec();
            hints.sort_by_key(|hint| hint.position);

            if let Some(path) = input.to_path() {
                let hints_file = path.clone_with_extension("candy.hints");
                let content = hints.iter().map(|hint| format!("{hint:?}")).join("\n");
                fs::write(hints_file.clone(), content).unwrap();
            }

            // Only show the most important hint per line.
            let hints = hints
                .into_iter()
                .group_by(|hint| hint.position.line)
                .into_iter()
                .map(|(_, hints)| hints.max_by_key(|hint| hint.kind).unwrap())
                .collect_vec();

            outgoing_hints.report_hints(input, hints).await;
        }
    }
}

struct OutgoingHints {
    sender: Sender<(Input, Vec<Hint>)>,
    last_sent: HashMap<Input, Vec<Hint>>,
}
impl OutgoingHints {
    fn new(sender: Sender<(Input, Vec<Hint>)>) -> Self {
        Self {
            sender,
            last_sent: HashMap::new(),
        }
    }

    async fn report_hints(&mut self, input: Input, hints: Vec<Hint>) {
        if self.last_sent.get(&input) != Some(&hints) {
            self.last_sent.insert(input.clone(), hints.clone());
            self.sender.send((input, hints)).await.unwrap();
        }
    }
}
