use crate::{
    builtin_functions,
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

    fn value_to_display_string(&self, value: Value) -> String;
}

fn run_all(db: &dyn Discover, input: Input) -> HashMap<hir::Id, DiscoverResult> {
    let (hir, _) = db.hir(input.clone()).unwrap();
    run_body(db, hir.as_ref(), Environment::new()).flatten()
}
fn run_body(db: &dyn Discover, body: &Body, environment: Environment) -> Environment {
    let mut environment = environment.new_child();
    for (id, _) in &body.expressions {
        environment = run(db, id.to_owned(), environment);
    }
    environment
}
fn run(db: &dyn Discover, id: hir::Id, environment: Environment) -> Environment {
    let mut environment = environment;
    let result = match db.find_expression(id.clone()).unwrap() {
        Expression::Int(int) => Value::Int(int.to_owned()).into(),
        Expression::Text(string) => Value::Text(string.to_owned()).into(),
        Expression::Reference(reference) => {
            if let &[builtin_function_index] = &reference.local[..] {
                if let Some(_) = builtin_functions::VALUES.get(builtin_function_index) {
                    panic!("References to built-in functions are not supported. Erroneous expression: {}", id)
                }
            }
            match db.find_expression(reference.to_owned()) {
                None => DiscoverResult::DependsOnParameter,
                Some(_) => environment.get(&reference).transitive(),
            }
        }
        Expression::Symbol(symbol) => Value::Symbol(symbol.to_owned()).into(),
        Expression::Struct(entries) => 'outer: loop {
            let mut struct_ = HashMap::new();
            for (key, value) in entries {
                let key = match environment.get(&key).transitive() {
                    DiscoverResult::Value(value) => value,
                    it => break 'outer it,
                };
                let value = match environment.get(&value).transitive() {
                    DiscoverResult::Value(value) => value,
                    it => break 'outer it,
                };
                struct_.insert(key, value);
            }
            break Value::Struct(struct_).into();
        },
        Expression::Lambda(hir::Lambda {
            first_id,
            parameters,
            body,
        }) => {
            let lambda = Lambda {
                captured_environment: environment.clone(),
                id: id.clone(),
            };

            for (index, _) in parameters.iter().enumerate() {
                environment.store(first_id.clone() + index, DiscoverResult::DependsOnParameter);
            }
            environment = run_body(db, &body, environment);
            Value::Lambda(lambda).into()
        }
        Expression::Body(body) => {
            environment = run_body(db, &body, environment);
            environment.get(body.out_id()).transitive()
        }
        Expression::Call {
            function,
            arguments,
        } => run_call(db, function, arguments, environment.clone()),
        Expression::Error { .. } => DiscoverResult::ErrorInHir,
    };
    environment.store(id, result);
    environment
}
pub(super) fn run_call(
    db: &dyn Discover,
    function: hir::Id,
    arguments: Vec<hir::Id>,
    environment: Environment,
) -> DiscoverResult {
    if let &[builtin_function_index] = &function.local[..] {
        if let Some(builtin_function) = builtin_functions::VALUES.get(builtin_function_index) {
            return run_builtin_function(
                db,
                builtin_function.to_owned(),
                arguments.to_owned(),
                environment,
            );
        }
    }

    let lambda = match environment.get(&function).transitive()? {
        Value::Lambda(lambda) => lambda,
        it => panic!("Tried to call something that wasn't a lambda: {:?}", it),
    };
    let lambda_hir = match db.find_expression(lambda.id)? {
        Expression::Lambda(lambda) => lambda,
        hir => panic!(
            "Discover lambda is not backed by a HIR lambda, but `{}`.",
            hir
        ),
    };

    if lambda_hir.parameters.len() != arguments.len() {
        return DiscoverResult::panic(format!(
            "Lambda parameter and argument counts don't match: {:?}.",
            lambda_hir
        ));
    }

    let mut inner_environment = lambda.captured_environment.to_owned();
    for (index, argument) in arguments.iter().enumerate() {
        inner_environment.store(
            lambda_hir.first_id.clone() + index,
            environment.get(argument),
        );
    }

    run_body(db, &lambda_hir.body, inner_environment).get(lambda_hir.body.out_id())
}

fn value_to_display_string(db: &dyn Discover, value: Value) -> String {
    match value {
        Value::Int(value) => format!("{}", value),
        Value::Text(value) => format!("\"{}\"", value),
        Value::Symbol(value) => format!("{}", value),
        Value::Struct(entries) => format!(
            "[{}]",
            entries
                .keys()
                .into_iter()
                .map(|it| (it, value_to_display_string(db, it.to_owned())))
                .sorted_by_key(|(_, it)| it.to_owned())
                .map(|(key, key_string)| format!(
                    "{}: {}",
                    key_string,
                    value_to_display_string(db, entries[key].to_owned())
                ))
                .join(", ")
        ),
        Value::Lambda(Lambda { id, .. }) => {
            let lambda = db.find_expression(id.clone()).unwrap();
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
