use im::HashMap;

use crate::{
    builtin_functions::BuiltinFunction,
    compiler::hir::{self, Expression},
    discover::run::run_call,
    input::Input,
};

use super::{
    result::DiscoverResult,
    run::Discover,
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

    let arguments = arguments
        .iter()
        .map(|it| environment.get(it).unwrap())
        .collect::<DiscoverResult<_>>()?;
    match builtin_function {
        BuiltinFunction::Add => add(arguments),
        BuiltinFunction::Equals => equals(arguments),
        BuiltinFunction::GetArgumentCount => get_argument_count(db, input, arguments),
        BuiltinFunction::Panic => panic(arguments),
        BuiltinFunction::Print => print(arguments),
        BuiltinFunction::StructGet => struct_get(arguments),
        BuiltinFunction::StructGetKeys => struct_get_keys(arguments),
        BuiltinFunction::StructHasKey => struct_has_key(arguments),
        BuiltinFunction::TypeOf => type_of(arguments),
        _ => panic!("Unhandled builtin function: {:?}", builtin_function),
    }
}

macro_rules! destructure {
    ($arguments:expr, $enum:pat, $body:block) => {{
        if let $enum = &$arguments[..] {
            $body
        } else {
            DiscoverResult::panic(format!("Invalid arguments").to_owned())
        }
    }};
}

fn add(arguments: Vec<Value>) -> DiscoverResult {
    destructure!(arguments, [Value::Int(a), Value::Int(b)], {
        Value::Int(a + b).into()
    })
}

fn equals(arguments: Vec<Value>) -> DiscoverResult {
    destructure!(arguments, [a, b], { Value::bool(a == b).into() })
}

fn get_argument_count(db: &dyn Discover, input: Input, arguments: Vec<Value>) -> DiscoverResult {
    destructure!(arguments, [Value::Lambda(function)], {
        // TODO: support parameter counts > 2^64 on 128-bit systems and better
        let expression = match db.find_expression(input, function.id.to_owned()).unwrap() {
            Expression::Lambda(lambda) => lambda,
            _ => panic!("Lambda's function"),
        };
        Value::Int(expression.parameters.len() as u64).into()
    })
}

fn if_else(
    db: &dyn Discover,
    input: Input,
    arguments: Vec<hir::Id>,
    environment: Environment,
) -> DiscoverResult {
    if let [condition, then, else_] = &arguments[..] {
        let body_id = match environment.get(condition).unwrap()? {
            value if value == Value::bool(true) => then,
            value if value == Value::bool(false) => else_,
            value => {
                return DiscoverResult::panic(format!(
                    "Condition must be a boolean, but was {:?}.",
                    value
                ));
            }
        };

        run_call(db, input, body_id.to_owned(), vec![], environment)
    } else {
        DiscoverResult::panic(format!(
            "Builtin if/else called with wrong number of arguments: {}, expected: {}",
            arguments.len(),
            3
        ))
    }
}

fn panic(arguments: Vec<Value>) -> DiscoverResult {
    destructure!(arguments, [value], { DiscoverResult::Panic(value.clone()) })
}

fn print(arguments: Vec<Value>) -> DiscoverResult {
    destructure!(arguments, [value], {
        println!("{:?}", value);
        Value::nothing().into()
    })
}

fn struct_get(arguments: Vec<Value>) -> DiscoverResult {
    destructure!(arguments, [struct_, key], {
        let struct_ = expect_struct("builtinStructGet".to_owned(), struct_)?;
        struct_
            .get(key)
            .map(|value| value.clone().into())
            .unwrap_or_else(|| {
                DiscoverResult::panic(format!("Struct does not contain key {:?}", key))
            })
    })
}
fn struct_get_keys(arguments: Vec<Value>) -> DiscoverResult {
    destructure!(arguments, [struct_], {
        let struct_ = expect_struct("builtinStructGetKeys".to_owned(), struct_)?;
        Value::list(struct_.keys().cloned().collect()).into()
    })
}
fn struct_has_key(arguments: Vec<Value>) -> DiscoverResult {
    destructure!(arguments, [struct_, key], {
        let struct_ = expect_struct("builtinStructHasKey".to_owned(), struct_)?;
        Value::bool(struct_.contains_key(key)).into()
    })
}
fn expect_struct(function_name: String, value: &Value) -> DiscoverResult<&HashMap<Value, Value>> {
    match value {
        Value::Struct(struct_) => struct_.into(),
        _ => {
            return DiscoverResult::panic(format!(
                "`{}` expected a struct as its first parameter, but received: {:?}",
                function_name, value
            ))
        }
    }
}

fn type_of(arguments: Vec<Value>) -> DiscoverResult {
    destructure!(arguments, [value], {
        match value {
            Value::Int(_) => Value::Symbol("Int".to_owned()).into(),
            Value::Text(_) => Value::Symbol("Text".to_owned()).into(),
            Value::Symbol(_) => Value::Symbol("Symbol".to_owned()).into(),
            Value::Struct(_) => Value::Symbol("Struct".to_owned()).into(),
            Value::Lambda(_) => Value::Symbol("Function".to_owned()).into(),
        }
    })
}
        })
    }

    fn resolve(&self, current_path: &Path) -> Vec<PathBuf> {}
}
