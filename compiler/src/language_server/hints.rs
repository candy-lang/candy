use super::utils::LspPositionConversion;
use crate::{
    compiler::{
        ast::{Ast, AstKind, FindAst},
        ast_to_hir::AstToHir,
        hir::{self, HirDb},
        hir_to_lir::HirToLir,
    },
    discover::run::Discover,
    input::{Input, InputDb},
    language_server::utils::TupleToPosition,
    vm::{tracer::TraceEntry, use_provider::FunctionUseProvider, value::Value, Status, Vm},
    CloneWithExtension,
};
use itertools::Itertools;
use lsp_types::{notification::Notification, Position};
use serde::{Deserialize, Serialize};
use std::fs;

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

#[salsa::query_group(HintsDbStorage)]
pub trait HintsDb:
    HirToLir + AstToHir + Discover + HirDb + InputDb + LspPositionConversion
{
    fn hints(&self, input: Input) -> Vec<Hint>;
}

fn hints(db: &dyn HintsDb, input: Input) -> Vec<Hint> {
    log::debug!("Calculating hints for {input}");

    let lir = match db.lir(input.clone()) {
        Some(lir) => (*lir).clone(),
        None => return vec![],
    };
    let module_closure = Value::module_closure_of_lir(input.clone(), lir);

    let mut vm = Vm::new();
    let use_provider = FunctionUseProvider {
        use_asset: &|input| {
            db.get_input(input.clone())
                .map(|bytes| (*bytes).clone())
                .ok_or_else(|| format!("Couldn't import file '{}'.", input))
        },
        use_local_module: &|input| db.lir(input).map(|lir| (*lir).clone()),
    };

    vm.set_up_module_closure_execution(&use_provider, module_closure);
    vm.run(&use_provider, 1000);

    let panic_hint = match vm.status() {
        Status::Running => {
            log::info!("VM is still running.");
            None
        }
        Status::Done => {
            let return_value = vm.tear_down_module_closure_execution();
            log::info!("VM is done. Export map: {return_value}");
            None
        }
        Status::Panicked(value) => {
            log::error!("VM panicked with value {value}.");
            if let Some(path) = input.to_path() {
                let trace = vm.tracer.dump_call_tree();
                let trace_file = path.clone_with_extension("candy.trace");
                fs::write(trace_file.clone(), trace).unwrap();
            }
            panic_hint(db, input.clone(), &vm, value)
        }
    };

    let interesting_values = vm
        .tracer
        .log()
        .iter()
        .filter_map(|entry| match entry {
            TraceEntry::ValueEvaluated { id, value } => {
                if id.input != input {
                    return None;
                }
                let ast_id = db.hir_to_ast_id(id.clone())?;
                let ast = (*db.ast(input.clone())?.0).clone();
                if let AstKind::Assignment { .. } = ast.find(&ast_id)?.kind {
                    Some((id.clone(), value.clone()))
                } else {
                    None
                }
            }
            _ => None,
        })
        .collect_vec();

    let value_hints = interesting_values
        .into_iter()
        .filter_map(|(id, value)| {
            Some(Hint {
                kind: HintKind::Value,
                text: format!(" # {value}"),
                position: id_to_end_of_line(db, input.clone(), id)?,
            })
        })
        .collect_vec();

    // If multiple hints are on the same line, only show the last one.
    let mut hints = value_hints
        .into_iter()
        .group_by(|hint| hint.position.line)
        .into_iter()
        .map(|(_, hints)| hints.into_iter().last().unwrap())
        .collect_vec();

    if let Some(hint) = panic_hint {
        hints.push(hint);
    }

    hints
}

fn panic_hint(db: &dyn HintsDb, input: Input, vm: &Vm, panic_message: Value) -> Option<Hint> {
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

fn id_to_end_of_line(db: &dyn HintsDb, input: Input, id: hir::Id) -> Option<Position> {
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
