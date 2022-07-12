use super::Hint;
use crate::{
    compiler::{
        ast::{AstKind, FindAst},
        ast_to_hir::AstToHir,
        cst_to_ast::CstToAst,
        hir::Id,
    },
    database::Database,
    input::Input,
    language_server::hints::{utils::id_to_end_of_line, HintKind},
    vm::{
        tracer::TraceEntry,
        use_provider::DbUseProvider,
        value::{Closure, Value},
        Status, Vm,
    },
    CloneWithExtension,
};
use itertools::Itertools;
use rand::{prelude::SliceRandom, thread_rng};
use std::{collections::HashMap, fs, sync::Arc};
use tokio::sync::Mutex;

#[derive(Default)]
pub struct ConstantEvaluator {
    vms: HashMap<Input, Vm>,
}

impl ConstantEvaluator {
    pub async fn update_input(&mut self, db: Arc<Mutex<Database>>, input: Input) {
        let db = db.lock().await;
        let module_closure = Closure::of_input(&db, input.clone()).unwrap();
        let mut vm = Vm::new();
        let use_provider = DbUseProvider { db: &db };
        vm.set_up_module_closure_execution(&use_provider, module_closure);

        self.vms.insert(input.clone(), vm);
    }

    pub fn remove_input(&mut self, input: Input) {
        self.vms.remove(&input).unwrap();
    }

    pub async fn run(&mut self, db: Arc<Mutex<Database>>) -> Option<Input> {
        let mut running_vms = self
            .vms
            .iter_mut()
            .filter(|(_, vm)| match vm.status() {
                Status::Running => true,
                Status::Done => false,
                Status::Panicked(_) => false,
            })
            .collect_vec();

        running_vms.shuffle(&mut thread_rng());
        if let Some((input, vm)) = running_vms.pop() {
            let db = db.lock().await;
            let use_provider = DbUseProvider { db: &db };
            vm.run(&use_provider, 500);
            Some(input.clone())
        } else {
            None
        }
    }

    pub fn get_fuzzable_closures(&self, input: &Input) -> Vec<(Id, Closure)> {
        self.vms[input]
            .fuzzable_closures
            .iter()
            .filter(|(id, _)| &id.input == input)
            .map(|it| it.clone())
            .collect_vec()
    }

    pub async fn get_hints(&self, db: Arc<Mutex<Database>>, input: &Input) -> Vec<Hint> {
        let db = db.lock().await;
        let vm = &self.vms[input];

        log::debug!("Calculating hints for {input}");
        let mut hints = vec![];

        match vm.status() {
            Status::Running => {}
            Status::Done => {}
            Status::Panicked(value) => match panic_hint(&db, input.clone(), &vm, value) {
                Some(hint) => {
                    hints.push(hint);
                }
                None => log::error!("Module panicked, but we are not displaying an error."),
            },
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
                position: id_to_end_of_line(&db, id).unwrap(),
            });
        }

        hints
    }
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
        position: id_to_end_of_line(db, id.clone())?,
    })
}
