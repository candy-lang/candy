use crate::{
    builtin_functions::BuiltinFunction,
    compiler::hir::{self, Expression},
    input::Input,
};

use super::{
    run::{Discover, DiscoverResult},
    value::{Environment, Value},
};

pub fn run_builtin_function(
    db: &dyn Discover,
    input: Input,
    builtin_function: BuiltinFunction,
    arguments: Vec<hir::Id>,
    environment: Environment,
) -> DiscoverResult {
    log::info!(
        "run_builtin_function: {:?} {}",
        builtin_function,
        arguments.len()
    );
    // Handle builtin functions that don't need to resolve the arguments.
    match builtin_function {
        BuiltinFunction::IfElse => return if_else(db, input, arguments, environment),
        _ => {}
    }

    let arguments = db.run_multiple_with_environment(input.to_owned(), arguments, environment)?;
    let arguments = match arguments {
        Ok(arguments) => arguments,
        Err(error) => return Some(Err(error)),
    };
    match builtin_function {
        BuiltinFunction::Add => add(arguments),
        BuiltinFunction::Equals => equals(arguments),
        BuiltinFunction::GetArgumentCount => get_argument_count(db, input, arguments),
        BuiltinFunction::Panic => panic(arguments),
        BuiltinFunction::Print => print(arguments),
        BuiltinFunction::TypeOf => type_of(arguments),
        _ => panic!("Unhandled builtin function: {:?}", builtin_function),
    }
}

macro_rules! destructure {
    ($arguments:expr, $enum:pat, $body:block) => {{
        if let $enum = &$arguments[..] {
            $body
        } else {
            Some(Err(Value::Text(format!("Invalid arguments").to_owned())))
        }
    }};
}

fn add(arguments: Vec<Value>) -> DiscoverResult {
    destructure!(arguments, [Value::Int(a), Value::Int(b)], {
        Some(Ok(Value::Int(a + b)))
    })
}

fn equals(arguments: Vec<Value>) -> DiscoverResult {
    destructure!(arguments, [a, b], { Some(Ok((a == b).into())) })
}

fn get_argument_count(db: &dyn Discover, input: Input, arguments: Vec<Value>) -> DiscoverResult {
    destructure!(arguments, [Value::Lambda(function)], {
        // TODO: support parameter counts > 2^64 on 128-bit systems and better
        let expression = match db.find_expression(input, function.id.to_owned())? {
            Expression::Lambda(lambda) => lambda,
            _ => return None,
        };
        Some(Ok((expression.parameters.len() as u64).into()))
    })
}

fn if_else(
    db: &dyn Discover,
    input: Input,
    arguments: Vec<hir::Id>,
    environment: Environment,
) -> DiscoverResult {
    log::error!("{:?}", arguments);
    if let [condition, then, else_] = &arguments[..] {
        let body_id = match db.run_with_environment(
            input.clone(),
            condition.to_owned(),
            environment.to_owned(),
        )? {
            Ok(value) if value == Value::bool_true() => then,
            Ok(value) if value == Value::bool_false() => else_,
            Ok(_) => return None,
            Err(error) => return Some(Err(error)),
        };

        db.run_call(input, body_id.to_owned(), vec![], environment)
    } else {
        Some(Err(Value::Text(
            format!(
                "Builtin if/else called with wrong number of arguments: {}, expected: {}",
                arguments.len(),
                3
            )
            .into(),
        )))
    }
}

fn panic(arguments: Vec<Value>) -> DiscoverResult {
    destructure!(arguments, [value], { Some(Err(value.clone())) })
}

fn print(arguments: Vec<Value>) -> DiscoverResult {
    destructure!(arguments, [value], {
        println!("{:?}", value);
        Some(Ok(Value::nothing()))
    })
}

fn type_of(arguments: Vec<Value>) -> DiscoverResult {
    destructure!(arguments, [value], {
        match value {
            Value::Int(_) => Some(Ok(Value::Symbol("Int".to_owned()))),
            Value::Text(_) => Some(Ok(Value::Symbol("Text".to_owned()))),
            Value::Symbol(_) => Some(Ok(Value::Symbol("Symbol".to_owned()))),
            Value::Lambda(_) => Some(Ok(Value::Symbol("Function".to_owned()))),
        }
    })
}
