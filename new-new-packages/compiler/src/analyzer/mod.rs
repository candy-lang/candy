use crate::{
    compiler::hir::{self, Lambda},
    discover::fiber::Value,
};
use std::sync::Arc;

pub fn analyze(hir: Arc<Lambda>) -> Vec<AnalyzerReport> {
    let mut reports = vec![];
    for id in hir.first_id.0..(hir.first_id.0 + hir.expressions.len()) {
        let id = hir::Id(id);
        match evaluate(&hir, id) {
            Ok(value) => reports.push(AnalyzerReport::ValueOfExpression { id, value }),
            Err(error) => match error {
                EvaluationError::HirContainsError => {}
                EvaluationError::Panic(message) => {
                    reports.push(AnalyzerReport::ExpressionPanics { id, message })
                }
            },
        }
    }
    reports
}

fn evaluate(hir: &Lambda, id: hir::Id) -> Result<Value, EvaluationError> {
    let expression = hir.get(id).expect(&format!("The id {} doesn't exist.", id));
    match expression {
        hir::Expression::Int(int) => Ok(Value::Int(*int)),
        hir::Expression::Text(text) => Ok(Value::Text(text.clone())),
        hir::Expression::Symbol(symbol) => Ok(Value::Symbol(symbol.clone())),
        hir::Expression::Lambda(lambda) => Err(EvaluationError::HirContainsError),
        hir::Expression::Call {
            function,
            arguments,
        } => {
            let function = evaluate(hir, *function);
            Err(EvaluationError::HirContainsError)
        }
        hir::Expression::Error => Err(EvaluationError::HirContainsError),
    }
}

enum EvaluationError {
    HirContainsError,
    Panic(String),
}

#[derive(Debug)]
pub enum AnalyzerReport {
    ValueOfExpression {
        id: hir::Id,
        value: Value,
    },
    ExpressionPanics {
        id: hir::Id,
        message: String,
    },
    FunctionHasError {
        function: hir::Id,
        error_inducing_inputs: Vec<Value>,
    },
}
