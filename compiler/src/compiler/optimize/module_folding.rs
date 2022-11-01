use crate::{
    compiler::{
        hir_to_mir::HirToMir,
        mir::{Expression, Id, Mir},
    },
    database::Database,
    module::{Module, UsePath},
};
use std::collections::HashMap;
use tracing::warn;

impl Mir {
    pub fn fold_modules(&mut self, db: &Database, import_chain: &[Module]) {
        self.body.visit(&mut |id, expression, visible, _| {
            let Expression::UseModule {
                    current_module,
                    relative_path,
                    responsible,
                } = expression else { return; };

            let use_id = id;
            let Expression::Text(path) = visible.get(*relative_path) else {
                warn!("use called with non-constant text");
                return; // TODO
            };
            let Ok(path) = UsePath::parse(&path) else {
                warn!("use called with an invalid path");
                return; // TODO
            };
            let Ok(module_to_import) = path.resolve_relative_to(current_module.clone()) else {
                warn!("use called with an invalid path");
                return; // TODO
            };
            if import_chain.contains(&module_to_import) {
                warn!("circular import");
                return; // TODO
            }

            let mir = db.mir(module_to_import.clone()).unwrap();
            let mut mir = (*mir).clone();
            let import_chain = {
                let mut chain = vec![];
                chain.extend(import_chain.iter().cloned());
                chain.push(module_to_import);
                chain
            };
            mir.optimize_obvious(db, &import_chain);

            let mapping: HashMap<Id, Id> = mir
                .body
                .all_ids()
                .into_iter()
                .map(|id| (id, self.id_generator.generate()))
                .collect();
            let mut body_to_insert = mir.body;
            body_to_insert.replace_ids(&mut |id| *id = mapping[id]);

            *expression = Expression::Multiple(body_to_insert);
        });
    }
}
