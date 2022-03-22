use super::utils::LspPositionConversion;
use crate::{
    compiler::{
        ast_to_hir::AstToHir,
        hir::{self, Expression, HirDb, Lambda},
    },
    discover::{result::DiscoverResult, run::Discover, value::Value},
    input::{Input, InputDb},
    language_server::utils::TupleToPosition,
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
pub trait HintsDb: AstToHir + Discover + HirDb + InputDb + LspPositionConversion {
    fn hints(&self, input: Input) -> Vec<Hint>;
}

fn hints(db: &dyn HintsDb, input: Input) -> Vec<Hint> {
    log::debug!("Calculating hints for {:?}", input);

    let (hir, _) = db.hir(input.clone()).unwrap();
    let discover_results = db.run_all(input.clone());

    collect_hir_ids_for_hints_list(db, input.clone(), hir.expressions.keys().cloned().collect())
        .into_iter()
        .filter_map(|id| {
            let (kind, value) = match discover_results.get(&id).unwrap() {
                DiscoverResult::Value(value) if value != &Value::nothing() => {
                    (HintKind::Value, value)
                }
                DiscoverResult::Panic(value) => (HintKind::Panic, value),
                _ => return None,
            };

            let span = db.hir_id_to_display_span(input.clone(), id).unwrap();

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
                kind,
                text: format!(
                    " # {}",
                    db.value_to_display_string(input.clone(), value.to_owned())
                ),
                position,
            })
        })
        // If multiple hints are on the same line, only show the first one.
        .group_by(|hint| hint.position.line)
        .into_iter()
        .map(|(_, hints)| hints.into_iter().nth(0).unwrap())
        .collect()
}

fn collect_hir_ids_for_hints_list(
    db: &dyn HintsDb,
    input: Input,
    ids: Vec<hir::Id>,
) -> Vec<hir::Id> {
    ids.into_iter()
        .flat_map(|id| collect_hir_ids_for_hints(db, input.clone(), id))
        .collect()
}
fn collect_hir_ids_for_hints(db: &dyn HintsDb, input: Input, id: hir::Id) -> Vec<hir::Id> {
    match db.find_expression(input.clone(), id.clone()).unwrap() {
        Expression::Int(_) => vec![],
        Expression::Text(_) => vec![],
        Expression::Reference(_) => vec![id],
        Expression::Symbol(_) => vec![],
        Expression::Struct(_) => vec![], // Handled separately // TODO
        Expression::Lambda(Lambda { body, .. }) => {
            collect_hir_ids_for_hints_list(db, input, body.expressions.keys().cloned().collect())
        }
        Expression::Body(body) => {
            collect_hir_ids_for_hints_list(db, input, body.expressions.keys().cloned().collect())
        }
        Expression::Call { arguments, .. } => {
            let mut ids = vec![id.to_owned()];
            for argument_id in arguments {
                let argument = db
                    .find_expression(input.clone(), argument_id.clone())
                    .unwrap();
                if let Expression::Reference(_) = argument {
                    continue;
                }

                ids.extend(collect_hir_ids_for_hints(db, input.clone(), argument_id));
            }
            ids
        }
        Expression::Error => vec![],
    }
}
