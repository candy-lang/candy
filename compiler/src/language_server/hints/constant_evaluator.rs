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
    tracer::{stack_trace::StackEntry, DummyTracer, FullTracer},
    vm::{
        self,
        context::{DbUseProvider, RunLimitedNumberOfInstructions},
        Closure, Heap, Pointer, Vm,
    },
};
use itertools::Itertools;
use rand::{prelude::SliceRandom, thread_rng};
use std::collections::HashMap;
use tracing::{span, trace, Level};

#[derive(Default)]
pub struct ConstantEvaluator {
    vms: HashMap<Module, Vm>,
}

impl ConstantEvaluator {
    pub fn update_module(&mut self, db: &Database, module: Module) {
        let mut vm = Vm::new();
        vm.set_up_for_running_module_closure(Closure::of_module(db, module.clone()).unwrap());
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
            .filter(|(_, vm)| matches!(vm.status(), vm::Status::CanRun))
            .collect_vec();
        trace!(
            "Constant evaluator running. {} running VMs, {} in total.",
            running_vms.len(),
            num_vms,
        );

        if let Some((module, vm)) = running_vms.choose_mut(&mut thread_rng()) {
            vm.run(
                &mut DbUseProvider { db },
                &mut RunLimitedNumberOfInstructions::new(500),
                &mut DummyTracer,
            );
            Some(module.clone())
        } else {
            None
        }
    }

    pub fn get_fuzzable_closures(&self, module: &Module) -> (Heap, Vec<(Id, Pointer)>) {
        let vm = &self.vms[module];
        let result = vm.clone().tear_down();
        // (heap, fuzzable_closures)
        todo!()
    }

    pub fn get_hints(&self, db: &Database, module: &Module) -> Vec<Hint> {
        let span = span!(Level::DEBUG, "Calculating hints for {module}");
        let _enter = span.enter();

        let vm = &self.vms[module];
        let mut hints = vec![];

        if let vm::Status::Panicked { reason } = vm.status() {
            if let Some(hint) = panic_hint(db, module.clone(), vm, reason) {
                hints.push(hint);
            }
        };

        // let TearDownResult { heap, tracer, .. } = vm.clone().tear_down();
        let heap: Heap = todo!();
        let tracer: FullTracer = todo!();
        for entry in &tracer.events {
            // if let EventData::ValueEvaluated { id, value } = &entry.data {
            //     if &id.module != module {
            //         continue;
            //     }
            //     let ast_id = match db.hir_to_ast_id(id.clone()) {
            //         Some(ast_id) => ast_id,
            //         None => continue,
            //     };
            //     let ast = match db.ast(module.clone()) {
            //         Some((ast, _)) => (*ast).clone(),
            //         None => continue,
            //     };
            //     let ast = match ast.find(&ast_id) {
            //         Some(ast) => ast,
            //         None => continue,
            //     };
            //     if !matches!(ast.kind, AstKind::Assignment(_)) {
            //         continue;
            //     }

            //     hints.push(Hint {
            //         kind: HintKind::Value,
            //         text: value.format(&heap),
            //         position: id_to_end_of_line(db, id.clone()).unwrap(),
            //     });
            // }
        }

        hints
    }
}

fn panic_hint(db: &Database, module: Module, vm: &Vm, reason: String) -> Option<Hint> {
    // We want to show the hint at the last call site still inside the current
    // module. If there is no call site in this module, then the panic results
    // from a compiler error in a previous stage which is already reported.
    let heap: Heap = todo!(); // vm.clone().tear_down();
    let tracer: FullTracer = todo!();
    // let stack = tracer.stack_trace();
    let stack: Vec<StackEntry> = todo!();
    if stack.len() == 1 {
        // The stack only contains an `InModule` entry. This indicates an error
        // during compilation resulting in a top-level error instruction.
        return None;
    }

    let last_call_in_this_module = stack.iter().find(|entry| {
        let id = match entry {
            StackEntry::Call { id, .. } => id,
            StackEntry::Needs { id, .. } => id,
            _ => return false,
        };
        // Make sure the entry comes from the same file and is not generated
        // code.
        id.module == module && db.hir_to_cst_id(id.clone()).is_some()
    })?;

    let (id, call_info) = match last_call_in_this_module {
        StackEntry::Call { id, closure, args } => (
            id,
            format!(
                "{} {}",
                closure.format(&heap),
                args.iter().map(|arg| arg.format(&heap)).join(" ")
            ),
        ),
        StackEntry::Needs {
            id,
            condition,
            reason,
        } => (
            id,
            format!("needs {} {}", condition.format(&heap), reason.format(&heap)),
        ),
        _ => unreachable!(),
    };

    Some(Hint {
        kind: HintKind::Panic,
        text: format!("Calling `{call_info}` panics because {reason}."),
        position: id_to_end_of_line(db, id.clone())?,
    })
}
