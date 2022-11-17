use crate::{
    compiler::mir::{Body, Expression, Id, Mir},
    utils::IdGenerator,
};
use std::{collections::HashMap, mem};

impl Mir {
    pub fn cleanup(&mut self) {
        self.sort_leading_constants();
        self.normalize_ids();
    }

    fn sort_leading_constants(&mut self) {
        let mut still_constants = true;
        let old_body = mem::take(&mut self.body);
        for (id, expression) in old_body.into_iter() {
            if still_constants && !expression.is_pure() {
                still_constants = false;
                Self::sort_constants(&mut self.body);
            }
            self.body.push(id, expression);
        }
        if still_constants {
            Self::sort_constants(&mut self.body);
        }
    }
    fn sort_constants(body: &mut Body) {
        body.sort_by(|(_, a), (_, b)| {
            fn order_score(expr: &Expression) -> u8 {
                match expr {
                    Expression::HirId(_) => 0,
                    Expression::Builtin(_) => 1,
                    Expression::Symbol(_) => 2,
                    Expression::Int(_) => 3,
                    Expression::Text(_) => 4,
                    _ => 5,
                }
            }
            match (a, b) {
                (Expression::HirId(a), Expression::HirId(b)) => format!("{a}").cmp(&format!("{b}")),
                (Expression::Builtin(a), Expression::Builtin(b)) => {
                    format!("{a:?}").cmp(&format!("{b:?}"))
                }
                (Expression::Symbol(a), Expression::Symbol(b)) => a.cmp(b),
                (Expression::Int(a), Expression::Int(b)) => a.cmp(b),
                (Expression::Text(a), Expression::Text(b)) => a.cmp(b),
                _ => order_score(a).cmp(&order_score(b)),
            }
        });
    }

    pub fn normalize_ids(&mut self) {
        let mut generator = IdGenerator::start_at(0);
        let mapping: HashMap<Id, Id> = self
            .body
            .defined_ids()
            .into_iter()
            .map(|id| (id, generator.generate()))
            .collect();

        self.body.replace_ids(&mut |id| *id = mapping[id])
    }
}
