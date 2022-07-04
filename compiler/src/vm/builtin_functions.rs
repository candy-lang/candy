use super::{heap::ObjectPointer, value::Value, Vm};
use crate::{builtin_functions::BuiltinFunction, compiler::lir::Instruction, input::Input};
use itertools::Itertools;
use log;

macro_rules! destructure {
    ($args:expr, $enum:pat, $body:block) => {{
        if let $enum = &$args[..] {
            $body
        } else {
            Err(format!("a builtin function received invalid arguments"))
        }
    }};
}

impl Vm {
    pub(super) fn run_builtin_function(
        &mut self,
        builtin_function: &BuiltinFunction,
        args: &[ObjectPointer],
    ) {
        log::trace!("run_builtin_function: builtin{builtin_function:?}");

        let args = args.iter().map(|it| self.heap.export(*it)).collect_vec();

        let return_value_or_panic_message = match &builtin_function {
            BuiltinFunction::Add => self.add(args),
            BuiltinFunction::Equals => self.equals(args),
            BuiltinFunction::GetArgumentCount => self.get_argument_count(args),
            BuiltinFunction::IfElse => match self.if_else(args) {
                // If successful, builtinIfElse doesn't return a value, but
                // diverges the control flow.
                Ok(()) => return,
                Err(message) => Err(message),
            },
            BuiltinFunction::Panic => self.panic_builtin(args).map(|_| panic!()),
            BuiltinFunction::Print => self.print(args),
            BuiltinFunction::StructGet => self.struct_get(args),
            BuiltinFunction::StructGetKeys => self.struct_get_keys(args),
            BuiltinFunction::StructHasKey => self.struct_has_key(args),
            BuiltinFunction::TypeOf => self.type_of(args),
            BuiltinFunction::Use => self.use_(args),
        };
        let return_value = match return_value_or_panic_message {
            Ok(value) => value,
            Err(panic_message) => self.panic(panic_message),
        };

        let return_object = self.heap.import(return_value);
        self.data_stack.push(return_object);
    }

    fn add(&mut self, args: Vec<Value>) -> Result<Value, String> {
        destructure!(args, [Value::Int(a), Value::Int(b)], { Ok((a + b).into()) })
    }

    fn equals(&mut self, args: Vec<Value>) -> Result<Value, String> {
        destructure!(args, [a, b], { Ok((a == b).into()) })
    }

    fn get_argument_count(&mut self, args: Vec<Value>) -> Result<Value, String> {
        destructure!(args, [Value::Closure { num_args, .. }], {
            Ok((*num_args as u64).into())
        })
    }

    fn if_else(&mut self, args: Vec<Value>) -> Result<(), String> {
        destructure!(
            args,
            [
                Value::Symbol(condition),
                Value::Closure {
                    captured: then_captured,
                    num_args: then_num_args,
                    body: then_body
                },
                Value::Closure {
                    captured: else_captured,
                    num_args: else_num_args,
                    body: else_body
                }
            ],
            {
                if *then_num_args > 0 {
                    return Err(format!("IfElse expects a closure without arguments as the then, got one with {then_num_args} arguments."));
                }
                if *else_num_args > 0 {
                    return Err(format!("IfElse expects a closure without arguments as the else, got one with {else_num_args} arguments."));
                }
                let condition = match condition.as_str() {
                    "True" => true,
                    "False" => false,
                    _ => {
                        return Err(format!(
                            "IfElse expected True or False as a condition, but got {condition}.",
                        ));
                    }
                };

                let closure_object = self.heap.import(if condition {
                    Value::Closure {
                        captured: then_captured.to_owned(),
                        num_args: *then_num_args,
                        body: then_body.to_owned(),
                    }
                } else {
                    Value::Closure {
                        captured: else_captured.to_owned(),
                        num_args: *else_num_args,
                        body: else_body.to_owned(),
                    }
                });
                log::debug!(
                    "IfElse executing the closure: {:?}",
                    self.heap.export_without_dropping(closure_object)
                );
                self.data_stack.push(closure_object);
                self.run_instruction(Instruction::Call { num_args: 0 });
                Ok(())
            }
        )
    }

    fn panic_builtin(&mut self, args: Vec<Value>) -> Result<!, String> {
        destructure!(args, [Value::Text(message)], { Err(message.to_string()) })
    }

    fn print(&mut self, args: Vec<Value>) -> Result<Value, String> {
        destructure!(args, [Value::Text(message)], {
            println!("{:?}", message);
            Ok(Value::nothing())
        })
    }

    fn struct_get(&mut self, args: Vec<Value>) -> Result<Value, String> {
        destructure!(args, [Value::Struct(struct_), key], {
            match struct_.get(&key) {
                Some(value) => Ok(value.clone().into()),
                None => Err(format!("Struct does not contain key {key:?}.")),
            }
        })
    }

    fn struct_get_keys(&mut self, args: Vec<Value>) -> Result<Value, String> {
        destructure!(args, [Value::Struct(struct_)], {
            Ok(Value::list(struct_.keys().cloned().collect()))
        })
    }

    fn struct_has_key(&mut self, args: Vec<Value>) -> Result<Value, String> {
        destructure!(args, [Value::Struct(struct_), key], {
            Ok((struct_.contains_key(key)).into())
        })
    }

    fn type_of(&mut self, args: Vec<Value>) -> Result<Value, String> {
        destructure!(args, [value], {
            Ok(Value::Symbol(
                match &value {
                    Value::Int(_) => "Int",
                    Value::Text(_) => "Text",
                    Value::Symbol(_) => "Symbol",
                    Value::Struct(_) => "Struct",
                    Value::Closure { .. } => "Function",
                    Value::Builtin { .. } => "Builtin",
                }
                .to_owned(),
            ))
        })
    }

    fn use_(&mut self, args: Vec<Value>) -> Result<Value, String> {
        destructure!(
            args,
            [Value::Struct(current_path_struct), Value::Text(target)],
            {
                let mut current_path = vec![];
                let mut index = 0;
                while let Some(component) = current_path_struct.get(&Value::Int(index)) {
                    current_path.push(component.clone().try_into_text().map_err(|it| format!("builtinUse expects a struct as current path with only textual paths as values, but the map contains the value {it} for key {index}")));
                    index += 1;
                }

                let target = UseTarget::parse(&target)?;

                if target.parent_navigations > current_path.len() {
                    return Err("Too many parent navigations.".to_string());
                }

                Ok(Value::Symbol("Used".to_string()))
            }
        )

        // let inputs = target.resolve(&current_path[..]);
        // let input = match inputs
        //     .iter()
        //     .filter(|&it| db.get_input(it.to_owned()).is_some())
        //     .next()
        // {
        //     Some(target) => target,
        //     None => {
        //         return self.panic(format!(
        //             "Target doesn't exist. Checked the following path(s): {}",
        //             inputs.iter().map(|it| format!("{}", it)).join(", ")
        //         ));
        //     }
        // };

        // TODO: Continue implementing use.
        // let (lir, _) = db.lir(input.clone()).unwrap();
        // TODO: Run LIR.
        // let discover_result = db.run_all(input.to_owned(), import_chain.to_owned());

        // TODO: Put public identifiers into map.
        // hir.identifiers
        //     .iter()
        //     .map(|(id, key)| {
        //         let mut key = key.to_owned();
        //         key.get_mut(0..1).unwrap().make_ascii_uppercase();
        //         let key = Value::Symbol(key.to_owned());

        //         let value = match discover_result.get(id) {
        //             Some(value) => value.to_owned()?,
        //             None => return DiscoverResult::ErrorInHir,
        //         };

        //         DiscoverResult::Value((key, value))
        //     })
        //     .collect::<DiscoverResult<HashMap<Value, Value>>>()
        //     .map(|it| Value::Struct(it))
    }
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
            *last = format!("{last}.candy");
            result.push(Input::File(path));
        }
        result
    }
}
