use im::HashMap;
use itertools::Itertools;

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

const TRACE_BUILTIN_FUNCTION_CALLS: bool = false;

pub fn run_builtin_function(
    db: &dyn Discover,
    import_chain: &[Input],
    builtin_function: BuiltinFunction,
    arguments: Vec<hir::Id>,
    environment: Environment,
) -> DiscoverResult {
    if TRACE_BUILTIN_FUNCTION_CALLS {
        log::trace!(
            "run_builtin_function: builtin{:?} {}",
            builtin_function,
            arguments.iter().join(" ")
        );
    }

    // Handle builtin functions that don't need to resolve the arguments.
    match builtin_function {
        BuiltinFunction::IfElse => return if_else(db, import_chain, arguments, environment),
        _ => {}
    }

    let arguments = arguments
        .iter()
        .map(|it| environment.get(it).transitive())
        .collect::<DiscoverResult<_>>()?;
    match builtin_function {
        BuiltinFunction::Add => add(arguments),
        BuiltinFunction::Equals => equals(arguments),
        BuiltinFunction::GetArgumentCount => get_argument_count(db, arguments),
        BuiltinFunction::Panic => panic(arguments),
        BuiltinFunction::Print => print(arguments),
        BuiltinFunction::StructGet => struct_get(arguments),
        BuiltinFunction::StructGetKeys => struct_get_keys(arguments),
        BuiltinFunction::StructHasKey => struct_has_key(arguments),
        BuiltinFunction::TypeOf => type_of(arguments),
        BuiltinFunction::Use => use_(db, import_chain, arguments),
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

fn get_argument_count(db: &dyn Discover, arguments: Vec<Value>) -> DiscoverResult {
    destructure!(arguments, [Value::Lambda(function)], {
        // TODO: support parameter counts > 2^64 on 128-bit systems and better
        let expression = match db.find_expression(function.id.to_owned()).unwrap() {
            Expression::Lambda(lambda) => lambda,
            _ => panic!("Lambda's function"),
        };
        Value::Int(expression.parameters.len() as u64).into()
    })
}

fn if_else(
    db: &dyn Discover,
    import_chain: &[Input],
    arguments: Vec<hir::Id>,
    environment: Environment,
) -> DiscoverResult {
    if let [condition, then, else_] = &arguments[..] {
        let body_id = match environment.get(condition)? {
            value if value == Value::bool(true) => then,
            value if value == Value::bool(false) => else_,
            value => {
                return DiscoverResult::panic(format!(
                    "Condition must be a boolean, but was {:?}.",
                    value
                ));
            }
        };

        run_call(db, import_chain, body_id.to_owned(), vec![], environment)
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

fn use_(db: &dyn Discover, import_chain: &[Input], arguments: Vec<Value>) -> DiscoverResult {
    destructure!(arguments, [current_path, target], {
        // `current_path` is set by us and not users, hence we don't have to validate it that strictly.
        let current_path_struct = match current_path {
            Value::Struct(value) => value,
            _ => unreachable!(),
        };
        let mut current_path = vec![];
        let mut index = 0;
        loop {
            if let Some(component) = current_path_struct.get(&Value::Int(index)) {
                match component {
                    Value::Text(component) => current_path.push(component.clone()),
                    _ => unreachable!(),
                }
            } else {
                break;
            }
            index += 1;
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

        if target.parent_navigations > current_path.len() {
            return DiscoverResult::panic("Too many parent navigations.".to_owned());
        }

        let inputs = target.resolve(&current_path[..]);
        let input = match inputs
            .iter()
            .filter(|&it| db.get_input(it.to_owned()).is_some())
            .next()
        {
            Some(target) => target,
            None => {
                return DiscoverResult::panic(format!(
                    "Target doesn't exist. Checked the following path(s): {}",
                    inputs.iter().map(|it| format!("{}", it)).join(", ")
                ));
            }
        };

        if import_chain.contains(input) {
            return DiscoverResult::CircularImport(import_chain.to_owned());
        }

        let (hir, _) = db.hir(input.clone()).unwrap();
        let discover_result = db.run_all(input.to_owned(), import_chain.to_owned());

        hir.identifiers
            .iter()
            .map(|(id, key)| {
                let mut key = key.to_owned();
                key.get_mut(0..1).unwrap().make_ascii_uppercase();
                let key = Value::Symbol(key.to_owned());

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

    fn resolve(&self, current_path: &[String]) -> Vec<Input> {
        let mut path = current_path.to_owned();
        if self.parent_navigations == 0 {
            assert!(!path.is_empty());
            let last = path.last_mut().unwrap();
            if last == ".candy" {
                path.pop();
            } else {
                *last = last
                    .strip_suffix(".candy")
                    .expect("File name must end with `.candy`.")
                    .to_owned();
            }
        } else {
            for _ in 0..self.parent_navigations {
                if path.is_empty() {
                    return vec![];
                }
                path.pop();
            }
        }

        for part in &self.path {
            path.push(part.to_owned());
        }

        let mut result = vec![];

        let mut subdirectory = path.clone();
        subdirectory.push(".candy".to_owned());
        result.push(Input::File(subdirectory));

        if path.len() >= 1 {
            let last = path.last_mut().unwrap();
            *last = format!("{}.candy", last);
            result.push(Input::File(path));
        }
        result
    }
}
