use super::Hint;
use crate::{
    compiler::{
        ast::{AstKind, FindAst},
        ast_to_hir::AstToHir,
        cst_to_ast::CstToAst,
        hir::Id,
    },
    database::Database,
    language_server::hints::{utils::id_to_end_of_line, HintKind},
    module::Module,
    vm::{tracer::TraceEntry, use_provider::DbUseProvider, Closure, Heap, Pointer, Status, Vm},
};
use itertools::Itertools;
use rand::{prelude::SliceRandom, thread_rng};
use std::collections::HashMap;

#[derive(Default)]
pub struct ConstantEvaluator {
    vms: HashMap<Module, Vm>,
}

impl ConstantEvaluator {
    pub fn update_module(&mut self, db: &Database, module: Module) {
        let vm = Vm::new_for_running_module_closure(
            &DbUseProvider { db },
            Closure::of_module(db, module.clone()).unwrap(),
        );
        self.vms.insert(module, vm);
    }

    pub fn remove_module(&mut self, module: Module) {
        self.vms.remove(&module).unwrap();
    }

    pub fn run(&mut self, db: &Database) -> Option<Module> {
        let num_vms = self.vms.len();
        let mut running_vms = self
            .vms
            .iter_mut()
            .filter(|(_, vm)| matches!(vm.status(), Status::Running))
            .collect_vec();
        log::trace!(
            "Constant evaluator running. {} running VMs, {} in total.",
            running_vms.len(),
            num_vms,
        );

        if let Some((module, vm)) = running_vms.choose_mut(&mut thread_rng()) {
            let use_provider = DbUseProvider { db };
            vm.run(&use_provider, 500);
            Some(module.clone())
        } else {
            None
        }
    }

    pub fn get_fuzzable_closures(&self, module: &Module) -> (&Heap, Vec<(Id, Pointer)>) {
        let vm = &self.vms[module];
        (
            &vm.heap,
            vm.fuzzable_closures
                .iter()
                .filter(|(id, _)| &id.module == module)
                .cloned()
                .collect_vec(),
        )
    }

    pub fn get_hints(&self, db: &Database, module: &Module) -> Vec<Hint> {
        let vm = &self.vms[module];

        log::debug!("Calculating hints for {module}");
        let mut hints = vec![];

        if let Status::Panicked { reason } = vm.status() {
            if let Some(hint) = panic_hint(db, module.clone(), vm, reason) {
                hints.push(hint);
            }
        };
        if module.to_possible_paths().is_some() {
            module.dump_associated_debug_file("trace", &vm.tracer.dump_call_tree());
        }

        for entry in vm.tracer.log() {
            let (id, value) = match entry {
                TraceEntry::ValueEvaluated { id, value } => {
                    if &id.module != module {
                        continue;
                    }
                    let ast_id = match db.hir_to_ast_id(id.clone()) {
                        Some(ast_id) => ast_id,
                        None => continue,
                    };
                    let ast = match db.ast(module.clone()) {
                        Some((ast, _)) => (*ast).clone(),
                        None => continue,
                    };
                    let ast = match ast.find(&ast_id) {
                        Some(ast) => ast,
                        None => continue,
                    };
                    if !matches!(ast.kind, AstKind::Assignment(_)) {
                        continue;
                    }
                    (id.clone(), value)
                }
                _ => continue,
            };

            hints.push(Hint {
                kind: HintKind::Value,
                text: value.format(&vm.heap),
                position: id_to_end_of_line(db, id).unwrap(),
            });
        }

        hints
    }
}

fn panic_hint(db: &Database, module: Module, vm: &Vm, reason: String) -> Option<Hint> {
    // We want to show the hint at the last call site still inside the current
    // module. If there is no call site in this module, then the panic results
    // from a compiler error in a previous stage which is already reported.
    let stack = vm.tracer.stack();
    if stack.len() == 1 {
        // The stack only contains a `ModuleStarted` entry. This indicates an
        // error during compilation resulting in a top-level error instruction.
        return None;
    }

    let last_call_in_this_module = stack.iter().find(|entry| {
        let id = match entry {
            TraceEntry::CallStarted { id, .. } => id,
            TraceEntry::NeedsStarted { id, .. } => id,
            _ => return false,
        };
        // Make sure the entry comes from the same file and is not generated
        // code.
        id.module == module && db.hir_to_cst_id(id.clone()).is_some()
    })?;

    let (id, call_info) = match last_call_in_this_module {
        TraceEntry::CallStarted { id, closure, args } => (
            id,
            format!(
                "{} {}",
                closure.format(&vm.heap),
                args.iter().map(|arg| arg.format(&vm.heap)).join(" ")
            ),
        ),
        TraceEntry::NeedsStarted {
            id,
            condition,
            reason,
        } => (
            id,
            format!(
                "needs {} {}",
                condition.format(&vm.heap),
                reason.format(&vm.heap)
            ),
        ),
        _ => unreachable!(),
    };

    Some(Hint {
        kind: HintKind::Panic,
        text: format!("Calling `{call_info}` panics because {reason}."),
        position: id_to_end_of_line(db, id.clone())?,
    })
}
