use candy_vm::fiber::InstructionPointer;
use itertools::Itertools;
use std::{fmt, ops::Range};

pub struct Coverage {
    visited: Vec<bool>,
}
impl Coverage {
    pub fn none() -> Self {
        Self { visited: vec![] }
    }

    pub fn add(&mut self, ip: InstructionPointer) {
        if *ip > self.visited.len() {
            self.visited
                .extend([false].repeat(*ip - self.visited.len()));
        }
        self.visited[*ip] = true;
    }

    fn format_range(&self, range: Range<InstructionPointer>) -> String {
        let mut s = vec![];

        s.push('[');
        for visited in &self.visited[*range.start..*range.end] {
            s.push(if *visited { '*' } else { ' ' });
        }
        s.push(']');
        s.into_iter().join("")
    }
}
impl fmt::Debug for Coverage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            self.format_range(0.into()..self.visited.len().into())
        )
    }
}
