use super::Hint;
use crate::{
    compiler::{
        ast::{AstKind, FindAst},
        ast_to_hir::AstToHir,
        cst_to_ast::CstToAst,
        hir::Id,
        hir_to_mir::TracingConfig,
    },
    database::Database,
    language_server::hints::{utils::id_to_end_of_line, HintKind},
    module::Module,
    vm::{
        self,
        context::{DbUseProvider, RunLimitedNumberOfInstructions},
        tracer::{
            full::{FullTracer, StoredFiberEvent, StoredVmEvent, TimedEvent},
            stack_trace::Call,
        },
        Closure, FiberId, Heap, Pointer, Vm,
    },
};
use itertools::Itertools;
use rand::{prelude::SliceRandom, thread_rng};
use std::collections::HashMap;
use tracing::{span, Level};

#[derive(Default)]
pub struct ConstantEvaluator {
    evaluators: HashMap<Module, Evaluator>,
}
struct Evaluator {
    tracer: FullTracer,
    vm: Vm,
}

impl ConstantEvaluator {
    pub fn update_module(&mut self, db: &Database, module: Module) {
        let config = TracingConfig {
            register_fuzzables: true,
            trace_calls: false,
            trace_evaluated_expressions: true,
        };
        let tracer = FullTracer::default();
        let mut vm = Vm::new();
        vm.set_up_for_running_module_closure(
            module.clone(),
            Closure::of_module(db, module.clone(), config).unwrap(),
        );
        self.evaluators.insert(module, Evaluator { tracer, vm });
    }

    pub fn remove_module(&mut self, module: Module) {
        self.evaluators.remove(&module).unwrap();
    }

    pub fn run(&mut self, db: &Database) -> Option<Module> {
        let mut running_evaluators = self
            .evaluators
            .iter_mut()
            .filter(|(_, evaluator)| matches!(evaluator.vm.status(), vm::Status::CanRun))
            .collect_vec();
        let Some((module, evaluator)) = running_evaluators.choose_mut(&mut thread_rng()) else {
            return None;
        };

        evaluator.vm.run(
            &DbUseProvider {
                db,
                config: TracingConfig::none(),
            },
            &mut RunLimitedNumberOfInstructions::new(500),
            &mut evaluator.tracer,
        );
        Some(module.clone())
    }

    pub fn get_fuzzable_closures(&self, module: &Module) -> (Heap, Vec<(Id, Pointer)>) {
        let evaluator = &self.evaluators[module];
        let fuzzable_closures = evaluator
            .tracer
            .events
            .iter()
            .filter_map(|event| match &event.event {
                StoredVmEvent::InFiber {
                    event:
                        StoredFiberEvent::FoundFuzzableClosure {
                            definition: id,
                            closure,
                        },
                    ..
                } => Some((evaluator.tracer.heap.get_hir_id(*id), *closure)),
                _ => None,
            })
            .collect();
        (evaluator.tracer.heap.clone(), fuzzable_closures)
    }

    pub fn get_hints(&self, db: &Database, module: &Module) -> Vec<Hint> {
        let span = span!(Level::DEBUG, "Calculating hints", %module);
        let _enter = span.enter();

        let evaluator = &self.evaluators[module];
        let mut hints = vec![];

        // TODO: Think about how to highlight the responsible piece of code.
        if let vm::Status::Panicked { reason, .. } = evaluator.vm.status() {
            if let Some(hint) = panic_hint(db, module.clone(), evaluator, reason) {
                hints.push(hint);
            }
        };

        for TimedEvent { event, .. } in &evaluator.tracer.events {
            let StoredVmEvent::InFiber { event, .. } = event else { continue; };
            let StoredFiberEvent::ValueEvaluated { expression, value } = event else { continue; };
            let id = evaluator.tracer.heap.get_hir_id(*expression);

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

            hints.push(Hint {
                kind: HintKind::Value,
                text: value.format(&evaluator.tracer.heap),
                position: id_to_end_of_line(db, id.clone()).unwrap(),
            });
        }

        hints
    }
}

fn panic_hint(
    db: &Database,
    module: Module,
    evaluator: &Evaluator,
    reason: String,
) -> Option<Hint> {
    // We want to show the hint at the last call site still inside the current
    // module. If there is no call site in this module, then the panic results
    // from a compiler error in a previous stage which is already reported.
    let stack_traces = evaluator.tracer.stack_traces();
    let stack = stack_traces.get(&FiberId::root()).unwrap();
    if stack.len() == 1 {
        // The stack only contains an `InModule` entry. This indicates an error
        // during compilation resulting in a top-level error instruction.
        return None;
    }

    let last_call_in_this_module = stack.iter().find(|call| {
        let call_site = evaluator.tracer.heap.get_hir_id(call.call_site);
        // Make sure the entry comes from the same file and is not generated
        // code.
        call_site.module == module && db.hir_to_cst_id(call_site).is_some()
    })?;

    let Call {
        call_site,
        callee,
        arguments: args,
        ..
    } = last_call_in_this_module;
    let call_site = evaluator.tracer.heap.get_hir_id(*call_site);
    let call_info = format!(
        "{} {}",
        callee.format(&evaluator.tracer.heap),
        args.iter()
            .map(|arg| arg.format(&evaluator.tracer.heap))
            .join(" "),
    );

    Some(Hint {
        kind: HintKind::Panic,
        text: format!("Calling `{call_info}` panics: {reason}"),
        position: id_to_end_of_line(db, call_site)?,
    })
}
