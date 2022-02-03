use std::{
    fmt::{self, Display, Formatter},
    ops::Add,
};

use im::HashMap;
use itertools::Itertools;

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct Id(pub Vec<usize>);
impl Display for Id {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "HirId({:?})", self.0)
    }
}

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum Expression {
    Int(u64),
    Text(String),
    Symbol(String),
    Lambda(Lambda),
    Body(Body),
    Call { function: Id, arguments: Vec<Id> },
    Error,
}
impl Expression {
    pub fn nothing() -> Self {
        Expression::Symbol("Nothing".to_owned())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Lambda {
    pub first_id: Id,
    pub parameters: Vec<String>,
    pub body: Body,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Body {
    pub expressions: HashMap<Id, Expression>,
    pub identifiers: HashMap<Id, String>,
    pub out: Option<Id>,
}

impl Add<usize> for Id {
    type Output = Self;

    fn add(self, rhs: usize) -> Self::Output {
        assert!(!self.0.is_empty());
    }
}

impl Lambda {
    pub fn new(first_id: Id, parameters: Vec<String>) -> Self {
        Self {
            first_id,
            parameters,
            body: Body::new(),
        }
    }
}
impl Body {
    pub fn new() -> Self {
        Self {
            expressions: HashMap::new(),
            identifiers: HashMap::new(),
            out: None,
        }
    }
    pub fn push(&mut self, id: Id, expression: Expression, identifier: Option<String>) {
        self.expressions.insert(id, expression);
        if let Some(identifier) = identifier {
            self.identifiers.insert(id, identifier);
        }
    }
    pub fn get(&self, id: &Id) -> Option<&Expression> {
        self.expressions.get(id)
    }
    pub fn get_mut(&mut self, id: &Id) -> Option<&mut Expression> {
        self.expressions.get_mut(id)
    }
}

impl fmt::Display for Expression {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Expression::Int(int) => write!(f, "int {}", int),
            Expression::Text(text) => write!(f, "text {:?}", text),
            Expression::Symbol(symbol) => write!(f, "symbol {}", symbol),
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
            Expression::Body(body) => {
                write!(
                    f,
                    "body [\n{}\n]",
                    body.to_string()
                        .lines()
                        .map(|line| format!("  {}", line))
                        .join("\n"),
                )
            }
            Expression::Call {
                function,
                arguments,
            } => {
                assert!(arguments.len() > 0, "A call needs to have arguments.");
                write!(f, "call {} with {}", function, arguments.iter().join(" "))
            }
            Expression::Error => write!(f, "<error>"),
        }
    }
}
impl fmt::Display for Lambda {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} parameters\n", self.parameters.len())?;
        write!(f, "{}", self.body)?;
        Ok(())
    }
}
impl fmt::Display for Body {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (id, expression) in self.expressions.iter() {
            write!(f, "{} = {}\n", id, expression)?;
        }
        write!(f, "out: {:?}\n", self.out)?;
        Ok(())
    }
}

impl Expression {
    fn find(&self, id: &Id) -> &Self {
        match self {
            Expression::Int { .. } => panic!("Couldn't find ID {}.", id),
            Expression::Text { .. } => panic!("Couldn't find ID {}.", id),
            Expression::Symbol { .. } => panic!("Couldn't find ID {}.", id),
            Expression::Lambda(Lambda { body, .. }) => body.find(id),
            Expression::Body(body) => body.find(id),
            Expression::Call {
                function,
                arguments,
            } => panic!("Couldn't find ID {}.", id),
            Expression::Error { .. } => panic!("Couldn't find ID {}.", id),
        }
    }
}
impl Body {
    pub fn find(&self, id: &Id) -> &Expression {
        if let Some(expression) = self.expressions.get(id) {
            expression
        } else {
            self.expressions
                .iter()
                .filter(|(key, _)| key.0 <= id.0)
                .max_by_key(|(key, _)| key.0)
                .unwrap()
                .1
                .find(id)
        }
    }
}
