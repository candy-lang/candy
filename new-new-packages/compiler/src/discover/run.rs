use crate::{
    builtin_functions::{self, BuiltinFunction},
    compiler::hir::{self, Body, Expression, HirDb},
    input::Input,
};
use im::HashMap;
use log;

use super::{
    builtin_functions::run_builtin_function,
    value::{Environment, Lambda, Value},
};

/// The result of a successful evaluation. Either a concrete value that could be
/// used instead of the expression, or an Error indicating that the code will
/// panic with a value.
pub type EvaluationResult = Result<Value, Value>;

/// The result of an attempted evaluation. May be None if the code still
/// contains errors or is impure or too complicated.
pub type DiscoverResult = Option<EvaluationResult>;

#[salsa::query_group(DiscoverStorage)]
pub trait Discover: HirDb {
    fn run_all(&self, input: Input) -> HashMap<hir::Id, DiscoverResult>;
    fn run(&self, input: Input, id: hir::Id) -> DiscoverResult;
    fn run_with_environment(
        &self,
        input: Input,
        id: hir::Id,
        environment: Environment,
    ) -> DiscoverResult;
    fn run_multiple_with_environment(
        &self,
        input: Input,
        ids: Vec<hir::Id>,
        environment: Environment,
    ) -> Option<Result<Vec<Value>, Value>>;
    fn run_call(
        &self,
        input: Input,
        function_id: hir::Id,
        arguments: Vec<hir::Id>,
        environment: Environment,
    ) -> DiscoverResult;
    fn run_builtin_function(
        &self,
        input: Input,
        builtin_function: BuiltinFunction,
        arguments: Vec<hir::Id>,
        environment: Environment,
    ) -> DiscoverResult;
}

fn run_all(db: &dyn Discover, input: Input) -> HashMap<hir::Id, DiscoverResult> {
    db.all_hir_ids(input.to_owned())
        .unwrap()
        .into_iter()
        .map(move |id| (id.to_owned(), db.run(input.to_owned(), id)))
        .collect()
}
fn run(db: &dyn Discover, input: Input, id: hir::Id) -> DiscoverResult {
    db.run_with_environment(input, id, Environment::new(HashMap::new()))
}
fn run_with_environment(
    db: &dyn Discover,
    input: Input,
    id: hir::Id,
    environment: Environment,
) -> DiscoverResult {
    if let Some(value) = environment.get(&id) {
        return Some(Ok(value));
    }

    let expression = db.find_expression(input.to_owned(), id.to_owned())?;
    match expression {
        Expression::Int(int) => Some(Ok(int.to_owned().into())),
        Expression::Text(string) => Some(Ok(string.to_owned().into())),
        Expression::Reference(reference) => db.run_with_environment(input, reference, environment),
        Expression::Symbol(symbol) => Some(Ok(Value::Symbol(symbol.to_owned()))),
        Expression::Lambda(_) => Some(Ok(Value::Lambda(Lambda {
            captured_environment: environment.to_owned(),
            id,
        }))),
        Expression::Body(Body { out, .. }) => {
            db.run_with_environment(input, out.unwrap(), environment)
        }
        Expression::Call {
            function,
            arguments,
        } => db.run_call(input, function, arguments, environment),
        Expression::Error => None,
    }
}
fn run_multiple_with_environment(
    db: &dyn Discover,
    input: Input,
    ids: Vec<hir::Id>,
    environment: Environment,
) -> Option<Result<Vec<Value>, Value>> {
    ids.into_iter()
        .map(|it| run_with_environment(db, input.to_owned(), it.to_owned(), environment.to_owned()))
        .collect::<Option<Result<Vec<Value>, Value>>>()
}
fn run_call(
    db: &dyn Discover,
    input: Input,
    function: hir::Id,
    arguments: Vec<hir::Id>,
    environment: Environment,
) -> DiscoverResult {
    if let &[builtin_function_index] = &function.0[..] {
        if let Some(builtin_function) = builtin_functions::VALUES.get(builtin_function_index) {
            return db.run_builtin_function(
                input,
                builtin_function.to_owned(),
                arguments.to_owned(),
                environment,
            );
        }
    }

    let lambda = match db.run_with_environment(
        input.to_owned(),
        function.to_owned(),
        environment.to_owned(),
    )? {
        Ok(Value::Lambda(lambda)) => lambda,
        Ok(_) => return None,
        Err(error) => return Some(Err(error)),
    };
    let lambda_hir = match db.find_expression(input.to_owned(), lambda.id)? {
        Expression::Lambda(lambda) => lambda,
        hir => panic!(
            "Discover lambda is not backed by a HIR lambda, but `{}`.",
            hir
        ),
    };

    let lambda_parent = if let Some(lambda_parent_id) = function.parent() {
        match db.find_expression(input.to_owned(), lambda_parent_id)? {
            Expression::Body(body) => body,
            hir => panic!("A called lambda's parent isn't a body, but `{}`", hir),
        }
    } else {
        let (hir, _) = db.hir(input.to_owned()).unwrap();
        hir.as_ref().to_owned()
    };
    let function_name = lambda_parent.identifiers.get(&function).unwrap();
    log::trace!("Calling function `{}`", &function_name);

    if lambda_hir.parameters.len() != arguments.len() {
        return None;
    }

    let arguments =
        db.run_multiple_with_environment(input.to_owned(), arguments, environment.to_owned())?;
    let arguments = match arguments {
        Ok(arguments) => arguments,
        Err(error) => return Some(Err(error)),
    };

    let mut inner_environment = environment.to_owned();
    for (index, argument) in arguments.into_iter().enumerate() {
        inner_environment.store(lambda_hir.first_id.clone() + index, argument);
    }

    db.run_with_environment(input, lambda_hir.body.out.unwrap(), inner_environment)
}
