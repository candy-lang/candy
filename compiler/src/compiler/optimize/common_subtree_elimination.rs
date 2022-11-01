use crate::{
    builtin_functions::BuiltinFunction,
    compiler::mir::{Expression, Id, Mir},
};
use std::collections::HashMap;
use tracing::{debug, warn};

// impl Mir {
//     pub fn eliminate_common_subtrees(&mut self) {
//         let mut pure_expressions: HashMap<Expression, Id> = HashMap::new();
//         let mut mapping: HashMap<Id, Id> = HashMap::new();

//         for id in self.body.iter().copied() {
//             id.replace_id_references(&mut self.expressions, &mut |id| {
//                 if let Some(replacement) = mapping.get(id) {
//                     *id = *replacement;
//                 }
//             });
//             let expression = self.expressions.get(&id).unwrap();
//             if !expression.is_pure() {
//                 continue;
//             }
//             if let Some(id_with_same_expression) = pure_expressions.get(expression) {
//                 self.expressions.insert(id, Expression::Reference(id));
//                 mapping.insert(id, *id_with_same_expression);
//             } else {
//                 pure_expressions.insert(expression.clone(), id);
//             }
//         }
//     }
// }
