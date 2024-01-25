use crate::{
    error::CompilerError,
    hir::{self},
    hir_to_mir::ExecutionTarget,
    id::CountableId,
    lir::{self, Lir},
    mir::{self},
    mir_optimize::OptimizeMir,
    string_to_rcst::ModuleError,
    utils::{HashMapExtension, HashSetExtension},
    TracingConfig,
};
use itertools::Itertools;
use rustc_hash::{FxHashMap, FxHashSet};
use std::sync::Arc;

#[salsa::query_group(MirToLirStorage)]
pub trait MirToLir: OptimizeMir {
    fn lir(&self, target: ExecutionTarget, tracing: TracingConfig) -> LirResult;
}

pub type LirResult = Result<(Arc<Lir>, Arc<FxHashSet<CompilerError>>), ModuleError>;

fn lir(db: &dyn MirToLir, target: ExecutionTarget, tracing: TracingConfig) -> LirResult {
    let module = target.module().clone();
    let (mir, errors) = db.optimized_mir(target, tracing)?;

    let mut context = LoweringContext::default();
    context.compile_function(
        FxHashSet::from_iter([hir::Id::new(module, vec![])]),
        &[],
        &[mir::Id::from_usize(0)],
        &mir.body,
    );
    let lir = Lir::new(context.constants, context.bodies);

    Ok((Arc::new(lir), errors))
}

#[derive(Clone, Debug, Default)]
struct LoweringContext {
    constants: lir::Constants,
    constant_mapping: FxHashMap<mir::Id, lir::ConstantId>,
    bodies: lir::Bodies,
}
impl LoweringContext {
    fn constant_for(&self, id: mir::Id) -> Option<lir::ConstantId> {
        self.constant_mapping.get(&id).copied()
    }

    fn compile_function(
        &mut self,
        original_hirs: FxHashSet<hir::Id>,
        captured: &[mir::Id],
        parameters: &[mir::Id],
        body: &mir::Body,
    ) -> lir::BodyId {
        let body = CurrentBody::compile_function(self, original_hirs, captured, parameters, body);
        self.bodies.push(body)
    }
}

#[derive(Clone, Debug)]
struct CurrentBody {
    id_mapping: FxHashMap<mir::Id, lir::Id>,
    body: lir::Body,
    current_constant: Option<mir::Id>,
    ids_to_drop: FxHashSet<lir::Id>,
}
impl CurrentBody {
    fn compile_function(
        context: &mut LoweringContext,
        original_hirs: FxHashSet<hir::Id>,
        captured: &[mir::Id],
        parameters: &[mir::Id],
        body: &mir::Body,
    ) -> lir::Body {
        let mut lir_body = Self::new(original_hirs, captured, parameters);
        for (id, expression) in body.iter() {
            lir_body.current_constant = None;
            lir_body.compile_expression(context, id, expression);
        }
        lir_body.finish(&context.constant_mapping)
    }

    fn new(
        original_hirs: FxHashSet<hir::Id>,
        captured: &[mir::Id],
        parameters: &[mir::Id],
    ) -> Self {
        let body = lir::Body::new(original_hirs, captured.len(), parameters.len());
        let id_mapping: FxHashMap<_, _> = captured
            .iter()
            .chain(parameters.iter())
            .copied()
            .enumerate()
            .map(|(index, id)| (id, lir::Id::from_usize(index)))
            .collect();
        // The responsible parameter is a HIR ID, which is (almost) always
        // constant. Hence, it doesn't normally have to be dropped.
        //
        // The exception is the responsible parameter passed when starting a VM,
        // which can be constant or non-constant.
        let ids_to_drop = id_mapping
            .values()
            .filter(
                #[allow(clippy::suspicious_operation_groupings)]
                |&lir_id| {
                    // Captured values should not be dropped in case the function is
                    // called again. They are dropped when the function object
                    // itself is dropped.
                    lir_id.to_usize() >= captured.len()
                },
            )
            .copied()
            .collect();
        Self {
            id_mapping,
            body,
            current_constant: None,
            ids_to_drop,
        }
    }

    fn compile_expression(
        &mut self,
        context: &mut LoweringContext,
        id: mir::Id,
        expression: &mir::Expression,
    ) {
        match expression {
            mir::Expression::Int(int) => self.push_constant(context, id, int.clone()),
            mir::Expression::Text(text) => self.push_constant(context, id, text.clone()),
            mir::Expression::Tag { symbol, value } => {
                if let Some(value) = value {
                    if let Some(constant_id) = context.constant_for(*value) {
                        self.push_constant(
                            context,
                            id,
                            lir::Constant::Tag {
                                symbol: symbol.clone(),
                                value: Some(constant_id),
                            },
                        );
                    } else {
                        let value = self.id_for(context, *value);
                        self.push(
                            id,
                            lir::Expression::CreateTag {
                                symbol: symbol.clone(),
                                value,
                            },
                        );
                    }
                } else {
                    self.push_constant(
                        context,
                        id,
                        lir::Constant::Tag {
                            symbol: symbol.clone(),
                            value: None,
                        },
                    );
                }
            }
            mir::Expression::Builtin(builtin) => self.push_constant(context, id, *builtin),
            mir::Expression::List(items) => {
                if let Some(items) = items
                    .iter()
                    .map(|item| context.constant_for(*item))
                    .collect::<Option<Vec<_>>>()
                {
                    self.push_constant(context, id, items);
                } else {
                    let items = self.ids_for(context, items);
                    self.push(id, items);
                }
            }
            mir::Expression::Struct(fields) => {
                if let Some(fields) = fields
                    .iter()
                    .map(|(key, value)| try {
                        (context.constant_for(*key)?, context.constant_for(*value)?)
                    })
                    .collect::<Option<FxHashMap<_, _>>>()
                {
                    self.push_constant(context, id, fields);
                } else {
                    let fields = fields
                        .iter()
                        .map(|(key, value)| {
                            (self.id_for(context, *key), self.id_for(context, *value))
                        })
                        .collect_vec();
                    self.push(id, fields);
                }
            }
            mir::Expression::Reference(referenced_id) => {
                // References only remain in the MIR to return a constant from a
                // function.
                if let Some(&referenced_id) = self.id_mapping.get(referenced_id) {
                    self.maybe_dup(referenced_id);
                    // TODO: The reference following MIR optimization isn't
                    // always working correctly. Add the following code once it
                    // does work.
                    // assert!(
                    //     !self.ids_to_drop.contains(&referenced_id),
                    //     "References in the optimized MIR should only point to constants.",
                    // );
                    self.push(id, referenced_id);
                    return;
                }

                self.push(id, context.constant_for(*referenced_id).unwrap());
            }
            mir::Expression::HirId(hir_id) => self.push_constant(context, id, hir_id.clone()),
            mir::Expression::Function {
                original_hirs,
                parameters,
                body,
            } => {
                let captured = expression
                    .captured_ids()
                    .into_iter()
                    .filter(|captured| !context.constant_mapping.contains_key(captured))
                    .sorted()
                    .collect_vec();

                let body_id =
                    context.compile_function(original_hirs.clone(), &captured, parameters, body);
                if captured.is_empty() {
                    self.push_constant(context, id, body_id);
                } else {
                    let captured = self.ids_for(context, &captured);
                    self.push(id, lir::Expression::CreateFunction { captured, body_id });
                }
            }
            mir::Expression::Parameter => unreachable!(),
            mir::Expression::Call {
                function,
                arguments,
            } => {
                let function = self.id_for(context, *function);
                let arguments = self.ids_for(context, arguments);
                self.push(
                    id,
                    lir::Expression::Call {
                        function,
                        arguments,
                    },
                );
            }
            mir::Expression::UseModule { .. } => {
                // Calls of the use function are completely inlined and, if
                // they're not statically known, are replaced by panics.
                // The only way a use can still be in the MIR is if the tracing
                // of evaluated expressions is enabled. We can emit any nonsense
                // here, since the instructions will never be executed anyway.
                // We just push an empty struct, as if the imported module
                // hadn't exported anything.
                self.push(id, lir::Expression::CreateStruct(vec![]));
            }
            mir::Expression::Panic {
                reason,
                responsible,
            } => {
                let reason = self.id_for(context, *reason);
                let responsible = self.id_for(context, *responsible);
                self.push(
                    id,
                    lir::Expression::Panic {
                        reason,
                        responsible,
                    },
                );
            }
            mir::Expression::TraceCallStarts {
                hir_call,
                function,
                arguments,
            } => {
                let hir_call = self.id_for(context, *hir_call);
                let function = self.id_for(context, *function);
                let arguments = self.ids_for(context, arguments);
                self.push_without_value(lir::Expression::TraceCallStarts {
                    hir_call,
                    function,
                    arguments,
                });
            }
            mir::Expression::TraceCallEnds { return_value } => {
                let return_value = self.id_for(context, *return_value);
                self.push_without_value(lir::Expression::TraceCallEnds { return_value });
            }
            mir::Expression::TraceTailCall {
                hir_call,
                function,
                arguments,
            } => {
                let hir_call = self.id_for(context, *hir_call);
                let function = self.id_for(context, *function);
                let arguments = self.ids_for(context, arguments);
                self.push_without_value(lir::Expression::TraceTailCall {
                    hir_call,
                    function,
                    arguments,
                });
            }
            mir::Expression::TraceExpressionEvaluated {
                hir_expression,
                value,
            } => {
                let hir_expression = self.id_for(context, *hir_expression);
                let value = self.id_for(context, *value);
                self.push_without_value(lir::Expression::TraceExpressionEvaluated {
                    hir_expression,
                    value,
                });
            }
            mir::Expression::TraceFoundFuzzableFunction {
                hir_definition,
                function,
            } => {
                let hir_definition = self.id_for(context, *hir_definition);
                let function = self.id_for(context, *function);
                self.push_without_value(lir::Expression::TraceFoundFuzzableFunction {
                    hir_definition,
                    function,
                });
            }
        }
    }

    fn ids_for(&mut self, context: &LoweringContext, ids: &[mir::Id]) -> Vec<lir::Id> {
        ids.iter().map(|it| self.id_for(context, *it)).collect()
    }
    fn id_for(&mut self, context: &LoweringContext, id: mir::Id) -> lir::Id {
        if let Some(&id) = self.id_mapping.get(&id) {
            self.maybe_dup(id);
            return id;
        }

        self.push(id, context.constant_for(id).unwrap())
    }
    /// Resolve a [`mir::ID`] to a [`lir::ID`] without inserting a dup for it.
    ///
    /// This is used for the responsible parameter in function calls since it
    /// will always be const.
    fn id_for_without_dup(&mut self, context: &LoweringContext, id: mir::Id) -> lir::Id {
        if let Some(&id) = self.id_mapping.get(&id) {
            return id;
        }

        self.push(id, context.constant_for(id).unwrap())
    }

    fn push_constant(
        &mut self,
        context: &mut LoweringContext,
        id: mir::Id,
        constant: impl Into<lir::Constant>,
    ) {
        let constant_id = context.constants.push(constant);
        context.constant_mapping.insert(id, constant_id);
        self.current_constant = Some(id);
    }

    fn push(&mut self, mir_id: mir::Id, expression: impl Into<lir::Expression>) -> lir::Id {
        let expression = expression.into();
        let is_constant = matches!(expression, lir::Expression::Constant(_));
        let id = self.body.push(expression);
        self.id_mapping.force_insert(mir_id, id);
        if !is_constant {
            self.ids_to_drop.force_insert(id);
        }
        id
    }
    /// Push an expression that doesn't produce a return value, i.e., a trace
    /// expression.
    fn push_without_value(&mut self, expression: impl Into<lir::Expression>) {
        self.body.push(expression.into());
    }

    fn maybe_dup(&mut self, id: lir::Id) {
        // We need to dup all values that we determined we have to drop (via
        // `self.ids_to_drop`) plus:
        //
        // - Captured values: These are only dropped when the function object
        //   itself is dropped and are hence not part of `self.ids_to_drop`.
        // - The responsible parameter when it is passed as a normal parameter
        //   (only happens when calling the `needs` function): Since responsible
        //   parameters are almost always constant HIR IDs, we don't
        //   reference-count them for every function call (see
        //   `self.id_for_without_dup`). However, when starting the VM with a
        //   non-constant HIR ID, this top-level responsibility could be dropped
        //   when calling `needs`.
        let is_captured = id.to_usize() < self.body.captured_count();
        if !is_captured && !self.ids_to_drop.contains(&id) {
            return;
        }

        self.body.push(lir::Expression::Dup { id, amount: 1 });
    }
    fn finish(mut self, constant_mapping: &FxHashMap<mir::Id, lir::ConstantId>) -> lir::Body {
        if let Some(current_constant) = self.current_constant {
            // If the top-level MIR contains only constants, its LIR body will
            // still be empty. Hence, we push a reference to the last constant
            // we encountered.
            self.push(current_constant, constant_mapping[&current_constant]);
        }

        let last_expression_id = self.body.last_expression_id().unwrap();
        self.ids_to_drop.remove(&last_expression_id);
        if !self.ids_to_drop.is_empty() {
            for id in self.ids_to_drop.iter().sorted().rev() {
                self.body.push(lir::Expression::Drop(*id));
            }
            self.body
                .push(lir::Expression::Reference(last_expression_id));
        }

        self.body
    }
}
