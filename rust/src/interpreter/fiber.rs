use crate::compiler::hir::{self, Expression, Id};
use log;
use std::collections::HashMap;

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
    hir: hir::Lambda,
}

impl Fiber {
    pub fn new(builtin_values: Vec<Value>, hir: hir::Lambda) -> Self {
        assert!(builtin_values.len() == hir.first_id);
        let environment = Environment::new(Environment::bindings_from_vec(0, builtin_values));
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
            lambda: lambda,
        }
    }

    fn current_id(&self) -> Id {
        self.lambda.hir.first_id + self.lambda.hir.parameter_count + self.instruction_pointer
    }
    pub fn run(&mut self) -> Result<Value, Value> {
        assert!(!self.lambda.hir.expressions.is_empty());
        while self.instruction_pointer < self.lambda.hir.expressions.len() {
            let expression = self.lambda.hir.get(self.current_id()).unwrap().clone();
            log::debug!(
                "Running instruction {}: {}",
                self.current_id(),
                expression.clone()
            );
            let value = self.run_expression(expression)?;
            self.environment.store(self.current_id(), value);
            self.instruction_pointer += 1;
        }
        Ok(self.environment.get(self.current_id() - 1))
    }
    fn run_expression(&mut self, expression: Expression) -> Result<Value, Value> {
        match expression {
            Expression::Int(int) => Ok(Value::Int(int)),
            Expression::Text(string) => Ok(Value::Text(string)),
            Expression::Symbol(symbol) => Ok(Value::Symbol(symbol)),
            Expression::Lambda(lambda) => Ok(Value::Lambda(Lambda {
                captured_environment: self.environment.clone(),
                hir: lambda,
            })),
            Expression::Call {
                function,
                arguments,
            } => {
                let lambda = match self.environment.get(function) {
                    Value::Lambda(lambda) => lambda,
                    value => {
                        return Err(Value::Text(format!(
                            "Tried to call a non-lambda: `{:?}`.",
                            value
                        )))
                    }
                };
                let function_name = self.lambda.hir.identifiers.get(&function).unwrap();
                log::debug!("Calling function `{}`", function_name.clone());

                if lambda.hir.parameter_count != arguments.len() {
                    return Err(Value::Text(format!(
                        "Function `{}` expects {} arguments, but {} were given.",
                        function_name,
                        lambda.hir.parameter_count,
                        arguments.len()
                    )));
                }

                let value = LambdaRunner::new(
                    lambda,
                    arguments
                        .into_iter()
                        .map(|it| self.environment.get(it))
                        .collect(),
                )
                .run();
                // log::debug!("Lambda returned {:?}", value);
                value
            }
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
            self.bindings.insert(id, value.clone()).is_none(),
            "Tried to overwrite a value at ID {}: {:?}",
            id,
            value
        );
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
            .map(|(index, it)| (first_id + index, it))
            .collect()
    }
}
