use crate::mir::{Expression, VisibleExpressions};

pub trait ExpressionOptimization {
    fn name(&self) -> &'static str;
    fn apply(&self, expression: &mut Expression, visible: &VisibleExpressions);
}
