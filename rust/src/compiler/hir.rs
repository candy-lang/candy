use std::fmt;

use itertools::Itertools;

pub type Id = usize;

#[derive(Clone, PartialEq, Eq)]
pub enum Expression {
    Int(u64),
    Text(String),
    Symbol(String),
    Lambda(Lambda),
    Call { function: Id, arguments: Vec<Id> },
}
impl Expression {
    pub fn nothing() -> Self {
        Expression::Symbol("Nothing".to_owned())
    }
    pub fn error() -> Self {
        Expression::Symbol("Error".to_owned())
    }
}

#[derive(Clone, PartialEq, Eq, Hash)]
pub struct Lambda {
    pub first_id: Id,
    pub parameter_count: usize,
    pub out: Id,
    expressions: Vec<Expression>,
}

impl Lambda {
    pub fn new(first_id: Id, parameter_count: usize) -> Self {
        Self {
            first_id,
            parameter_count,
            out: first_id,
            expressions: vec![],
        }
    }
    pub fn next_id(&self) -> Id {
        self.first_id + self.parameter_count + self.expressions.len()
    }
    pub fn push(&mut self, expression: Expression) -> Id {
        let id = self.next_id();
        self.expressions.push(expression);
        id
    }
    pub fn get(&self, id: Id) -> Option<&Expression> {
        // TODO: use a different type when supporting more expressions than 2^127
        let index = id as i128 - self.first_id as i128 - self.parameter_count as i128;
        if index < 0 {
            None
        } else {
            self.expressions.get(index as usize)
        }
    }
    pub fn get_mut(&mut self, id: Id) -> Option<&mut Expression> {
        // TODO: use a different type when supporting more expressions than 2^127
        let index = id as i128 - self.first_id as i128 - self.parameter_count as i128;
        if index < 0 {
            None
        } else {
            self.expressions.get_mut(index as usize)
        }
    }
    pub fn iter(&self) -> impl DoubleEndedIterator<Item = (Id, Expression)> {
        Iter {
            shift: self.first_id + self.parameter_count,
            inner: self.expressions.clone().into_iter().enumerate(),
        }
    }
    pub fn iter_mut(&mut self) -> impl DoubleEndedIterator<Item = (Id, &mut Expression)> {
        Iter {
            shift: self.first_id + self.parameter_count,
            inner: self.expressions.iter_mut().enumerate(),
        }
    }
}
pub struct Iter<T, I: Iterator<Item = (usize, T)>> {
    shift: Id,
    inner: I,
}
impl<T, I: Iterator<Item = (usize, T)>> Iterator for Iter<T, I> {
    type Item = (Id, T);

    fn next(&mut self) -> Option<Self::Item> {
        self.inner
            .next()
            .map(|(id, expression)| (id + self.shift, expression))
    }
}
impl<T, I: DoubleEndedIterator<Item = (usize, T)>> DoubleEndedIterator for Iter<T, I> {
    fn next_back(&mut self) -> Option<Self::Item> {
        self.inner
            .next_back()
            .map(|(id, expression)| (id + self.shift, expression))
    }
}

impl std::hash::Hash for Expression {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        core::mem::discriminant(self).hash(state);
        match self {
            Expression::Int(int) => int.hash(state),
            Expression::Text(text) => text.hash(state),
            Expression::Symbol(symbol) => symbol.hash(state),
            Expression::Lambda(Lambda {
                first_id,
                parameter_count,
                out,
                expressions,
            }) => {
                first_id.hash(state);
                parameter_count.hash(state);
                out.hash(state);
                expressions.hash(state);
            }
            Expression::Call {
                function,
                arguments,
            } => {
                function.hash(state);
                arguments.hash(state);
            }
        }
    }
}

impl fmt::Display for Expression {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Expression::Int(int) => write!(f, "int {}", int),
            Expression::Text(text) => write!(f, "text {:?}", text),
            Expression::Symbol(symbol) => write!(f, "symbol :{}", symbol),
            Expression::Lambda(lambda) => {
                write!(
                    f,
                    "lambda [\n{}\n]",
                    lambda
                        .to_string()
                        .lines()
                        .map(|line| format!("  {}", line))
                        .join("\n"),
                )
            }
            Expression::Call {
                function,
                arguments,
            } => {
                if arguments.is_empty() {
                    write!(f, "call {}", function)
                } else {
                    write!(f, "call {} with {}", function, arguments.iter().join(" "))
                }
            }
        }
    }
}
impl fmt::Display for Lambda {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} parameters\n", self.parameter_count)?;
        for (id, action) in self.iter() {
            write!(f, "{} = {}\n", id, action)?;
        }
        write!(f, "out: {}\n", self.out)?;
        Ok(())
    }
}
