use crate::compiler::mir::{Body, Expression, Mir};
use core::fmt;
use std::ops::Add;

pub struct Complexity {
    is_self_contained: bool,
    expressions: usize,
}

impl Complexity {
    fn none() -> Self {
        Self {
            is_self_contained: true,
            expressions: 0,
        }
    }
    fn single_expression() -> Self {
        Self {
            is_self_contained: true,
            expressions: 1,
        }
    }
}
impl Add for Complexity {
    type Output = Complexity;

    fn add(self, other: Self) -> Self::Output {
        Complexity {
            is_self_contained: self.is_self_contained && other.is_self_contained,
            expressions: self.expressions + other.expressions,
        }
    }
}
impl fmt::Display for Complexity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}, {} expressions",
            if self.is_self_contained {
                "self-contained"
            } else {
                "still contains `use`"
            },
            self.expressions
        )
    }
}

impl Mir {
    pub fn complexity(&self) -> Complexity {
        self.body.complexity()
    }
}
impl Body {
    fn complexity(&self) -> Complexity {
        let mut complexity = Complexity::none();
        for (_, expression) in self.iter() {
            complexity = complexity + expression.complexity();
        }
        complexity
    }
}
impl Expression {
    fn complexity(&self) -> Complexity {
        match self {
            Expression::Lambda { body, .. } => Complexity::single_expression() + body.complexity(),
            Expression::UseModule { .. } => Complexity {
                is_self_contained: false,
                expressions: 1,
            },
            _ => Complexity::single_expression(),
        }
    }
}
