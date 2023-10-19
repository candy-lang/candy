use crate::{
    mir::{Body, Expression, Id, Mir, VisibleExpressions},
    utils::RcImHashSet,
};
use rustc_hash::FxHashSet;
use tracing::error;

impl Mir {
    pub fn validate(&self) {
        self.body
            .validate(&mut FxHashSet::default(), RcImHashSet::default());
    }
}

impl Body {
    pub fn validate(&self, defined_ids: &mut FxHashSet<Id>, mut visible: RcImHashSet<Id>) {
        if self.expressions.is_empty() {
            error!("A body of a function is empty! Functions should have at least a return value.");
            error!("This is the MIR:\n{self}");
            panic!("MIR is invalid!");
        }
        for (id, expression) in self.iter() {
            for captured in expression.captured_ids() {
                if !visible.contains(&captured) {
                    error!("MIR is invalid! {id} captures {captured}, but that's not visible.");
                    error!("This is the MIR:\n{self}");
                    panic!("MIR is invalid!");
                }
            }
            if let Expression::Function {
                original_hirs: _,
                parameters,
                responsible_parameter,
                body,
            } = expression
            {
                let mut inner_visible = visible.clone();
                inner_visible.extend(parameters.iter().copied());
                inner_visible.insert(*responsible_parameter);
                body.validate(defined_ids, inner_visible);
            }

            if defined_ids.contains(&id) {
                error!("ID {id} exists twice.");
                error!("This is the MIR:\n{self}");
                panic!("MIR is invalid!");
            }
            defined_ids.insert(id);

            visible.insert(id);
        }
    }
}

impl Expression {
    pub fn validate(&self, visible: &VisibleExpressions) {
        for id in self.captured_ids() {
            if !visible.contains(id) {
                println!("Expression references ID {id:?}, but that ID is not visible:");
                println!("{self}");
                panic!("Expression references ID that is not in its scope.");
            }
        }
    }
}
