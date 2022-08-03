use super::Hint;
use crate::{
    compiler::{
        ast::{Assignment, AstKind, FindAst},
        ast_to_hir::AstToHir,
        cst_to_ast::CstToAst,
        hir::Id,
    },
    database::Database,
    language_server::hints::{utils::id_to_end_of_line, HintKind},
    module::Module,
    vm::{tracer::TraceEntry, use_provider::DbUseProvider, value::Closure, Status, Vm},
    CloneWithExtension,
};
use itertools::Itertools;
use rand::{prelude::SliceRandom, thread_rng};
use std::{collections::HashMap, fs};

#[derive(Default)]
pub struct ConstantEvaluator {
    vms: HashMap<Module, Vm>,
}

impl ConstantEvaluator {
    pub fn update_module(&mut self, db: &Database, module: Module) {
        let module_closure = Closure::of_module(&db, module.clone()).unwrap();
        let mut vm = Vm::new();
        let use_provider = DbUseProvider { db: &db };
        vm.set_up_module_closure_execution(&use_provider, module_closure);

        self.vms.insert(module.clone(), vm);
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

        running_vms.shuffle(&mut thread_rng());
        if let Some((module, vm)) = running_vms.pop() {
            let use_provider = DbUseProvider { db };
            vm.run(&use_provider, 500);
            Some(module.clone())
        } else {
            None
        }
    }

    pub fn get_fuzzable_closures(&self, module: &Module) -> Vec<(Id, Closure)> {
        self.vms[module]
            .fuzzable_closures
            .iter()
            .filter(|(id, _)| &id.module == module)
            .map(|it| it.clone())
            .collect_vec()
    }

    pub fn get_hints(&self, db: &Database, module: &Module) -> Vec<Hint> {
        let vm = &self.vms[module];

        log::debug!("Calculating hints for {module}");
        let mut hints = vec![];

        if let Status::Panicked { reason } = vm.status() {
            match panic_hint(&db, module.clone(), &vm, reason) {
                Some(hint) => {
                    hints.push(hint);
                }
                None => log::error!("Module panicked, but we are not displaying an error."),
            }
        };
        if module.to_possible_paths().is_some() {
            let trace = vm.tracer.dump_call_tree();
            let trace_file = module.associated_debug_file("trace");
            fs::write(trace_file.clone(), trace).unwrap();
        }

        for entry in vm.tracer.log() {
            let (id, name, value) = match entry {
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
                    let name = match ast.find(&ast_id) {
                        None => continue,
                        Some(ast) => match &ast.kind {
                            AstKind::Assignment(Assignment { name, .. }) => name.value.clone(),
                            _ => continue,
                        },
                    };
                    (id.clone(), name, value.clone())
                }
                _ => continue,
            };

            hints.push(Hint {
                kind: HintKind::Value,
                text: format!("{name} = {value}"),
                position: id_to_end_of_line(&db, id).unwrap(),
            });
        }

        hints
    }
}

fn panic_hint(db: &Database, module: Module, vm: &Vm, reason: String) -> Option<Hint> {
    // We want to show the hint at the last call site still inside the current
    // module. If there is no call site in this module, then the panic results
    // from a compiler error in a previous stage which is already reported.
    let last_call_in_this_module = vm
        .tracer
        .stack()
        .iter()
        .filter(|entry| {
            let id = match entry {
                TraceEntry::CallStarted { id, .. } => id,
                TraceEntry::NeedsStarted { id, .. } => id,
                _ => return false,
            };
            // Make sure the entry comes from the same file and is not generated
            // code.
            id.module == module && db.hir_to_cst_id(id.clone()).is_some()
        })
        .next()?;

    let (id, call_info) = match last_call_in_this_module {
        TraceEntry::CallStarted { id, closure, args } => (
            id,
            format!(
                "{closure} {}",
                args.iter().map(|arg| format!("{arg}")).join(" ")
            ),
        ),
        TraceEntry::NeedsStarted {
            id,
            condition,
            reason,
        } => (id, format!("needs {condition} {reason}")),
        _ => unreachable!(),
    };

    Some(Hint {
        kind: HintKind::Panic,
        text: format!("Calling `{call_info}` panics because {reason}."),
        position: id_to_end_of_line(db, id.clone())?,
    })
}
