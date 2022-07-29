use super::{
    heap::ObjectPointer,
    use_provider::UseProvider,
    value::{Closure, Value},
    Vm,
};
use crate::{builtin_functions::BuiltinFunction, compiler::lir::Instruction, input::Input};
use itertools::Itertools;

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
    pub(super) fn run_builtin_function<U: UseProvider>(
        &mut self,
        use_provider: &U,
        builtin_function: &BuiltinFunction,
        args: &[ObjectPointer],
    ) {
        log::trace!("run_builtin_function: builtin{builtin_function:?}");

        let args = args.iter().map(|it| self.heap.export(*it)).collect_vec();

        let return_value_or_panic_reason = match &builtin_function {
            BuiltinFunction::Add => self.add(args),
            BuiltinFunction::Equals => self.equals(args),
            BuiltinFunction::GetArgumentCount => self.get_argument_count(args),
            BuiltinFunction::IfElse => match self.if_else(use_provider, args) {
                // If successful, IfElse doesn't return a value, but diverges
                // the control flow.
                Ok(()) => return,
                Err(reason) => Err(reason),
            },
            BuiltinFunction::Print => self.print(args),
            BuiltinFunction::StructGet => self.struct_get(args),
            BuiltinFunction::StructGetKeys => self.struct_get_keys(args),
            BuiltinFunction::StructHasKey => self.struct_has_key(args),
            BuiltinFunction::TypeOf => self.type_of(args),
            BuiltinFunction::UseAsset => self.use_asset(use_provider, args),
            BuiltinFunction::UseLocalModule => {
                // If successful, UseLocalModule doesn't return a value, but
                // diverges the control flow.
                match self.use_local_module(use_provider, args) {
                    Ok(()) => return,
                    Err(reason) => Err(reason),
                }
            }
        };
        let return_value = match return_value_or_panic_reason {
            Ok(value) => value,
            Err(reason) => self.panic(reason),
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
        destructure!(args, [Value::Closure(Closure { num_args, .. })], {
            Ok((*num_args as u64).into())
        })
    }

    fn if_else<U: UseProvider>(
        &mut self,
        use_provider: &U,
        args: Vec<Value>,
    ) -> Result<(), String> {
        destructure!(
            args,
            [
                Value::Symbol(condition),
                Value::Closure(then_closure),
                Value::Closure(else_closure)
            ],
            {
                if then_closure.num_args > 0 {
                    return Err(format!("IfElse expects a closure without arguments as the then, got one with {} arguments.", then_closure.num_args));
                }
                if else_closure.num_args > 0 {
                    return Err(format!("IfElse expects a closure without arguments as the else, got one with {} arguments.", else_closure.num_args));
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
                    Value::Closure(then_closure.clone())
                } else {
                    Value::Closure(else_closure.clone())
                });
                log::debug!(
                    "IfElse executing the closure: {:?}",
                    self.heap.export_without_dropping(closure_object)
                );
                self.data_stack.push(closure_object);
                self.run_instruction(use_provider, Instruction::Call { num_args: 0 });
                Ok(())
            }
        )
    }

    fn print(&mut self, args: Vec<Value>) -> Result<Value, String> {
        destructure!(args, [Value::Text(message)], {
            log::info!("{message:?}");
            Ok(Value::nothing())
        })
    }

    fn struct_get(&mut self, args: Vec<Value>) -> Result<Value, String> {
        destructure!(args, [Value::Struct(struct_), key], {
            match struct_.get(key) {
                Some(value) => Ok(value.clone()),
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

    fn use_asset<U: UseProvider>(
        &mut self,
        use_provider: &U,
        args: Vec<Value>,
    ) -> Result<Value, String> {
        let (current_path, target) = Self::parse_current_path_and_target(args)?;
        let target = UseTarget::parse(&target)?;
        let input = target.resolve_asset(&current_path)?;
        let content = use_provider.use_asset(input)?;
        Ok(Value::list(
            content
                .iter()
                .map(|byte| Value::Int(*byte as u64))
                .collect_vec(),
        ))
    }

    fn use_local_module<U: UseProvider>(
        &mut self,
        use_provider: &U,
        args: Vec<Value>,
    ) -> Result<(), String> {
        let (current_path, target) = Self::parse_current_path_and_target(args)?;
        let target = UseTarget::parse(&target)?;
        let possible_inputs = target.resolve_local_module(&current_path)?;
        let (input, lir) = 'find_existing_input: {
            for input in possible_inputs {
                if let Some(lir) = use_provider.use_local_module(input.clone()) {
                    break 'find_existing_input (input, lir);
                }
            }
            return Err("couldn't import module".to_string());
        };

        let module_closure = Value::Closure(Closure::of_lir(input, lir));
        let address = self.heap.import(module_closure);
        self.data_stack.push(address);
        self.run_instruction(use_provider, Instruction::Call { num_args: 0 });
        Ok(())
    }

    fn parse_current_path_and_target(args: Vec<Value>) -> Result<(Vec<String>, String), String> {
        destructure!(
            args,
            [Value::Struct(current_path_struct), Value::Text(target)],
            {
                // `current_path_struct` is set by us and not users, hence we don't have to validate it that strictly.
                let mut current_path = vec![];
                let mut index = 0;
                while let Some(component) = current_path_struct.get(&Value::Int(index)) {
                    current_path.push(component.clone().try_into_text().unwrap());
                    index += 1;
                }
                Ok((current_path, target.to_string()))
            }
        )
    }
}

struct UseTarget {
    parent_navigations: usize,
    path: String,
}
impl UseTarget {
    const PARENT_NAVIGATION_CHAR: char = '.';

    fn parse(mut target: &str) -> Result<Self, String> {
        let parent_navigations = {
            let mut navigations = 0;
            while target.starts_with(UseTarget::PARENT_NAVIGATION_CHAR) {
                navigations += 1;
                target = &target[UseTarget::PARENT_NAVIGATION_CHAR.len_utf8()..];
            }
            match navigations {
                0 => return Err("the target must start with at least one dot".to_string()),
                i => i - 1, // two dots means one parent navigation
            }
        };
        let path = {
            if !target
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '.')
            {
                return Err("the target name can only contain letters and dots".to_string());
            }
            target.to_string()
        };
        Ok(UseTarget {
            parent_navigations,
            path,
        })
    }

    fn resolve_asset(&self, current_path: &[String]) -> Result<Input, String> {
        let mut path = current_path.to_owned();
        if self.parent_navigations == 0 && path.last() != Some(&".candy".to_string()) {
            return Err(
                "importing child files (starting with a single dot) only works from `.candy` files"
                    .to_string(),
            );
        }
        for _ in 0..self.parent_navigations {
            if path.pop() == None {
                return Err("too many parent navigations".to_string());
            }
        }
        path.push(self.path.to_string());
        Ok(Input::File(path.clone()))
    }

    fn resolve_local_module(&self, current_path: &[String]) -> Result<Vec<Input>, String> {
        if self.path.contains('.') {
            return Err("the target name contains a file ending".to_string());
        }

        let mut path = current_path.to_owned();
        for _ in 0..self.parent_navigations {
            if path.pop() == None {
                return Err("too many parent navigations".to_string());
            }
        }
        let possible_paths = vec![
            path.clone()
                .into_iter()
                .chain([format!("{}.candy", self.path)])
                .collect_vec(),
            path.clone()
                .into_iter()
                .chain([self.path.to_string(), ".candy".to_string()])
                .collect_vec(),
        ];
        Ok(possible_paths.into_iter().map(Input::File).collect_vec())
    }
}
