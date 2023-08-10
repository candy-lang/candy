use crate::mir::{Body, Expression, Mir};
use core::fmt;
use std::{cmp::Ordering, ops::Add};

#[derive(Clone, Copy, Debug)]
pub struct Complexity {
    pub is_self_contained: bool,
    pub expressions: usize,
}

impl Complexity {
    const fn none() -> Self {
        Self {
            is_self_contained: true,
            expressions: 0,
        }
    }
    const fn single_expression() -> Self {
        Self {
            is_self_contained: true,
            expressions: 1,
        }
    }
}
impl Add for Complexity {
    type Output = Self;

    fn add(self, other: Self) -> Self::Output {
        Self {
            is_self_contained: self.is_self_contained && other.is_self_contained,
            expressions: self.expressions + other.expressions,
        }
    }
}
impl PartialOrd for Complexity {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        match (self.is_self_contained, other.is_self_contained) {
            (false, false) => None,
            (false, true) => Some(Ordering::Greater),
            (true, false) => Some(Ordering::Less),
            (true, true) => self.expressions.partial_cmp(&other.expressions),
        }
    }
}
impl PartialEq for Complexity {
    fn eq(&self, other: &Self) -> bool {
        self.partial_cmp(other) == Some(Ordering::Equal)
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
            self.expressions,
        )
    }
}

impl Mir {
    #[must_use]
    pub fn complexity(&self) -> Complexity {
        self.body.complexity()
    }
}
impl Body {
    #[must_use]
    pub fn complexity(&self) -> Complexity {
        self.iter()
            .fold(Complexity::none(), |complexity, (_, expression)| {
                complexity + expression.complexity()
            })
    }
}
impl Expression {
    fn complexity(&self) -> Complexity {
        match self {
            Self::Function { body, .. } => Complexity::single_expression() + body.complexity(),
            Self::UseModule { .. } => Complexity {
                is_self_contained: false,
                expressions: 1,
            },
            _ => Complexity::single_expression(),
        }
    }
}
