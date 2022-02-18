use crate::{
    compiler::hir::{self, Expression, HirDb, Lambda},
    discover::{run::Discover, value::Value},
    input::InputReference,
    language_server::utils::LspPositionConversion,
};

#[salsa::query_group(AnalyzeStorage)]
pub trait Analyze: Discover + HirDb + LspPositionConversion {
    fn analyze(&self, input_reference: InputReference) -> Vec<AnalyzerReport>;
}

fn analyze(db: &dyn Analyze, input_reference: InputReference) -> Vec<AnalyzerReport> {
    let (hir, _) = db.hir(input_reference.clone()).unwrap();
    let ids = collect_hir_ids_for_value_hints_list(
        db,
        input_reference.clone(),
        hir.expressions.keys().cloned().collect(),
    );
    ids.into_iter()
        .map(move |id| (id.clone(), db.run(input_reference.clone(), id)))
        .into_iter()
        .filter_map(|(id, value)| value.map(|it| (id, it)))
        .map(move |(id, value)| match value {
            Ok(value) => AnalyzerReport::ValueOfExpression {
                id: id.to_owned(),
                value,
            },
            Err(value) => AnalyzerReport::ExpressionPanics {
                id: id.to_owned(),
                value,
            },
        })
        .collect()
}

fn collect_hir_ids_for_value_hints_list(
    db: &dyn Analyze,
    input_reference: InputReference,
    ids: Vec<hir::Id>,
) -> Vec<hir::Id> {
    ids.into_iter()
        .flat_map(|id| collect_hir_ids_for_value_hints(db, input_reference.clone(), id))
        .collect()
}
fn collect_hir_ids_for_value_hints(
    db: &dyn Analyze,
    input_reference: InputReference,
    id: hir::Id,
) -> Vec<hir::Id> {
    match db
        .find_expression(input_reference.clone(), id.clone())
        .unwrap()
    {
        Expression::Int(_) => vec![],
        Expression::Text(_) => vec![],
        Expression::Reference(_) => {
            vec![id]
        }
        Expression::Symbol(_) => vec![],
        Expression::Lambda(Lambda { body, .. }) => collect_hir_ids_for_value_hints_list(
            db,
            input_reference,
            body.expressions.keys().cloned().collect(),
        ),
        Expression::Body(body) => collect_hir_ids_for_value_hints_list(
            db,
            input_reference,
            body.expressions.keys().cloned().collect(),
        ),
        Expression::Call { arguments, .. } => {
            let mut ids = collect_hir_ids_for_value_hints_list(db, input_reference, arguments);
            ids.push(id.to_owned());
            ids
        }
        Expression::Error => vec![],
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
