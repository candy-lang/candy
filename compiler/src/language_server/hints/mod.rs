//! Unlike the usual language server features, hints are not generated on-demand
//! with the usual request-response model. Instead, a hints server runs in the
//! background all the time. That way, the hints can progressively get better.
//! For example, when opening a long file, the hints may appear from top to
//! bottom as more code is evaluated. Then, the individual closures could get
//! fuzzed with ever-more-complex inputs, resulting in some error cases to be
//! displayed over time.
//! While doing all that, we can pause regularly between executing instructions
//! so that we don't occupy a single CPU at 100%.

mod input_runner;
mod utils;

use self::input_runner::{collect_hints, vm_for_input};
use crate::{
    database::Database,
    input::Input,
    vm::{use_provider::DbUseProvider, Status, Vm},
};
use itertools::Itertools;
use lsp_types::{notification::Notification, Position};
use rand::{prelude::SliceRandom, thread_rng};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, sync::Arc, time::Duration};
use tokio::{
    sync::{
        mpsc::{error::TryRecvError, Receiver, Sender},
        Mutex,
    },
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
#[derive(PartialEq, Eq, Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum HintKind {
    Value,
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
    db: Arc<Mutex<Database>>,
    mut incoming_events: Receiver<Event>,
    outgoing_hints: Sender<(Input, Vec<Hint>)>,
) {
    let mut vms = HashMap::new();

    'server_loop: loop {
        loop {
            match incoming_events.try_recv() {
                Ok(event) => handle_event(db.clone(), &mut incoming_events, &mut vms, event).await,
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => break 'server_loop,
            }
        }

        let input_of_chosen_vm = vms
            .iter()
            .filter(|(_, vm)| match vm.status() {
                Status::Running => true,
                Status::Done => false,
                Status::Panicked(_) => false,
            })
            .collect_vec()
            .choose(&mut thread_rng())
            .map(|(input, _)| (*input).clone());

        if let Some(input) = input_of_chosen_vm {
            let hints = {
                let vm = vms.get_mut(&input).unwrap();
                let db = db.lock().await;
                let use_provider = DbUseProvider { db: &db };
                vm.run(&use_provider, 5);
                collect_hints(&db, &input, vm)
            };
            outgoing_hints.send((input, hints)).await.unwrap();
        }

        sleep(Duration::from_millis(100)).await;
    }
}

async fn handle_event(
    db: Arc<Mutex<Database>>,
    incoming_events: &mut Receiver<Event>,
    vms: &mut HashMap<Input, Vm>,
    event: Event,
) {
    match event {
        Event::UpdateModule(input) => match vm_for_input(db, input.clone()).await {
            Some(vm) => {
                vms.insert(input, vm);
            }
            None => {}
        },
        Event::CloseModule(input) => {
            vms.remove(&input);
        }
        Event::Shutdown => {
            incoming_events.close();
        }
    }
}
