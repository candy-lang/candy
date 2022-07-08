use super::utils::LspPositionConversion;
use crate::{
    compiler::{
        ast_to_hir::AstToHir,
        hir::{self, HirDb},
        hir_to_lir::HirToLir,
    },
    discover::run::Discover,
    input::{Input, InputDb},
    language_server::utils::TupleToPosition,
    vm::{tracer::TraceEntry, use_provider::FunctionUseProvider, value::Value, Status, Vm},
};
use itertools::Itertools;
use lsp_types::{notification::Notification, Position};
use serde::{Deserialize, Serialize};

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
    let panicked = match vm.status() {
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
            // log::error!("This is the stack trace:");
            // vm.tracer.dump_stack_trace(&db, input);
            Some(value)
        }
    };

    let id_of_this_module = hir::Id::new(input.clone(), vec![]);
    let values = vm
        .tracer
        .log()
        .iter()
        .filter_map(|entry| match entry {
            TraceEntry::ValueEvaluated { id, value } => {
                if id_of_this_module.is_parent_of(id) {
                    Some((id.clone(), value.clone()))
                } else {
                    None
                }
            }
            _ => None,
        })
        .collect_vec();

    let value_hints = values
        .into_iter()
        .filter_map(|(id, value)| {
            let span = db.hir_id_to_display_span(id)?;

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

            Some(Hint {
                kind: HintKind::Value,
                text: format!(" # {value}"),
                position,
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

    if let Some(message) = panicked {
        hints.push(Hint {
            kind: HintKind::Panic,
            text: format!(
                " # The code in this module panicked because {}.",
                if let Value::Text(message) = message {
                    message
                } else {
                    format!("{message}")
                }
            ),
            position: Position {
                line: 0,
                character: 1,
            },
        });
    }

    hints
}
