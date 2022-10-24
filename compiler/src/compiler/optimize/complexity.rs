use crate::compiler::mir::{Expression, Id, Mir};
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
        self.complexity_of_many(&self.body)
    }

    fn complexity_of_single(&self, id: &Id) -> Complexity {
        match self.expressions.get(id).unwrap() {
            Expression::Lambda { body, .. } => {
                Complexity::single_expression() + self.complexity_of_many(body)
            }
            Expression::UseModule { .. } => Complexity {
                is_self_contained: false,
                expressions: 1,
            },
            _ => Complexity::single_expression(),
        }
    }

    fn complexity_of_many(&self, ids: &[Id]) -> Complexity {
        let mut complexity = Complexity::none();
        for id in ids {
            complexity = complexity + self.complexity_of_single(id);
        }
        complexity
    }
}
