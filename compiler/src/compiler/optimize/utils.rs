use crate::compiler::hir::{Body, Expression, Id};

impl Body {
    pub fn is_constant(&self, id: &Id) -> bool {
        match self.expressions.get(id).unwrap() {
            Expression::Int(_)
            | Expression::Text(_)
            | Expression::Symbol(_)
            | Expression::Builtin(_) => true,
            Expression::Reference(id) => self.is_constant(id),
            Expression::Struct(fields) => fields
                .iter()
                .all(|(key, value)| self.is_constant(key) && self.is_constant(value)),
            Expression::Lambda(lambda) => lambda
                .captured_ids(id)
                .iter()
                .all(|id| self.is_constant(id)),
            Expression::Call { .. }
            | Expression::UseModule { .. }
            | Expression::Needs { .. }
            | Expression::Error { .. } => false,
        }
    }

    pub fn equals(&self, a: &Id, b: &Id) -> Option<bool> {
        if a == b {
            return Some(true);
        }
        let a_expr = self.expressions.get(a).unwrap();
        let b_expr = self.expressions.get(a).unwrap();
        if let Expression::Reference(reference) = a_expr {
            return self.equals(reference, b);
        }
        if let Expression::Reference(reference) = b_expr {
            return self.equals(a, reference);
        }

        match (
            self.expressions.get(a).unwrap(),
            self.expressions.get(b).unwrap(),
        ) {
            (Expression::Int(a), Expression::Int(b)) => Some(a == b),
            (Expression::Text(a), Expression::Text(b)) => Some(a == b),
            (Expression::Symbol(a), Expression::Symbol(b)) => Some(a == b),
            (Expression::Struct(a), Expression::Struct(b)) => {
                // TODO
                todo!()
            }
            // Also consider lambdas equal where only some IDs are named
            // differently.
            (Expression::Lambda(a), Expression::Lambda(b)) => Some(a == b),
            (Expression::Builtin(a), Expression::Builtin(b)) => Some(a == b),
            (Expression::Call { .. }, _)
            | (Expression::UseModule { .. }, _)
            | (Expression::Needs { .. }, _)
            | (Expression::Error { .. }, _)
            | (_, Expression::Call { .. })
            | (_, Expression::UseModule { .. })
            | (_, Expression::Needs { .. })
            | (_, Expression::Error { .. }) => None,
            (_, _) => Some(false),
        }
    }

    // Replaces a range of the statements with some other statements. Updates
    // all later references into the range using the `reference_replacements`.
    //
    // # Example
    //
    // Given this code:
    //
    // ```txt
    // HirId(0) = symbol :
    // HirId(1) = int 1
    // HirId(2) = int 2
    // HirId(3) = builtinAdd 1 2
    // HirId(4) = builtinPrint 3
    // HirId(5) = symbol :foo
    // HirId(6) = builtinPrint 5
    // ```
    //
    // Calling `replace_range(HirId(1), 3, [number 3], {3, 1})` turns it into this:
    //
    // ```txt
    // HirId(0) = symbol :
    // HirId(1) = number 3
    // HirId(2) = primitive_print 1
    // HirId(3) = symbol :foo
    // HirId(4) = primitive_print 3
    // ```
    // pub fn replace_range(
    //     &mut self,
    //     start: Id,
    //     length: usize,
    //     replacement: Vec<Statement>,
    //     reference_replacements: HashMap<Id, Id>,
    // ) {
    //     debug!(
    //         "Optimizer: Replacing {} len {} with {:?}. Replacements: {:?}",
    //         start, length, replacement, reference_replacements
    //     );
    //     let mut statements = vec![];

    //     let start = start;
    //     let end = start + length as u32;
    //     let start_index = start as usize - self.in_ as usize - 1;
    //     let end_index = end as usize - self.in_ as usize - 1;

    //     // The statements before the replaced part stay the same.
    //     for statement in &self.statements[0..start_index] {
    //         statements.push(statement.clone());
    //     }

    //     // The replaced part gets ignored, we use the replacement instead.
    //     for statement in &replacement {
    //         statements.push(statement.clone());
    //     }

    //     // The statements after that need to get their IDs replaced. IDs that
    //     // reference statements before the replaced ones stay the same. IDs that
    //     // reference into the replaced range get replaced according to the
    //     // `reference_replacements`. IDs that reference statements after the
    //     // replaced range get shifted – the replacement may have a different
    //     // length than the replaced statements.
    //     let shift = replacement.len() as isize - length as isize;
    //     let transform = |id| {
    //         if id < start {
    //             id
    //         } else if id >= end {
    //             (id as isize + shift) as u32
    //         } else {
    //             *reference_replacements.get(&id).expect(&format!(
    //                 "Reference to ID {} in replaced range with no replacement.",
    //                 id
    //             ))
    //         }
    //     };
    //     for statement in &mut self.statements[end_index as usize..] {
    //         let mut statement = statement.clone();
    //         statement.replace_ids(&transform);
    //         statements.push(statement);
    //     }

    //     self.statements = statements;
    //     self.in_ = transform(self.in_);
    //     self.out = transform(self.out);

    //     debug!("Now the HIR is this: {}", self);
    // }

    // pub fn replace_ids<F: Fn(Id) -> Id>(&mut self, transform: &F) {
    //     self.in_ = transform(self.in_);
    //     self.out = transform(self.out);
    //     for (_, statement) in self.iter_mut() {
    //         statement.replace_ids(transform);
    //     }
    // }
}
