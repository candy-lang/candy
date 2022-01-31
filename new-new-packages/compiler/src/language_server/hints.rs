use super::utils::LspPositionConversion;
use crate::{
    analyzer::{analyze, AnalyzerReport},
    compiler::ast_to_hir::AstToHir,
    input::InputReference,
};
use lsp_types::{notification::Notification, Position, Range};
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
pub trait HintsDb: LspPositionConversion + AstToHir {
    fn hints(&self, input_reference: InputReference) -> Vec<Hint>;
}

fn hints(db: &dyn HintsDb, input_reference: InputReference) -> Vec<Hint> {
    let (hir, _) = &db.hir(input_reference).unwrap();

    let reports = analyze((*hir).clone());
    for report in &reports {
        log::error!("Report: {:?}", report);
    }

    reports
        .into_iter()
        .map(|report| Hint {
            text: match report {
                AnalyzerReport::ValueOfExpression { id, value } => format!("{:?}", value),
                AnalyzerReport::ExpressionPanics { id, message } => message,
                AnalyzerReport::FunctionHasError {
                    function,
                    error_inducing_inputs,
                } => "A function has an error.".into(),
            },
            range: Range {
                start: Position {
                    line: 10,
                    character: 2,
                },
                end: Position {
                    line: 10,
                    character: 5,
                },
            },
        })
        .collect()
}
