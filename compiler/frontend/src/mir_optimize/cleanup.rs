//! Cleanup makes the MIR more regular. Thus, it's easier to read for humans and
//! salsa should have an easier time caching optimized MIRs.
//!
//! Here's a before-and-after example:
//!
//! ```mir
//! $4 = "Banana"  |  $0 = "Apple"
//! $8 = Foo       |  $1 = "Banana"
//! $2 = "Apple"   |  $2 = Foo
//! ...            |  ...
//! ```

use rustc_hash::FxHashMap;

use super::pure::PurenessInsights;
use crate::{
    id::IdGenerator,
    mir::{Body, Expression, Id, Mir},
};
use std::env;

impl Mir {
    pub fn cleanup(&mut self, pureness: &mut PurenessInsights) {
        self.sort_constants_to_front(pureness);
        self.normalize_ids(pureness);
    }
    /// Sorts the leading constants in the body. This wouldn't be super useful
    /// when applied to an unoptimized MIR, but because we optimize it using
    /// [constant lifting], we can assume that all constants are at the
    /// beginning of the body.
    ///
    /// [constant lifting]: super::constant_lifting
    fn sort_constants_to_front(&mut self, pureness: &PurenessInsights) {
        // Extract all constants from the body so that we can sort them. We may
        // not include the last expression in the sorting because it is being
        // returned.
        let (constants, mut non_constants): (Vec<_>, Vec<_>) = self
            .body
            .expressions
            .drain(..self.body.expressions.len() - 1)
            .partition(|(_, expression)| pureness.is_definition_const(expression));
        non_constants.push(self.body.expressions.pop().unwrap());

        let mut constants = Body::new(constants);
        Self::sort_constants(&mut constants);
        self.body = constants;
        self.body.expressions.append(&mut non_constants);
    }
    /// Assumes that the given body contains only constants.
    fn sort_constants(body: &mut Body) {
        body.sort_by(|(_, a), (_, b)| {
            const fn order_score(expr: &Expression) -> u8 {
                match expr {
                    Expression::HirId(_) => 0,
                    Expression::Builtin(_) => 1,
                    Expression::Tag { value: None, .. } => 2,
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
                (
                    Expression::Tag {
                        symbol: a,
                        value: None,
                    },
                    Expression::Tag {
                        symbol: b,
                        value: None,
                    },
                ) => a.cmp(b),
                (Expression::Int(a), Expression::Int(b)) => a.cmp(b),
                (Expression::Text(a), Expression::Text(b)) => a.cmp(b),
                _ => order_score(a).cmp(&order_score(b)),
            }
        });
    }

    pub fn normalize_ids(&mut self, pureness: &mut PurenessInsights) {
        if env::var("CANDY_MIR_NORMALIZE_IDS") == Ok("false".to_string()) {
            return;
        }

        let mut generator = IdGenerator::start_at(1);
        let mapping: FxHashMap<Id, Id> = self
            .body
            .defined_ids()
            .into_iter()
            .map(|id| (id, generator.generate()))
            .collect();

        self.body.replace_ids(&mut |id| *id = mapping[&*id]);
        pureness.on_normalize_ids(&mapping);
    }
}
