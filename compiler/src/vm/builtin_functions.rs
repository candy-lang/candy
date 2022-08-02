use super::{
    heap::ObjectPointer,
    use_provider::UseProvider,
    value::{Closure, Value},
    Vm,
};
use crate::{builtin_functions::BuiltinFunction, compiler::lir::Instruction, module::Module};
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
}
