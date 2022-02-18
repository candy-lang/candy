use super::utils::LspPositionConversion;
use crate::{
    analyzer::{Analyze, AnalyzerReport},
    compiler::{ast_to_hir::AstToHir, cst::CstVecExtension},
    input::InputReference,
    language_server::utils::TupleToPosition,
};
use lsp_types::{notification::Notification, Range};
use serde::{Deserialize, Serialize};

#[derive(PartialEq, Eq, Debug, Clone, Serialize, Deserialize)]
pub struct Hint {
    text: String,
    range: Range,
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
pub trait HintsDb: Analyze + AstToHir + LspPositionConversion {
    fn hints(&self, input_reference: InputReference) -> Vec<Hint>;
}

fn hints(db: &dyn HintsDb, input_reference: InputReference) -> Vec<Hint> {
    log::debug!("Calculating hints!");

    let (cst, _) = db.cst_raw(input_reference.clone()).unwrap();
    let (_, ast_to_cst_id_mapping, _) = db.ast_raw(input_reference.clone()).unwrap();
    let (.., hir_to_ast_id_mapping, _) = db.hir_raw(input_reference.clone()).unwrap();

    db.analyze(input_reference.clone())
        .into_iter()
        .filter_map(|report| {
            let (id, message) = match report {
                AnalyzerReport::ValueOfExpression { id, value } => (id, format!("{}", value)),
                AnalyzerReport::ExpressionPanics { id, value } => (id, format!("{}", value)),
                AnalyzerReport::FunctionHasError { function, .. } => {
                    (function, "A function has an error.".into())
                }
            };
            let id = match hir_to_ast_id_mapping.get(&id) {
                Some(id) => id,
                None => {
                    log::info!("Couldn't find ID {}.", id);
                    return None;
                }
            };
            let id = match ast_to_cst_id_mapping.get(&id) {
                Some(id) => id,
                None => {
                    log::info!("Couldn't find ID {:?}.", id);
                    return None;
                }
            };
            let span = cst.find(&id).unwrap().span();

            Some(Hint {
                text: format!(" # {}", message),
                range: Range {
                    start: db
                        .utf8_byte_offset_to_lsp(span.start, input_reference.clone())
                        .to_position(),
                    end: db
                        .utf8_byte_offset_to_lsp(span.end, input_reference.clone())
                        .to_position(),
                },
            })
        })
        .collect()
}
