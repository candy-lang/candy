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

    fn value_to_display_string(&self, input: Input, value: Value) -> String;
}

fn run_all(db: &dyn Discover, input: Input) -> HashMap<hir::Id, DiscoverResult> {
    let (hir, _) = db.hir(input.clone()).unwrap();
    run_body(db, input, hir.as_ref(), Environment::new()).flatten()
}
fn run_body(db: &dyn Discover, input: Input, body: &Body, environment: Environment) -> Environment {
    let mut environment = environment.new_child();
    for (id, _) in &body.expressions {
        environment = run(db, input.clone(), id.to_owned(), environment);
    }
    environment
}
fn run(db: &dyn Discover, input: Input, id: hir::Id, environment: Environment) -> Environment {
    let mut environment = environment;
    let result = match db.find_expression(input.clone(), id.clone()).unwrap() {
        Expression::Int(int) => Value::Int(int.to_owned()).into(),
        Expression::Text(string) => Value::Text(string.to_owned()).into(),
        Expression::Reference(reference) => {
            if let &[builtin_function_index] = &reference.0[..] {
                if let Some(_) = builtin_functions::VALUES.get(builtin_function_index) {
                    panic!("References to built-in functions are not supported. Erroneous expression: {}", id)
                }
            }
            match db.find_expression(input.clone(), reference.to_owned()) {
                None => DiscoverResult::DependsOnParameter,
                Some(_) => environment
                    .get(&reference)
                    .expect("Value behind reference must already be in environment")
                    .transitive(),
            }
        }
        Expression::Symbol(symbol) => Value::Symbol(symbol.to_owned()).into(),
        Expression::Struct(entries) => 'outer: loop {
            let mut struct_ = HashMap::new();
            for (key, value) in entries {
                environment = run(db, input.clone(), key.clone(), environment);
                environment = run(db, input.clone(), value.clone(), environment);
                let key = match environment.get(&key).unwrap().transitive() {
                    DiscoverResult::Value(value) => value,
                    it => break 'outer it,
                };
                let value = match environment.get(&value).unwrap().transitive() {
                    DiscoverResult::Value(value) => value,
                    it => break 'outer it,
                };
                struct_.insert(key, value);
            }
            break Value::Struct(struct_).into();
        },
        Expression::Lambda(hir::Lambda { body, .. }) => {
            let result = Value::Lambda(Lambda {
                captured_environment: environment.to_owned(),
                id: id.clone(),
            })
            .into();
            environment = run_body(db, input.clone(), &body, environment);
            result
        }
        Expression::Body(body) => {
            environment = run_body(db, input, &body, environment);
            environment.get(body.out_id()).unwrap().transitive()
        }
        Expression::Call {
            function,
            arguments,
        } => run_call(db, input, function, arguments, environment.clone()),
        Expression::Error => DiscoverResult::ErrorInHir,
    };
    environment.store(id, result);
    environment
}
pub(super) fn run_call(
    db: &dyn Discover,
    input: Input,
    function: hir::Id,
    arguments: Vec<hir::Id>,
    environment: Environment,
) -> DiscoverResult {
    if let &[builtin_function_index] = &function.0[..] {
        if let Some(builtin_function) = builtin_functions::VALUES.get(builtin_function_index) {
            return run_builtin_function(
                db,
                input,
                builtin_function.to_owned(),
                arguments.to_owned(),
                environment,
            );
        }
    }

    let lambda = match environment.get(&function).unwrap().transitive()? {
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
        return DiscoverResult::panic(format!(
            "Lambda parameter and argument counts don't match: {:?}.",
            lambda_hir
        ));
    }

    let mut inner_environment = lambda.captured_environment.to_owned();
    for (index, argument) in arguments.iter().enumerate() {
        inner_environment.store(
            lambda_hir.first_id.clone() + index,
            environment.get(argument).unwrap(),
        );
    }

    run_body(db, input, &lambda_hir.body, inner_environment)
        .get(lambda_hir.body.out_id())
        .unwrap()
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
            // TODO(JonasWanke): remove this when we store the input inside each ID
            let lambda = match db.find_expression(input, id.clone()) {
                Some(lambda) => lambda,
                None => return "<lambda>".to_owned(),
            };
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
