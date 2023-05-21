use crate::{
    mir::{Body, Expression, Id, Mir, VisibleExpressions},
    rich_ir::ToRichIr,
};
use rustc_hash::FxHashSet;
use tracing::error;

impl Mir {
    pub fn validate(&self) {
        self.body
            .validate(&mut FxHashSet::default(), im::HashSet::new());
    }
}

impl Body {
    pub fn validate(&self, defined_ids: &mut FxHashSet<Id>, mut visible: im::HashSet<Id>) {
        if self.expressions.is_empty() {
            error!("A body of a function is empty! Functions should have at least a return value.");
            error!("This is the MIR:\n{}", self.to_rich_ir());
            panic!("MIR is invalid!");
        }
        for (id, expression) in self.iter() {
            for captured in expression.captured_ids() {
                if !visible.contains(&captured) {
                    error!(
                        "MIR is invalid! {} captures {}, but that's not visible.",
                        id.to_rich_ir().text,
                        captured.to_rich_ir().text,
                    );
                    error!("This is the MIR:\n{}", self.to_rich_ir());
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
            if let Expression::Multiple(body) = expression {
                body.validate(defined_ids, visible.clone());
            }

            if defined_ids.contains(&id) {
                error!("ID {} exists twice.", id.to_rich_ir().text);
                error!("This is the MIR:\n{}", self.to_rich_ir());
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
                error!("Expression references ID {id:?}, but that ID is not visible:");
                error!("{}", self.to_rich_ir());
                panic!("Expression references ID that is not in its scope.");
            }
        }
    }
}
