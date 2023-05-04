use super::{utils::IdToEndOfLine, Hint, HintKind};
use candy_frontend::{
    ast::{Assignment, AssignmentBody, AstDb, AstKind},
    ast_to_hir::AstToHir,
    hir::{Expression, HirDb, Id},
    module::{Module, ModuleDb},
    position::PositionConversionDb,
    rich_ir::ToRichIr,
};
use candy_vm::{
    context::RunLimitedNumberOfInstructions,
    fiber::FiberId,
    heap::Closure,
    lir::Lir,
    tracer::{
        full::{FullTracer, StoredFiberEvent, StoredVmEvent, TimedEvent},
        stack_trace::Call,
    },
    vm::{self, Vm},
};
use itertools::Itertools;
use rand::{prelude::SliceRandom, thread_rng};
use rustc_hash::FxHashMap;
use std::sync::Arc;
use tracing::{span, Level};

#[derive(Default)]
pub struct ConstantEvaluator {
    evaluators: FxHashMap<Module, Evaluator>,
}
struct Evaluator {
    lir: Arc<Lir>,
    tracer: FullTracer,
    vm: Vm<Arc<Lir>>,
}

impl ConstantEvaluator {
    pub fn update_module(&mut self, module: Module, lir: Arc<Lir>) {
        let vm = Vm::for_module(lir.clone());
        let tracer = FullTracer::default();
        self.evaluators
            .insert(module, Evaluator { lir, tracer, vm });
    }

    pub fn remove_module(&mut self, module: Module) {
        self.evaluators.remove(&module).unwrap();
    }

    pub fn run(&mut self) -> Option<Module> {
        let mut running_evaluators = self
            .evaluators
            .iter_mut()
            .filter(|(_, evaluator)| matches!(evaluator.vm.status(), vm::Status::CanRun))
            .collect_vec();
        let (module, evaluator) = running_evaluators.choose_mut(&mut thread_rng())?;

        evaluator.vm.run(
            &mut RunLimitedNumberOfInstructions::new(500),
            &mut evaluator.tracer,
        );
        Some(module.clone())
    }

    pub fn get_fuzzable_closures(&self, module: &Module) -> (Arc<Lir>, Vec<(Id, Closure)>) {
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
                } => Some((id.get().to_owned(), *closure)),
                _ => None,
            })
            .collect();
        (evaluator.lir.clone(), fuzzable_closures)
    }

    pub fn get_hints<DB>(&self, db: &DB, module: &Module) -> Vec<Hint>
    where
        DB: AstDb + AstToHir + HirDb + ModuleDb + PositionConversionDb,
    {
        let module_string = module.to_rich_ir().text;
        let span = span!(Level::DEBUG, "Calculating hints", %module_string);
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
            let id = expression.get();
            if &id.module != module {
                continue;
            }

            let Some(hir) = db.find_expression(id.clone()) else { continue; };
            match hir {
                Expression::Reference(_) => {
                    // Could be an assignment.
                    let Some(ast_id) = db.hir_to_ast_id(id.clone()) else { continue; };
                    let Some(ast) = db.find_ast(ast_id) else { continue; };
                    let AstKind::Assignment(Assignment { body, .. }) = &ast.kind else { continue; };
                    let creates_hint = match body {
                        AssignmentBody::Lambda { .. } => true,
                        AssignmentBody::Body { pattern, .. } => {
                            matches!(pattern.kind, AstKind::Identifier(_))
                        }
                    };
                    if !creates_hint {
                        continue;
                    }

                    hints.push(Hint {
                        kind: HintKind::Value,
                        text: value.to_string(),
                        position: db.id_to_end_of_line(id.clone()).unwrap(),
                    });
                }
                Expression::PatternIdentifierReference { .. } => {
                    let body = db.containing_body_of(id.clone());
                    let name = body.identifiers.get(id).unwrap();
                    hints.push(Hint {
                        kind: HintKind::Value,
                        text: format!("{name} = {value}"),
                        position: db.id_to_end_of_line(id.clone()).unwrap(),
                    });
                }
                _ => {}
            }
        }

        hints
    }
}

fn panic_hint<DB>(db: &DB, module: Module, evaluator: &Evaluator, reason: String) -> Option<Hint>
where
    DB: AstToHir + ModuleDb + PositionConversionDb,
{
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

    // Find the last call in this module.
    let (
        Call {
            callee, arguments, ..
        },
        call_site,
    ) = stack
        .iter()
        .map(|call| (call, call.call_site.get().to_owned()))
        .find(|(_, call_site)| {
            // Make sure the entry comes from the same file and is not generated
            // code.
            call_site.module == module && db.hir_to_cst_id(call_site.to_owned()).is_some()
        })?;

    let call_info = format!(
        "{callee} {}",
        arguments.iter().map(|it| it.to_string()).join(" "),
    );

    Some(Hint {
        kind: HintKind::Panic,
        text: format!("Calling `{call_info}` panics: {reason}"),
        position: db.id_to_end_of_line(call_site)?,
    })
}
