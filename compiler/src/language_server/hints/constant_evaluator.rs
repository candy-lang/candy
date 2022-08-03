use super::Hint;
use crate::{
    compiler::{
        ast::{AstKind, FindAst},
        ast_to_hir::AstToHir,
        cst_to_ast::CstToAst,
        hir::Id,
    },
    database::Database,
    input::Input,
    language_server::hints::{utils::id_to_end_of_line, HintKind},
    vm::{tracer::TraceEntry, use_provider::DbUseProvider, value::Closure, Status, Vm},
    CloneWithExtension,
};
use itertools::Itertools;
use rand::{prelude::SliceRandom, thread_rng};
use std::{collections::HashMap, fs};

#[derive(Default)]
pub struct ConstantEvaluator {
    vms: HashMap<Input, Vm>,
}

impl ConstantEvaluator {
    pub fn update_input(&mut self, db: &Database, input: Input) {
        let module_closure = Closure::of_input(db, input.clone()).unwrap();
        let mut vm = Vm::new();
        let use_provider = DbUseProvider { db };
        vm.set_up_module_closure_execution(&use_provider, module_closure);

        self.vms.insert(input, vm);
    }

    pub fn remove_input(&mut self, input: Input) {
        self.vms.remove(&input).unwrap();
    }

    pub fn run(&mut self, db: &Database) -> Option<Input> {
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

        if let Some((input, vm)) = running_vms.choose_mut(&mut thread_rng()) {
            let use_provider = DbUseProvider { db };
            vm.run(&use_provider, 500);
            Some(input.clone())
        } else {
            None
        }
    }

    pub fn get_fuzzable_closures(&self, input: &Input) -> Vec<(Id, Closure)> {
        self.vms[input]
            .fuzzable_closures
            .iter()
            .filter(|(id, _)| &id.input == input)
            .cloned()
            .collect_vec()
    }

    pub fn get_hints(&self, db: &Database, input: &Input) -> Vec<Hint> {
        let vm = &self.vms[input];

        log::trace!("Calculating hints for {input}");
        let mut hints = vec![];

        if let Status::Panicked { reason } = vm.status() {
            if let Some(hint) = panic_hint(db, input.clone(), vm, reason) {
                hints.push(hint);
            }
        };
        if let Some(path) = input.to_path() {
            let trace = vm.tracer.dump_call_tree();
            let trace_file = path.clone_with_extension("candy.trace");
            fs::write(trace_file, trace).unwrap();
        }

        for entry in vm.tracer.log() {
            let (id, value) = match entry {
                TraceEntry::ValueEvaluated { id, value } => {
                    if &id.input != input {
                        continue;
                    }
                    let ast_id = match db.hir_to_ast_id(id.clone()) {
                        Some(ast_id) => ast_id,
                        None => continue,
                    };
                    let ast = match db.ast(input.clone()) {
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
                    (id.clone(), value.clone())
                }
                _ => continue,
            };

            hints.push(Hint {
                kind: HintKind::Value,
                text: format!("{value}"),
                position: id_to_end_of_line(db, id).unwrap(),
            });
        }

        hints
    }
}

fn panic_hint(db: &Database, input: Input, vm: &Vm, reason: String) -> Option<Hint> {
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
        id.input == input && db.hir_to_cst_id(id.clone()).is_some()
    })?;

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
