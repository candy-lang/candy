//! Unlike the usual language server features, hints are not generated on-demand
//! with the usual request-response model. Instead, a hints server runs in the
//! background all the time. That way, the hints can progressively get better.
//! For example, when opening a long file, the hints may appear from top to
//! bottom as more code is evaluated. Then, the individual closures could get
//! fuzzed with ever-more-complex inputs, resulting in some error cases to be
//! displayed over time.
//! While doing all that, we can pause regularly between executing instructions
//! so that we don't occupy a single CPU at 100%.

use super::utils::LspPositionConversion;
use crate::{
    compiler::{
        ast::{AstKind, FindAst},
        ast_to_hir::AstToHir,
        cst_to_ast::CstToAst,
        hir,
    },
    database::Database,
    input::{Input, InputDb},
    language_server::utils::TupleToPosition,
    vm::{tracer::TraceEntry, use_provider::DbUseProvider, value::Value, Status, Vm},
    CloneWithExtension,
};
use itertools::Itertools;
use lsp_types::{notification::Notification, Position};
use rand::{prelude::SliceRandom, thread_rng};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fs, sync::Arc, time::Duration};
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

async fn vm_for_input(db: Arc<Mutex<Database>>, input: Input) -> Option<Vm> {
    let db = db.lock().await;
    let module_closure = Value::module_closure_of_input(&db, input.clone())?;
    let mut vm = Vm::new();
    let use_provider = DbUseProvider { db: &db };
    vm.set_up_module_closure_execution(&use_provider, module_closure);
    Some(vm)
}

fn collect_hints(db: &Database, input: &Input, vm: &mut Vm) -> Vec<Hint> {
    log::debug!("Calculating hints for {input}");
    let mut hints = vec![];

    match vm.status() {
        Status::Running => {
            log::info!("VM is still running.");
        }
        Status::Done => {
            let return_value = vm.tear_down_module_closure_execution();
            log::info!("VM is done. Export map: {return_value}");
        }
        Status::Panicked(value) => {
            log::error!("VM panicked with value {value}.");
            match panic_hint(&db, input.clone(), &vm, value) {
                Some(hint) => {
                    hints.push(hint);
                }
                None => log::error!("Module panicked, but we are not displaying an error."),
            }
        }
    };
    if let Some(path) = input.to_path() {
        let trace = vm.tracer.dump_call_tree();
        let trace_file = path.clone_with_extension("candy.trace");
        fs::write(trace_file.clone(), trace).unwrap();
    }

    for entry in vm.tracer.log() {
        let (id, value) = match entry {
            TraceEntry::ValueEvaluated { id, value } => {
                if &id.input != input {
                    continue;
                }
                let ast_id = match db.hir_to_ast_id(id.clone()) {
                    Some(ast_id) => ast_id,
                    None => continue,
                };
                let ast = match db.ast(input.clone()) {
                    Some((ast, _)) => (*ast).clone(),
                    None => continue,
                };
                match ast.find(&ast_id) {
                    None => continue,
                    Some(ast) => match ast.kind {
                        AstKind::Assignment { .. } => {}
                        _ => continue,
                    },
                }
                (id.clone(), value.clone())
            }
            _ => continue,
        };

        hints.push(Hint {
            kind: HintKind::Value,
            text: format!(" # {value}"),
            position: id_to_end_of_line(&db, input.clone(), id).unwrap(),
        });
    }

    // If multiple hints are on the same line, only show the last one.
    // TODO: Give panic hints a higher priority.
    let hints = hints
        .into_iter()
        .group_by(|hint| hint.position.line)
        .into_iter()
        .map(|(_, hints)| hints.into_iter().last().unwrap())
        .collect_vec();

    hints
}

fn panic_hint(db: &Database, input: Input, vm: &Vm, panic_message: Value) -> Option<Hint> {
    // We want to show the hint at the last call site still inside the current
    // module. If there is no call site in this module, then the panic results
    // from a compiler error in a previous stage which is already reported.
    let last_call_in_this_module = vm
        .tracer
        .stack()
        .iter()
        .rev()
        .filter(|entry| {
            let id = match entry {
                TraceEntry::CallStarted { id, .. } => id,
                TraceEntry::NeedsStarted { id, .. } => id,
                _ => return false,
            };
            // Make sure the entry comes from the same file and is not generated code.
            id.input == input && db.hir_to_cst_id(id.clone()).is_some()
        })
        .next()?;

    let (id, call_info) = match last_call_in_this_module {
        TraceEntry::CallStarted { id, closure, args } => (
            id,
            format!(
                "{closure} {}",
                args.iter().map(|arg| format!("{arg}")).join(" ")
            ),
        ),
        TraceEntry::NeedsStarted {
            id,
            condition,
            message,
        } => (id, format!("needs {condition} {message}")),
        _ => unreachable!(),
    };

    let message = if let Value::Text(message) = panic_message {
        message
    } else {
        format!("{panic_message}")
    };
    Some(Hint {
        kind: HintKind::Panic,
        text: format!(
            " # Calling `{call_info}` panicked because {}{}",
            message,
            if message.ends_with('.') { "" } else { "." }
        ),
        position: id_to_end_of_line(db, input, id.clone())?,
    })
}

fn id_to_end_of_line(db: &Database, input: Input, id: hir::Id) -> Option<Position> {
    let span = db.hir_id_to_display_span(id.clone())?;

    let line = db
        .offset_to_lsp(input.clone(), span.start)
        .to_position()
        .line;
    let line_start_offsets = db.line_start_utf8_byte_offsets(input.clone());
    let last_characer_of_line = if line as usize == line_start_offsets.len() - 1 {
        db.get_input(input.clone()).unwrap().len()
    } else {
        line_start_offsets[(line + 1) as usize] - 1
    };
    let position = db
        .offset_to_lsp(input.clone(), last_characer_of_line)
        .to_position();
    Some(position)
}
