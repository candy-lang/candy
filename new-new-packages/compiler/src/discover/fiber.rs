use crate::{
    builtin_functions,
    compiler::{
        ast_to_hir::AstToHir,
        hir::{self, Body, Expression, Id},
    },
    input::InputReference,
};
use im::HashMap;
use log;

use super::value::{Environment, Lambda, Value};

/// A fiber can execute some byte code. It's "single-threaded", a pure
/// mathematical machine and only communicates with the outside world through
/// channels, which can be provided during instantiation as ambients.
#[derive(Debug)]
pub struct Fiber {
    runner: BodyRunner,
    status: FiberStatus,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum FiberStatus {
    Running,
    Done(Value),
    Panicked(Value),
}

#[derive(Debug)]
struct BodyRunner {
    instruction_pointer: usize, // instruction pointer
    environment: Environment,
    body: Body,
}

/// The result of a successful evaluation. Either a concrete value that could be
/// used instead of the expression, or an Error indicating that the code will
/// panic with a value.
pub type EvaluationResult = Result<Value, Value>;

/// The result of an attempted evaluation. May be None if the code still
/// contains errors or is impure or too complicated.
pub type DiscoverResult = Option<EvaluationResult>;

#[salsa::query_group(DiscoverStorage)]
pub trait Discover: AstToHir {
    fn run(&self, input_reference: InputReference, id: hir::Id) -> DiscoverResult;
    fn run_with_environment(
        &self,
        input_reference: InputReference,
        id: hir::Id,
        environment: Environment,
    ) -> DiscoverResult;
}

fn run(db: &dyn Discover, input_reference: InputReference, id: hir::Id) -> DiscoverResult {
    db.run_with_environment(input_reference, id, Environment::new(HashMap::new()))
}
fn run_with_environment(
    db: &dyn Discover,
    input_reference: InputReference,
    id: hir::Id,
    environment: Environment,
) -> DiscoverResult {
    let (hir, _) = db.hir(input_reference).unwrap();
    let expression = hir.find(&id);
    match expression {
        Expression::Int(int) => Some(Ok(int.to_owned().into())),
        Expression::Text(string) => Some(Ok(string.to_owned().into())),
        Expression::Symbol(symbol) => Some(Ok(Value::Symbol(symbol.to_owned()))),
        Expression::Lambda(lambda) => Some(Ok(Value::Lambda(Lambda {
            captured_environment: environment.clone(),
            id,
        }))),
        Expression::Body(_) => todo!(),
        Expression::Call {
            function,
            arguments,
        } => {
            let arguments = arguments
                .into_iter()
                .map(|it| run(db, input_reference.clone(), it.to_owned()))
                .collect::<Option<Result<Vec<Value>, Value>>>()?;
            let arguments = match arguments {
                Ok(arguments) => arguments,
                Err(value) => return Some(Err(value)),
            };

            if let Some(builtin_function) = builtin_functions::VALUES.get(function.0) {
                return builtin_function.call(arguments, |lambda, arguments| {
                    BodyRunner::new(lambda, arguments).run()
                });
            }

            let lambda = match environment.get(function) {
                Value::Lambda(lambda) => lambda,
                value => return Err(format!("Tried to call a non-lambda: `{:?}`.", value).into()),
            };
            let lambda_hir = match hir.find(&lambda.id) {
                Expression::Lambda(lambda) => lambda,
                _ => return Some(Err("Tried to call a non-lambda.")),
            }
            
            let function_name = self.body.hir.body.identifiers.get(&function).unwrap();
            log::trace!("Calling function `{}`", &function_name);

            if lambda_hir.parameters.len() != arguments.len() {
                return Err(Value::argument_count_mismatch_text(
                    function_name,
                    lambda_hir.parameters.len(),
                    arguments.len(),
                ));
            }

            let inner_environment = environment.clone();
            for (index, argument) in arguments.iter().enumerate() {
                inner_environment.store(lambda_hir.first_id+ index, argument);
            }

            let value = BodyRunner::new(lambda, arguments).run();
            // log::trace!("Lambda returned {:?}", value);
            value
        }
        Expression::Error => None,
    }
}

impl Fiber {
    pub fn new(body: hir::Body) -> Self {
        assert_eq!(builtin_functions::VALUES.len(), hir.first_id.0);
        let environment = Environment::new(HashMap::new());
        Self {
            runner: BodyRunner::new(
                Lambda {
                    captured_environment: environment.clone(),
                    hir: body,
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

impl BodyRunner {
    pub fn new(body: Lambda, arguments: Vec<Value>) -> Self {
        let environment = Environment {
            parent: Some(Box::new(body.captured_environment.clone())),
            bindings: Environment::bindings_from_vec(body.hir.first_id, arguments),
        };
        Self {
            instruction_pointer: 0,
            environment,
            body,
        }
    }

    fn current_id(&self) -> Id {
        hir::Id(self.body.hir.first_id.0 + self.body.first_id + self.instruction_pointer)
    }
    pub fn run(&mut self) -> Result<Value, Value> {
        assert!(!self.body.hir.expressions.is_empty());
        while self.instruction_pointer < self.body.hir.expressions.len() {
            let expression = self.body.hir.get(self.current_id()).unwrap().clone();
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
                        BodyRunner::new(lambda, arguments).run()
                    });
                }

                let lambda = match self.environment.get(function) {
                    Value::Lambda(lambda) => lambda,
                    value => {
                        return Err(format!("Tried to call a non-lambda: `{:?}`.", value).into())
                    }
                };
                let function_name = self.body.hir.body.identifiers.get(&function).unwrap();
                log::trace!("Calling function `{}`", &function_name);

                if lambda.hir.parameters.len() != arguments.len() {
                    return Err(Value::argument_count_mismatch_text(
                        function_name,
                        lambda.hir.parameters.len(),
                        arguments.len(),
                    ));
                }

                let value = BodyRunner::new(lambda, arguments).run();
                // log::trace!("Lambda returned {:?}", value);
                value
            }
            Expression::Error => panic!("We shouldn't evaluate code that has an error."),
            Expression::Body(_) => todo!(),
        }
    }
}
