use bitvec::prelude::*;
use candy_vm::InstructionPointer;
use std::{
    fmt,
    ops::{Add, Range},
};

pub struct Coverage(BitVec);
pub struct RangeCoverage<'a> {
    offset: InstructionPointer,
    coverage: &'a BitSlice,
}

impl Coverage {
    pub fn none(size: usize) -> Self {
        Self(BitVec::repeat(false, size))
    }

    pub fn add(&mut self, ip: InstructionPointer) {
        self.0.set(*ip, true);
    }

    pub fn in_range(&self, range: &Range<InstructionPointer>) -> RangeCoverage {
        RangeCoverage {
            offset: range.start,
            coverage: &self.0[*range.start..*range.end],
        }
    }
    pub fn all(&self) -> RangeCoverage {
        RangeCoverage {
            offset: 0.into(),
            coverage: &self.0[..],
        }
    }
}
impl Add for &Coverage {
    type Output = Coverage;

    fn add(self, rhs: Self) -> Self::Output {
        let covered = self
            .0
            .iter()
            .map(|bit| *bit)
            .zip(rhs.0.iter().map(|bit| *bit))
            .map(|(a, b)| a | b)
            .collect();
        Coverage(covered)
    }
}

impl<'a> RangeCoverage<'a> {
    pub fn is_covered(&self, ip: InstructionPointer) -> bool {
        *self.coverage.get(*ip - *self.offset).unwrap()
    }

    pub fn improvement_on(&self, other: &RangeCoverage) -> usize {
        assert_eq!(self.offset, other.offset);
        self.coverage
            .iter()
            .zip(other.coverage.iter())
            .filter(|(a, b)| **a && !**b)
            .count()
    }

    #[allow(clippy::cast_precision_loss)]
    pub fn relative_coverage(&self) -> f64 {
        assert!(!self.coverage.is_empty());
        let num_covered = self.coverage.count_ones();
        let num_total = self.coverage.len();
        (num_covered as f64) / (num_total as f64)
    }
}

impl<'a> fmt::Debug for RangeCoverage<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[")?;
        for bit in self.coverage {
            write!(f, "{}", if *bit { '*' } else { ' ' })?;
        }
        write!(f, "]")
    }
}
