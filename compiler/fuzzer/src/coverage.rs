use bitvec::prelude::*;
use candy_vm::fiber::InstructionPointer;
use itertools::Itertools;
use std::{
    fmt,
    ops::{Add, Range},
};

pub struct Coverage(BitVec);

impl Coverage {
    pub fn none() -> Self {
        Self(bitvec![])
    }

    pub fn add(&mut self, ip: InstructionPointer) {
        if *ip >= self.0.len() {
            self.0.extend([false].repeat(*ip - self.0.len() + 1));
        }
        self.0.set(*ip, true);
    }

    pub fn is_covered(&self, ip: InstructionPointer) -> bool {
        *self.0.get(*ip).unwrap()
    }

    pub fn improvement_on(&self, other: &Coverage) -> usize {
        self.0
            .iter()
            .zip(other.0.iter())
            .filter(|(a, b)| **a && !**b)
            .count()
    }

    fn bitslice_in_range(&self, range: Range<InstructionPointer>) -> &BitSlice {
        &self.0[*range.start..*range.end]
    }
    pub fn in_range(&self, range: Range<InstructionPointer>) -> Coverage {
        Self(self.bitslice_in_range(range).to_bitvec())
    }

    pub fn relative_coverage(&self) -> f64 {
        assert!(!self.0.is_empty());
        let num_covered = self.0.count_ones();
        let num_total = self.0.len();
        (num_covered as f64) / (num_total as f64)
    }
}
impl Add for &Coverage {
    type Output = Coverage;

    fn add(self, rhs: Self) -> Self::Output {
        let covered = self
            .0
            .iter()
            .map(|bit| *bit)
            .zip_longest(rhs.0.iter().map(|bit| *bit))
            .map(|it| {
                let (a, b) = it.or_default();
                a | b
            })
            .collect();
        Coverage(covered)
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
        write!(f, "{}", self.format_range(0.into()..self.0.len().into()),)
    }
}
