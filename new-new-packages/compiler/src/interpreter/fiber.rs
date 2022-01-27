use crate::{
    builtin_functions,
    compiler::hir::{self, Expression, Id},
};
use im::HashMap;
use log;

/// A fiber can execute some byte code. It's "single-threaded", a pure
/// mathematical machine and only communicates with the outside world through
/// channels, which can be provided during instantiation as ambients.
#[derive(Debug)]
pub struct Fiber {
    runner: LambdaRunner,
    status: FiberStatus,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum FiberStatus {
    Running,
    Done(Value),
    Panicked(Value),
}

#[derive(Debug)]
struct LambdaRunner {
    instruction_pointer: usize, // instruction pointer
    environment: Environment,
    lambda: Lambda,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Environment {
    parent: Option<Box<Environment>>,
    bindings: HashMap<Id, Value>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Value {
    Int(u64),
    Text(String),
    Symbol(String),
    Lambda(Lambda),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Lambda {
    captured_environment: Environment,
    pub hir: hir::Lambda,
}

impl Fiber {
    pub fn new(hir: hir::Lambda) -> Self {
        assert_eq!(builtin_functions::VALUES.len(), hir.first_id.0);
        let environment = Environment::new(HashMap::new());
        Self {
            runner: LambdaRunner::new(
                Lambda {
                    captured_environment: environment.clone(),
                    hir: hir,
                },
                vec![],
            ),
            status: FiberStatus::Running,
        }
    }

    pub fn status(&self) -> FiberStatus {
        self.status.clone()
    }

    pub fn run(&mut self) {
        assert_eq!(
            self.status,
            FiberStatus::Running,
            "Called run on Fiber with a status that is not running."
        );

        self.status = match self.runner.run() {
            Ok(value) => FiberStatus::Done(value),
            Err(value) => FiberStatus::Panicked(value),
        };
    }
}

impl LambdaRunner {
    pub fn new(lambda: Lambda, arguments: Vec<Value>) -> Self {
        let environment = Environment {
            parent: Some(Box::new(lambda.captured_environment.clone())),
            bindings: Environment::bindings_from_vec(lambda.hir.first_id, arguments),
        };
        Self {
            instruction_pointer: 0,
            environment,
            lambda,
        }
    }

    fn current_id(&self) -> Id {
        hir::Id(
            self.lambda.hir.first_id.0 + self.lambda.hir.parameter_count + self.instruction_pointer,
        )
    }
    pub fn run(&mut self) -> Result<Value, Value> {
        assert!(!self.lambda.hir.expressions.is_empty());
        while self.instruction_pointer < self.lambda.hir.expressions.len() {
            let expression = self.lambda.hir.get(self.current_id()).unwrap().clone();
            log::trace!("Running instruction {}: {}", self.current_id(), &expression);
            let value = self.run_expression(expression)?;
            self.environment.store(self.current_id(), value);
            self.instruction_pointer += 1;
        }
        Ok(self.environment.get(hir::Id(self.current_id().0 - 1)))
    }
    fn run_expression(&mut self, expression: Expression) -> Result<Value, Value> {
        match expression {
            Expression::Int(int) => Ok(Value::Int(int)),
            Expression::Text(string) => Ok(string.into()),
            Expression::Symbol(symbol) => Ok(Value::Symbol(symbol)),
            Expression::Lambda(lambda) => Ok(Value::Lambda(Lambda {
                captured_environment: self.environment.clone(),
                hir: lambda,
            })),
            Expression::Call {
                function,
                arguments,
            } => {
                let arguments = arguments
                    .into_iter()
                    .map(|it| self.environment.get(it))
                    .collect();

                if let Some(builtin_function) = builtin_functions::VALUES.get(function.0) {
                    return builtin_function.call(arguments, |lambda, arguments| {
                        LambdaRunner::new(lambda, arguments).run()
                    });
                }

                let lambda = match self.environment.get(function) {
                    Value::Lambda(lambda) => lambda,
                    value => {
                        return Err(format!("Tried to call a non-lambda: `{:?}`.", value).into())
                    }
                };
                let function_name = self.lambda.hir.identifiers.get(&function).unwrap();
                log::trace!("Calling function `{}`", &function_name);

                if lambda.hir.parameter_count != arguments.len() {
                    return Err(Value::argument_count_mismatch_text(
                        function_name,
                        lambda.hir.parameter_count,
                        arguments.len(),
                    ));
                }

                let value = LambdaRunner::new(lambda, arguments).run();
                // log::trace!("Lambda returned {:?}", value);
                value
            }
            Expression::Error => panic!("We shouldn't evaluate code that has an error."),
        }
    }
}

impl Environment {
    pub fn new(bindings: HashMap<Id, Value>) -> Environment {
        Self {
            parent: None,
            bindings,
        }
    }
    fn store(&mut self, id: Id, value: Value) {
        assert!(
            !self.bindings.contains_key(&id),
            "Tried to overwrite a value at ID {}: {:?}",
            &id,
            &value
        );
        assert!(self.bindings.insert(id, value).is_none())
    }
    fn get(&self, id: Id) -> Value {
        self.bindings
            .get(&id)
            .map(|it| it.clone())
            .unwrap_or_else(move || {
                self.parent
                    .as_ref()
                    .expect(&format!(
                        "Couldn't find value for ID {} in the environment: {:?}",
                        id,
                        self.clone()
                    ))
                    .get(id)
            })
    }

    fn bindings_from_vec(first_id: Id, values: Vec<Value>) -> HashMap<Id, Value> {
        values
            .into_iter()
            .enumerate()
            .map(|(index, it)| (hir::Id(first_id.0 + index), it))
            .collect()
    }
}
impl Value {
    pub fn nothing() -> Self {
        Value::Symbol("Nothing".to_owned())
    }
    pub fn bool_true() -> Self {
        Value::Symbol("True".to_owned())
    }
    pub fn bool_false() -> Self {
        Value::Symbol("False".to_owned())
    }
    pub fn argument_count_mismatch_text(
        function_name: &str,
        parameter_count: usize,
        argument_count: usize,
    ) -> Value {
        format!(
            "Function `{}` expects {} arguments, but {} were given.",
            function_name, parameter_count, argument_count
        )
        .into()
    }
}

impl From<u64> for Value {
    fn from(value: u64) -> Self {
        Value::Int(value)
    }
}
impl From<String> for Value {
    fn from(value: String) -> Self {
        Value::Text(value)
    }
}
impl From<bool> for Value {
    fn from(value: bool) -> Self {
        if value {
            Value::bool_true()
        } else {
            Value::bool_false()
        }
    }
}
