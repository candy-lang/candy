use crate::{
    compiler::hir::{self, HirDb},
    discover::{run::Discover, value::Value},
    input::InputReference,
    language_server::utils::LspPositionConversion,
};

#[salsa::query_group(AnalyzeStorage)]
pub trait Analyze: Discover + HirDb + LspPositionConversion {
    fn analyze(&self, input_reference: InputReference) -> Vec<AnalyzerReport>;
}

fn analyze(db: &dyn Analyze, input_reference: InputReference) -> Vec<AnalyzerReport> {
    db.run_all(input_reference.to_owned())
        .into_iter()
        .filter_map(move |(id, value)| match value {
            Some(Ok(value)) => Some(AnalyzerReport::ValueOfExpression {
                id: id.to_owned(),
                value,
            }),
            Some(Err(value)) => Some(AnalyzerReport::ExpressionPanics {
                id: id.to_owned(),
                value,
            }),
            None => None,
        })
        .collect()
}

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub enum AnalyzerReport {
    ValueOfExpression {
        id: hir::Id,
        value: Value,
    },
    ExpressionPanics {
        id: hir::Id,
        value: Value,
    },
    FunctionHasError {
        function: hir::Id,
        error_inducing_inputs: Vec<Value>,
    },
}
