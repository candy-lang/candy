use super::{utils::IdToEndOfLine, Hint, HintKind};
use crate::utils::JoinWithCommasAndAnd;
use candy_frontend::{
    ast::{Assignment, AssignmentBody, AstDb, AstKind},
    ast_to_hir::AstToHir,
    cst::CstDb,
    hir::{self, Expression, HirDb, Id},
    mir_optimize::OptimizeMir,
    module::{Module, ModuleDb},
    position::PositionConversionDb,
    TracingConfig, TracingMode,
};
use candy_fuzzer::{FuzzablesFinder, Fuzzer, Status};
use candy_vm::{
    context::RunLimitedNumberOfInstructions,
    fiber::{EndedReason, VmEnded},
    lir::Lir,
    mir_to_lir::compile_lir,
    tracer::{
        compound::CompoundTracer,
        evaluated_values::EvaluatedValuesTracer,
        stack_trace::{Call, StackTracer},
    },
    vm::{self, Vm},
};
use itertools::Itertools;
use rand::{prelude::SliceRandom, thread_rng};
use rustc_hash::FxHashMap;
use std::{mem, sync::Arc};
use tracing::error;

pub enum HintsFinder {
    /// First, we run the module. This produces hints for constants and finds
    /// fuzzable closures that we can later analyze.
    Evaluate {
        lir: Arc<Lir>,
        tracer: HintsTracer,
        vm: Vm<Arc<Lir>, HintsTracer>,
    },
    /// Then, functions are fuzzed.
    Fuzz {
        ended: VmEnded,
        stack_tracer: StackTracer,
        evaluated_values: EvaluatedValuesTracer,
        fuzzers: FxHashMap<Id, Fuzzer>,
    },
}
type HintsTracer =
    CompoundTracer<StackTracer, CompoundTracer<EvaluatedValuesTracer, FuzzablesFinder>>;

impl HintsFinder {
    pub fn for_module(db: &(impl CstDb + OptimizeMir), module: Module) -> Self {
        let tracing = TracingConfig {
            register_fuzzables: TracingMode::OnlyCurrent,
            calls: TracingMode::Off,
            evaluated_expressions: TracingMode::OnlyCurrent,
        };
        let (lir, _) = compile_lir(db, module.clone(), tracing);
        let lir = Arc::new(lir);

        let mut tracer = CompoundTracer {
            tracer0: StackTracer::default(),
            tracer1: CompoundTracer {
                tracer0: EvaluatedValuesTracer::new(module),
                tracer1: FuzzablesFinder::default(),
            },
        };
        let vm = Vm::for_module(lir.clone(), &mut tracer);

        Self::Evaluate { lir, tracer, vm }
    }

    pub fn run(&mut self) {
        match self {
            HintsFinder::Evaluate { lir, tracer, vm } => {
                vm.run(&mut RunLimitedNumberOfInstructions::new(500), tracer);
                if matches!(vm.status(), vm::Status::Done | vm::Status::Panicked(_)) {
                    let vm = mem::replace(vm, Vm::uninitialized(lir.clone()));
                    let ended = vm.tear_down(tracer);
                    let CompoundTracer {
                        tracer0: stack_tracer,
                        tracer1:
                            CompoundTracer {
                                tracer0: evaluated_values,
                                tracer1: fuzzables_finder,
                            },
                    } = tracer;
                    let fuzzers = tracer
                        .tracer1
                        .tracer1
                        .fuzzables()
                        .unwrap()
                        .iter()
                        .map(|(id, function)| {
                            (id.clone(), Fuzzer::new(lir.clone(), *function, id.clone()))
                        })
                        .collect();
                    *self = Self::Fuzz {
                        ended,
                        stack_tracer: mem::take(stack_tracer),
                        evaluated_values: mem::replace(
                            evaluated_values,
                            EvaluatedValuesTracer::new(Module::from_package_name(
                                "Dummy".to_string(),
                            )),
                        ),
                        fuzzers,
                    };
                }
            }
            HintsFinder::Fuzz { ended, fuzzers, .. } => {
                // debug!("Fuzzing with {} fuzzers.", fuzzers.len());
                let mut running_fuzzers = fuzzers
                    .values_mut()
                    .filter(|fuzzer| matches!(fuzzer.status(), Status::StillFuzzing { .. }))
                    .collect_vec();
                let Some(fuzzer) = running_fuzzers.choose_mut(&mut thread_rng()) else { return; };
                fuzzer.run(&mut RunLimitedNumberOfInstructions::new(500));

                match &fuzzer.status() {
                    Status::StillFuzzing { .. } => None,
                    Status::FoundPanic { .. } => Some(fuzzer.function_id.module.clone()),
                    Status::TotalCoverageButNoPanic => None,
                };
            }
        }
    }

    pub fn hints<DB>(&self, db: &DB, module: &Module) -> Vec<Vec<Hint>>
    where
        DB: AstDb + AstToHir + HirDb + ModuleDb + PositionConversionDb,
    {
        let mut hints = vec![];

        match &self {
            HintsFinder::Evaluate { lir, tracer, vm } => {
                // TODO: Show incremental constant evaluation hints.
            }
            HintsFinder::Fuzz {
                ended,
                stack_tracer,
                evaluated_values,
                fuzzers,
            } => {
                // TODO: Think about how to highlight the responsible piece of code.
                if let EndedReason::Panicked(panic) = &ended.reason {
                    if let Some(hint) = panic_hint(db, module.clone(), stack_tracer, &panic.reason)
                    {
                        hints.push(vec![hint]);
                    }
                }

                for (id, value) in evaluated_values.values() {
                    let Some(hir) = db.find_expression(id.clone()) else { continue; };
                    match hir {
                        Expression::Reference(_) => {
                            // Could be an assignment.
                            let Some(ast_id) = db.hir_to_ast_id(id.clone()) else { continue; };
                            let Some(ast) = db.find_ast(ast_id) else { continue; };
                            let AstKind::Assignment(Assignment { body, .. }) = &ast.kind else { continue; };
                            let creates_hint = match body {
                                AssignmentBody::Function { .. } => true,
                                AssignmentBody::Body { pattern, .. } => {
                                    matches!(pattern.kind, AstKind::Identifier(_))
                                }
                            };
                            if !creates_hint {
                                continue;
                            }

                            hints.push(vec![Hint {
                                kind: HintKind::Value,
                                text: value.to_string(),
                                position: db.id_to_end_of_line(id.clone()).unwrap(),
                            }]);
                        }
                        Expression::PatternIdentifierReference { .. } => {
                            let body = db.containing_body_of(id.clone());
                            let name = body.identifiers.get(id).unwrap();
                            hints.push(vec![Hint {
                                kind: HintKind::Value,
                                text: format!("{name} = {value}"),
                                position: db.id_to_end_of_line(id.clone()).unwrap(),
                            }]);
                        }
                        _ => {}
                    }
                }

                for fuzzer in fuzzers.values() {
                    let Status::FoundPanic {
                        input,
                        panic,
                        ..
                    } = fuzzer.status() else { continue; };

                    let id = fuzzer.function_id.clone();
                    let first_hint = {
                        let parameter_names = match db.find_expression(id.clone()) {
                            Some(Expression::Function(hir::Function { parameters, .. })) => {
                                parameters
                                    .into_iter()
                                    .map(|parameter| parameter.keys.last().unwrap().to_string())
                                    .collect_vec()
                            }
                            Some(_) => panic!("Looks like we fuzzed a non-function. That's weird."),
                            None => {
                                error!("Using fuzzing, we found an error in a generated function.");
                                continue;
                            }
                        };
                        Hint {
                            kind: HintKind::Fuzz,
                            text: format!(
                                "If this is called with {},",
                                parameter_names
                                    .iter()
                                    .zip(input.arguments.iter())
                                    .map(|(name, argument)| format!("`{name} = {argument:?}`"))
                                    .collect_vec()
                                    .join_with_commas_and_and(),
                            ),
                            position: db.id_to_end_of_line(id.clone()).unwrap(),
                        }
                    };

                    let second_hint = {
                        if &panic.responsible.module != module {
                            // The function panics internally for an input, but it's the
                            // fault of an inner function that's in another module.
                            // TODO: The fuzz case should instead be highlighted in the
                            // used function directly. We don't do that right now
                            // because we assume the fuzzer will find the panic when
                            // fuzzing the faulty function, but we should save the
                            // panicking case (or something like that) in the future.
                            continue;
                        }
                        if db.hir_to_cst_id(id.clone()).is_none() {
                            panic!(
                                "It looks like the generated code {} is at fault for a panic.",
                                panic.responsible,
                            );
                        }

                        // TODO: In the future, re-run only the failing case with
                        // tracing enabled and also show the arguments to the failing
                        // function in the hint.
                        Hint {
                            kind: HintKind::Fuzz,
                            text: format!("then {} panics: {}", panic.responsible, panic.reason),
                            position: db.id_to_end_of_line(panic.responsible.clone()).unwrap(),
                        }
                    };

                    hints.push(vec![first_hint, second_hint]);
                }
            }
        }

        hints
    }
}

fn panic_hint<DB>(db: &DB, module: Module, tracer: &StackTracer, reason: &str) -> Option<Hint>
where
    DB: AstToHir + ModuleDb + PositionConversionDb,
{
    // We want to show the hint at the last call site still inside the current
    // module. If there is no call site in this module, then the panic results
    // from a compiler error in a previous stage which is already reported.
    let stack = tracer.panic_chain().unwrap();

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
