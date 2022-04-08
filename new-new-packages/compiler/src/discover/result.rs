use std::{
    convert::Infallible,
    iter::FromIterator,
    ops::{ControlFlow, FromResidual, Try},
};

use super::value::Value;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum DiscoverResult<T = Value> {
    Value(T),
    Panic(Value),
    DependsOnParameter,
    PreviousExpressionPanics,
    ErrorInHir,
}
impl<T> DiscoverResult<T> {
    pub fn panic(message: String) -> Self {
        DiscoverResult::Panic(Value::Text(message))
    }

    pub fn value(self) -> Option<T> {
        match self {
            DiscoverResult::Value(value) => Some(value),
            _ => None,
        }
    }
    pub fn map<U, F: FnOnce(T) -> U>(self, op: F) -> DiscoverResult<U> {
        match self {
            DiscoverResult::Value(value) => DiscoverResult::Value(op(value)),
            DiscoverResult::Panic(value) => DiscoverResult::Panic(value),
            DiscoverResult::DependsOnParameter => DiscoverResult::DependsOnParameter,
            DiscoverResult::PreviousExpressionPanics => DiscoverResult::PreviousExpressionPanics,
            DiscoverResult::ErrorInHir => DiscoverResult::ErrorInHir,
        }
    }
    pub fn transitive(self) -> Self {
        match self {
            DiscoverResult::Panic(_) => DiscoverResult::PreviousExpressionPanics,
            it => it,
        }
    }
}
impl<T> Try for DiscoverResult<T> {
    type Output = T;
    type Residual = DiscoverResult<Infallible>;

    fn from_output(output: Self::Output) -> Self {
        DiscoverResult::Value(output)
    }
    fn branch(self) -> std::ops::ControlFlow<Self::Residual, Self::Output> {
        match self {
            DiscoverResult::Value(value) => ControlFlow::Continue(value),
            DiscoverResult::Panic(panic) => ControlFlow::Break(DiscoverResult::Panic(panic)),
            DiscoverResult::DependsOnParameter => {
                ControlFlow::Break(DiscoverResult::DependsOnParameter)
            }
            DiscoverResult::PreviousExpressionPanics => {
                ControlFlow::Break(DiscoverResult::PreviousExpressionPanics)
            }
            DiscoverResult::ErrorInHir => ControlFlow::Break(DiscoverResult::ErrorInHir),
        }
    }
}
impl<T> FromResidual for DiscoverResult<T> {
    fn from_residual(residual: DiscoverResult<Infallible>) -> Self {
        match residual {
            DiscoverResult::Value(_) => unreachable!(),
            DiscoverResult::Panic(panic) => DiscoverResult::Panic(panic),
            DiscoverResult::DependsOnParameter => DiscoverResult::DependsOnParameter,
            DiscoverResult::PreviousExpressionPanics => DiscoverResult::PreviousExpressionPanics,
            DiscoverResult::ErrorInHir => DiscoverResult::ErrorInHir,
        }
    }
}

impl<T> FromResidual<Option<Infallible>> for DiscoverResult<T> {
    fn from_residual(residual: Option<Infallible>) -> Self {
        match residual {
            Some(_) => unreachable!(),
            None => DiscoverResult::DependsOnParameter,
        }
    }
}
impl<T> From<T> for DiscoverResult<T> {
    fn from(value: T) -> Self {
        DiscoverResult::Value(value)
    }
}
impl<A, V: FromIterator<A>> FromIterator<DiscoverResult<A>> for DiscoverResult<V> {
    fn from_iter<I: IntoIterator<Item = DiscoverResult<A>>>(iter: I) -> DiscoverResult<V> {
        let result = iter
            .into_iter()
            .map(|x| match x {
                DiscoverResult::Value(value) => Ok(value),
                it => Err(it),
            })
            .collect::<Result<_, _>>();
        match result {
            Ok(value) => DiscoverResult::Value(value),
            Err(DiscoverResult::Value(_)) => unreachable!(),
            Err(DiscoverResult::Panic(panic)) => DiscoverResult::Panic(panic),
            Err(DiscoverResult::DependsOnParameter) => DiscoverResult::DependsOnParameter,
            Err(DiscoverResult::PreviousExpressionPanics) => {
                DiscoverResult::PreviousExpressionPanics
            }
            Err(DiscoverResult::ErrorInHir) => DiscoverResult::ErrorInHir,
        }
    }
}
