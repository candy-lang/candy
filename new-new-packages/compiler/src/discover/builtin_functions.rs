use std::path::{Path, PathBuf};

use im::HashMap;
use itertools::Itertools;

use crate::{
    builtin_functions::BuiltinFunction,
    compiler::hir::{self, Expression},
    database::PROJECT_DIRECTORY,
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
        BuiltinFunction::Use => use_(db, arguments),
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

fn use_(db: &dyn Discover, arguments: Vec<Value>) -> DiscoverResult {
    destructure!(arguments, [current_path, target], {
        let current_path_string = match current_path {
            Value::Text(value) => value,
            it => {
                return DiscoverResult::panic(format!(
                    "`use` expected a text as its first parameter, but received: {:?}",
                    it
                ))
            }
        };
        // `current_path` is set by us and not users, hence we don't have to validate it that strictly.
        let mut current_path = PathBuf::new();
        current_path.push(".");
        for segment in current_path_string.split('/') {
            current_path.push(segment);
        }

        let target = match target {
            Value::Text(value) => value,
            it => {
                return DiscoverResult::panic(format!(
                    "`use` expected a text as its second parameter, but received: {:?}",
                    it
                ))
            }
        };
        let target = match UseTarget::parse(target) {
            Ok(target) => target,
            Err(error) => return DiscoverResult::panic(error),
        };

        if target.parent_navigations > current_path.components().count() - 1 {
            return DiscoverResult::panic("Too many parent navigations.".to_owned());
        }

        let project_dir = PROJECT_DIRECTORY.lock().unwrap().clone().unwrap();
        let target_paths = target.resolve(&current_path);
        let target = match target_paths
            .iter()
            .filter(|it| project_dir.join(it).exists())
            .next()
        {
            Some(target) => target,
            None => {
                return DiscoverResult::panic(format!(
                    "Target doesn't exist. Checked the following path(s): {}",
                    target_paths
                        .iter()
                        .map(|it| it.to_str().unwrap())
                        .join(", ")
                ));
            }
        };

        let input = Input::File(project_dir.join(target).to_owned());
        let (hir, _) = db.hir(input.clone()).unwrap();
        let discover_result = db.run_all(input);

        hir.identifiers
            .iter()
            .map(|(id, key)| {
                let key = Value::Text(key.to_owned());
                let value = match discover_result.get(id) {
                    Some(value) => value.to_owned()?,
                    None => return DiscoverResult::ErrorInHir,
                };
                DiscoverResult::Value((key, value))
            })
            .collect::<DiscoverResult<HashMap<Value, Value>>>()
            .map(|it| Value::Struct(it))
    })
}
struct UseTarget {
    parent_navigations: usize,
    path: Vec<String>,
}
impl UseTarget {
    const PARENT_NAVIGATION_CHAR: char = '.';

    fn parse(target: &str) -> Result<Self, String> {
        let mut parent_navigations = 0;
        let mut target = target;
        while target.chars().next() == Some(UseTarget::PARENT_NAVIGATION_CHAR) {
            parent_navigations += 1;
            target = &target[UseTarget::PARENT_NAVIGATION_CHAR.len_utf8()..];
        }

        let mut path = vec![];
        loop {
            let mut chars = vec![];
            while let Some(c) = target.chars().next() {
                if c == UseTarget::PARENT_NAVIGATION_CHAR {
                    break;
                }
                chars.push(c);
                target = &target[c.len_utf8()..];
            }

            if target.is_empty() {
                path.push(chars.into_iter().join(""));
                break;
            }

            if chars.is_empty() {
                return Err("Target contains consecutive dots (`.`) in the path.".to_owned());
            }

            path.push(chars.into_iter().join(""));
        }
        Ok(UseTarget {
            parent_navigations,
            path,
        })
    }

    fn resolve(&self, current_path: &Path) -> Vec<PathBuf> {
        let mut path = current_path;
        for _ in 0..self.parent_navigations {
            path = path.parent().unwrap();
        }

        let mut path = path.to_path_buf();
        for part in &self.path {
            path.push(part);
        }

        let mut result = vec![];

        let mut subdirectory = path.clone();
        subdirectory.push(".candy");
        result.push(subdirectory);

        if path.components().count() > 1 {
            path.set_file_name(format!(
                "{}.candy",
                path.file_name().unwrap().to_str().unwrap()
            ));
            result.push(path);
        }
        result
    }
}
