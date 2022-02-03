use crate::builtin_functions::BuiltinFunction;

use super::value::{Lambda, Value};

macro_rules! destructure {
    ($arguments:expr, $enum:pat, $body:block) => {{
        if let $enum = &$arguments[..] {
            $body
        } else {
            panic!()
        }
    }};
}

macro_rules! generate_call {
    ($function_name:ident $(, $argument_names:ident)*) => {
        fn $function_name<F>(arguments: Vec<Value>, call: F) -> Result<Value, Value>
        where
            F: Fn(Lambda, Vec<Value>) -> Result<Value, Value>,
        {
            destructure!(arguments, [Value::Lambda(function), $($argument_names),*], {
                call(function.clone(), vec![$($argument_names.clone()),*])
            })
      }
    };
}

impl BuiltinFunction {
    pub fn call<F>(&self, arguments: Vec<Value>, call: F) -> Result<Value, Value>
    where
        F: Fn(Lambda, Vec<Value>) -> Result<Value, Value>,
    {
        match self {
            BuiltinFunction::Add => Ok(Self::add(arguments)),
            BuiltinFunction::Call0 => Self::call0(arguments, call),
            BuiltinFunction::Call1 => Self::call1(arguments, call),
            BuiltinFunction::Call2 => Self::call2(arguments, call),
            BuiltinFunction::Call3 => Self::call3(arguments, call),
            BuiltinFunction::Call4 => Self::call4(arguments, call),
            BuiltinFunction::Call5 => Self::call5(arguments, call),
            BuiltinFunction::Equals => Ok(Self::equals(arguments)),
            BuiltinFunction::GetArgumentCount => Ok(Self::get_argument_count(arguments)),
            BuiltinFunction::IfElse => Self::if_else(arguments, call),
            BuiltinFunction::Panic => Self::panic(arguments),
            BuiltinFunction::Print => Ok(Self::print(arguments)),
            BuiltinFunction::TypeOf => Ok(Self::type_of(arguments)),
        }
    }

    fn add(arguments: Vec<Value>) -> Value {
        destructure!(arguments, [Value::Int(a), Value::Int(b)], {
            Value::Int(a + b)
        })
    }

    generate_call!(call0);
    generate_call!(call1, argument0);
    generate_call!(call2, argument0, argument1);
    generate_call!(call3, argument0, argument1, argument2);
    generate_call!(call4, argument0, argument1, argument2, argument3);
    generate_call!(call5, argument0, argument1, argument2, argument3, argument4);

    fn equals(arguments: Vec<Value>) -> Value {
        destructure!(arguments, [a, b], { (a == b).into() })
    }

    fn get_argument_count(arguments: Vec<Value>) -> Value {
        destructure!(arguments, [Value::Lambda(function)], {
            // TODO: support parameter counts > 2^64 on 128-bit systems and better
            (function.hir.parameters.len() as u64).into()
        })
    }

    fn if_else<F>(arguments: Vec<Value>, call: F) -> Result<Value, Value>
    where
        F: Fn(Lambda, Vec<Value>) -> Result<Value, Value>,
    {
        destructure!(
            arguments,
            [
                Value::Symbol(condition),
                Value::Lambda(then),
                Value::Lambda(else_)
            ],
            {
                match condition.as_str() {
                    "True" => call(then.clone(), vec![]),
                    "False" => call(else_.clone(), vec![]),
                    _ => panic!(),
                }
            }
        )
    }

    fn panic(arguments: Vec<Value>) -> Result<Value, Value> {
        destructure!(arguments, [value], { Err(value.clone()) })
    }

    fn print(arguments: Vec<Value>) -> Value {
        destructure!(arguments, [value], {
            println!("{:?}", value);
            Value::nothing()
        })
    }

    fn type_of(arguments: Vec<Value>) -> Value {
        destructure!(arguments, [value], {
            match value {
                Value::Int(_) => Value::Symbol("Int".to_owned()),
                Value::Text(_) => Value::Symbol("Text".to_owned()),
                Value::Symbol(_) => Value::Symbol("Symbol".to_owned()),
                Value::Lambda(_) => Value::Symbol("Function".to_owned()),
            }
        })
    }
}

pub trait DestructureTuple<T> {
    fn tuple2(self, function_name: &str) -> Result<(T, T), Value>;
}
impl<T> DestructureTuple<T> for Vec<T> {
    fn tuple2(self, function_name: &str) -> Result<(T, T), Value> {
        if self.len() != 2 {
            Err(Value::argument_count_mismatch_text(
                function_name,
                self.len(),
                2,
            ))
        } else {
            let mut iter = self.into_iter();
            let first = iter.next().unwrap();
            let second = iter.next().unwrap();
            assert!(matches!(iter.next(), None));
            Ok((first, second))
        }
    }
}
