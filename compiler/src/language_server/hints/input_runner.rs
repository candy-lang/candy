use super::Hint;
use crate::{
    compiler::{
        ast::{AstKind, FindAst},
        ast_to_hir::AstToHir,
        cst_to_ast::CstToAst,
    },
    database::Database,
    input::Input,
    language_server::hints::{utils::id_to_end_of_line, HintKind},
    vm::{tracer::TraceEntry, use_provider::DbUseProvider, value::Value, Status, Vm},
    CloneWithExtension,
};
use itertools::Itertools;
use std::{fs, sync::Arc};
use tokio::sync::Mutex;

pub async fn vm_for_input(db: Arc<Mutex<Database>>, input: Input) -> Option<Vm> {
    let db = db.lock().await;
    let module_closure = Value::module_closure_of_input(&db, input.clone())?;
    let mut vm = Vm::new();
    let use_provider = DbUseProvider { db: &db };
    vm.set_up_module_closure_execution(&use_provider, module_closure);
    Some(vm)
}

pub fn collect_hints(db: &Database, input: &Input, vm: &mut Vm) -> Vec<Hint> {
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

    Some(Hint {
        kind: HintKind::Panic,
        text: format!(
            " # Calling {call_info} panicked because {}.",
            if let Value::Text(message) = panic_message {
                message
            } else {
                format!("{panic_message}")
            }
        ),
        position: id_to_end_of_line(db, input, id.clone())?,
    })
}
