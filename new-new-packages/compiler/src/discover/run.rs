use crate::{
    builtin_functions::{self, BuiltinFunction},
    compiler::hir::{self, Body, Expression, HirDb},
    input::Input,
};
use im::HashMap;
use itertools::Itertools;
use log;

use super::{
    builtin_functions::run_builtin_function,
    result::DiscoverResult,
    value::{Environment, Lambda, Value},
};

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
    ) -> DiscoverResult<Vec<Value>>;
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

    fn value_to_display_string(&self, input: Input, value: Value) -> String;
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
        return value.into();
    }

    match db.find_expression(input.to_owned(), id.to_owned())? {
        Expression::Int(int) => Value::Int(int.to_owned()).into(),
        Expression::Text(string) => Value::Text(string.to_owned()).into(),
        Expression::Reference(reference) => db.run_with_environment(input, reference, environment),
        Expression::Symbol(symbol) => Value::Symbol(symbol.to_owned()).into(),
        Expression::Struct(entries) => {
            let struct_ = entries
                .into_iter()
                .map(|(key, value)| {
                    let key = db.run_with_environment(input.clone(), key, environment.clone())?;
                    let value =
                        db.run_with_environment(input.clone(), value, environment.clone())?;
                    (key, value).into()
                })
                .collect::<DiscoverResult<HashMap<Value, Value>>>()?;
            Value::Struct(struct_).into()
        }
        Expression::Lambda(_) => Value::Lambda(Lambda {
            captured_environment: environment.to_owned(),
            id,
        })
        .into(),
        Expression::Body(Body { out, .. }) => {
            db.run_with_environment(input, out.unwrap(), environment)
        }
        Expression::Call {
            function,
            arguments,
        } => db.run_call(input, function, arguments, environment),
        Expression::Error => DiscoverResult::ErrorInHir,
    }
}
fn run_multiple_with_environment(
    db: &dyn Discover,
    input: Input,
    ids: Vec<hir::Id>,
    environment: Environment,
) -> DiscoverResult<Vec<Value>> {
    ids.into_iter()
        .map(|it| run_with_environment(db, input.to_owned(), it.to_owned(), environment.to_owned()))
        .collect()
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
        Value::Lambda(lambda) => lambda,
        it => panic!("Tried to call something that wasn't a lambda: {:?}", it),
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
        return DiscoverResult::Panic(Value::Text(format!(
            "Lambda parameter and argument counts don't match: {:?}.",
            lambda_hir
        )));
    }

    let arguments =
        db.run_multiple_with_environment(input.to_owned(), arguments, environment.to_owned())?;

    let mut inner_environment = environment.to_owned();
    for (index, argument) in arguments.into_iter().enumerate() {
        inner_environment.store(lambda_hir.first_id.clone() + index, argument);
    }

    db.run_with_environment(input, lambda_hir.body.out.unwrap(), inner_environment)
}

fn value_to_display_string(db: &dyn Discover, input: Input, value: Value) -> String {
    match value {
        Value::Int(value) => format!("{}", value),
        Value::Text(value) => format!("\"{}\"", value),
        Value::Symbol(value) => format!("{}", value),
        Value::Struct(entries) => format!(
            "[{}]",
            entries
                .into_iter()
                .map(|(key, value)| format!(
                    "{}: {}",
                    value_to_display_string(db, input.clone(), key),
                    value_to_display_string(db, input.clone(), value)
                ))
                .join(", ")
        ),
        Value::Lambda(Lambda { id, .. }) => {
            let lambda = db.find_expression(input, id.clone()).unwrap();
            if let Expression::Lambda(hir::Lambda { parameters, .. }) = lambda {
                if parameters.is_empty() {
                    "{ … }".to_owned()
                } else {
                    format!("{{ {} -> … }}", parameters.join(" "))
                }
            } else {
                panic!("HIR of lambda {} is not a lambda.", id);
            }
        }
    }
}
