use bitvec::prelude::*;
use candy_vm::fiber::InstructionPointer;
use itertools::Itertools;
use std::{
    fmt,
    ops::{Add, Range},
};

pub struct Coverage {
    covered: BitVec,
}
impl Coverage {
    pub fn none() -> Self {
        Self { covered: bitvec![] }
    }

    pub fn add(&mut self, ip: InstructionPointer) {
        if *ip >= self.covered.len() {
            self.covered
                .extend([false].repeat(*ip - self.covered.len() + 1));
        }
        self.covered.set(*ip, true);
    }

    pub fn is_covered(&self, ip: InstructionPointer) -> bool {
        *self.covered.get(*ip).unwrap()
    }

    pub fn improvement_on(&self, other: &Coverage) -> usize {
        self.covered
            .iter()
            .zip(other.covered.iter())
            .filter(|(a, b)| **a && !**b)
            .count()
    }

    pub fn relative_coverage_of_range(&self, range: Range<InstructionPointer>) -> f64 {
        assert!(!range.is_empty());
        let len = *range.end - *range.start;
        range.filter(|ip| self.is_covered(*ip)).count() as f64 / len as f64
    }
}
impl Add for &Coverage {
    type Output = Coverage;

    fn add(self, rhs: Self) -> Self::Output {
        let covered = self
            .covered
            .iter()
            .map(|bit| *bit)
            .zip_longest(rhs.covered.iter().map(|bit| *bit))
            .map(|it| {
                let (a, b) = it.or_default();
                a | b
            })
            .collect();
        Coverage { covered }
    }
}

impl Coverage {
    pub fn format_range(&self, range: Range<InstructionPointer>) -> String {
        let mut s = "[".to_owned();

        for ip in *range.start..*range.end {
            s.push(if self.is_covered(ip.into()) { '*' } else { ' ' });
        }
        s.push(']');
        s
    }
}
impl fmt::Debug for Coverage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            self.format_range(0.into()..self.covered.len().into()),
        )
    }
}
