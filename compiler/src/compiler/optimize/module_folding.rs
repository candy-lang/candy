use crate::{
    compiler::{
        ast_to_hir::AstToHir,
        hir::{Body, Expression, Id},
    },
    database::Database,
    module::{Module, UsePath},
};
use im::HashMap;
use tracing::warn;

impl Body {
    pub fn fold_modules(&mut self, db: &Database, import_chain: &[Module]) {
        self.fold_inner_modules(db, import_chain, HashMap::new());
    }

    fn fold_inner_modules(
        &mut self,
        db: &Database,
        import_chain: &[Module],
        mut previous_expressions: HashMap<Id, Expression>,
    ) {
        for id in self.ids.clone() {
            let mut expression = self.expressions.get(&id).unwrap().clone();
            match &mut expression {
                Expression::Lambda(lambda) => {
                    lambda
                        .body
                        .fold_inner_modules(db, import_chain, previous_expressions.clone());
                }
                Expression::UseModule {
                    current_module,
                    relative_path,
                } => {
                    let Some(Expression::Text(path)) = previous_expressions.get(relative_path) else {
                        warn!("use called with non-constant text");
                        return;
                    };

                    let Ok(path) = UsePath::parse(&path) else {
                        warn!("use called with an invalid path");
                        return;
                    };

                    let Ok(module_to_import) = path.resolve_relative_to(current_module.clone()) else {
                        warn!("use called with an invalid path");
                        return;
                    };

                    let (hir, _) = db.hir(module_to_import.clone()).unwrap();
                    let mut hir = (*hir).clone();
                    let import_chain = {
                        let mut chain = vec![];
                        chain.extend(import_chain.iter().cloned());
                        chain.push(module_to_import);
                        chain
                    };
                    hir.optimize_obvious(db, &import_chain);

                    hir.replace_ids(&mut |id_in_module| {
                        id_in_module.module = id.module.clone();
                        for key in &id.keys {
                            id_in_module.keys.insert(0, key.clone());
                        }
                    });
                    let index = self.ids.iter().position(|it| *it == id).unwrap();
                    for (i, id) in hir.ids.iter().enumerate() {
                        self.ids.insert(index + i, id.clone());
                        self.expressions
                            .insert(id.clone(), hir.expressions.get(id).unwrap().clone());
                    }
                    *self.expressions.get_mut(&id).unwrap() =
                        Expression::Reference(hir.return_value());
                }
                _ => {}
            }
            previous_expressions.insert(id.clone(), expression);
        }
    }
}
