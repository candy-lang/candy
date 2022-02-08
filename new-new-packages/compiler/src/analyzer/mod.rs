use crate::{
    compiler::hir::{self, Body, Expression, HirDb, Lambda},
    discover::{run::Discover, value::Value},
    input::InputReference,
    language_server::utils::LspPositionConversion,
};

#[salsa::query_group(AnalyzeStorage)]
pub trait Analyze: Discover + HirDb + LspPositionConversion {
    fn analyze(&self, input_reference: InputReference) -> Vec<AnalyzerReport>;
}

fn analyze(db: &dyn Analyze, input_reference: InputReference) -> Vec<AnalyzerReport> {
    hir_ids_for_value_hints(db, input_reference.clone())
        .into_iter()
        .map(move |id| (id.clone(), db.run(input_reference.clone(), id)))
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

fn hir_ids_for_value_hints(db: &dyn Analyze, input_reference: InputReference) -> Vec<hir::Id> {
    let (hir, _) = db.hir(input_reference).unwrap();
    let mut ids = vec![];
    hir.collect_hir_ids_for_value_hints(&mut ids);
    ids
}

impl Expression {
    fn collect_hir_ids_for_value_hints(&self, ids: &mut Vec<hir::Id>) {
        match self {
            Expression::Int(_) => {}
            Expression::Text(_) => {}
            Expression::Reference(_) => {}
            Expression::Symbol(_) => {}
            Expression::Lambda(Lambda { body, .. }) => {
                body.collect_hir_ids_for_value_hints(ids);
            }
            Expression::Body(body) => body.collect_hir_ids_for_value_hints(ids),
            Expression::Call { arguments, .. } => {
                ids.extend(arguments.iter().cloned());
            }
            Expression::Error => {}
        }
    }
}
impl Body {
    fn collect_hir_ids_for_value_hints(&self, ids: &mut Vec<hir::Id>) {
        for (id, expression) in &self.expressions {
            // We don't show the value if it's literally right there.
            let should_show_hints = match expression {
                Expression::Int(_) => false,
                Expression::Text(_) => false,
                Expression::Reference(_) => true,
                Expression::Symbol(_) => false,
                Expression::Lambda(_) => false,
                Expression::Body(_) => true,
                Expression::Call { .. } => true,
                Expression::Error => true,
            };
            if !should_show_hints {
                return;
            }

            ids.push(id.to_owned());
            expression.collect_hir_ids_for_value_hints(ids);
        }
    }
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
