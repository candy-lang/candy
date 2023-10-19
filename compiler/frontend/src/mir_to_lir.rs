use crate::{
    error::CompilerError,
    hir::{self},
    id::CountableId,
    lir::{self, Lir},
    mir::{self},
    mir_optimize::OptimizeMir,
    module::Module,
    string_to_rcst::ModuleError,
    TracingConfig,
};
use itertools::Itertools;
use rustc_hash::{FxHashMap, FxHashSet};
use std::sync::Arc;

#[salsa::query_group(MirToLirStorage)]
pub trait MirToLir: OptimizeMir {
    fn lir(&self, module: Module, tracing: TracingConfig) -> LirResult;
}

pub type LirResult = Result<(Arc<Lir>, Arc<FxHashSet<CompilerError>>), ModuleError>;

fn lir(db: &dyn MirToLir, module: Module, tracing: TracingConfig) -> LirResult {
    let (mir, _, _, errors) = db.optimized_mir(module.clone(), tracing)?;

    let mut context = LoweringContext::default();
    context.compile_function(
        FxHashSet::from_iter([hir::Id::new(module, vec![])]),
        &[],
        &[],
        mir::Id::from_usize(0),
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
        responsible_parameter: mir::Id,
        body: &mir::Body,
    ) -> lir::BodyId {
        let body = CurrentBody::compile_function(
            self,
            original_hirs,
            captured,
            parameters,
            responsible_parameter,
            body,
        );
        self.bodies.push(body)
    }
}

#[derive(Clone, Debug)]
struct CurrentBody {
    id_mapping: FxHashMap<mir::Id, lir::Id>,
    body: lir::Body,
    last_constant: Option<mir::Id>,
    ids_to_drop: FxHashSet<lir::Id>,
}
impl CurrentBody {
    fn compile_function(
        context: &mut LoweringContext,
        original_hirs: FxHashSet<hir::Id>,
        captured: &[mir::Id],
        parameters: &[mir::Id],
        responsible_parameter: mir::Id,
        body: &mir::Body,
    ) -> lir::Body {
        let mut lir_body = Self::new(original_hirs, captured, parameters, responsible_parameter);
        for (id, expression) in body.iter() {
            lir_body.compile_expression(context, id, expression);
        }
        lir_body.finish(&context.constant_mapping)
    }

    fn new(
        original_hirs: FxHashSet<hir::Id>,
        captured: &[mir::Id],
        parameters: &[mir::Id],
        responsible_parameter: mir::Id,
    ) -> Self {
        let body = lir::Body::new(original_hirs, captured.len(), parameters.len());
        let id_mapping: FxHashMap<_, _> = captured
            .iter()
            .chain(parameters.iter())
            .copied()
            .chain([responsible_parameter])
            .enumerate()
            .map(|(index, id)| (id, lir::Id::from_usize(index)))
            .collect();
        // The responsible parameter is a HIR ID, which is always constant.
        // Hence, it never has to be dropped.
        let ids_to_drop = id_mapping
            .iter()
            .filter(|(&k, _)| k != responsible_parameter)
            .map(|(_, v)| v)
            .copied()
            .collect();
        Self {
            id_mapping,
            body,
            last_constant: None,
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
                        self.push(
                            id,
                            lir::Expression::CreateTag {
                                symbol: symbol.clone(),
                                value: self.id_mapping[value],
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
                responsible_parameter,
                body,
            } => {
                let captured = expression
                    .captured_ids()
                    .into_iter()
                    .filter(|captured| !context.constant_mapping.contains_key(captured))
                    .sorted()
                    .collect_vec();

                let body_id = context.compile_function(
                    original_hirs.clone(),
                    &captured,
                    parameters,
                    *responsible_parameter,
                    body,
                );
                if captured.is_empty() {
                    self.push_constant(context, id, body_id);
                } else {
                    let captured = captured.iter().map(|it| self.id_mapping[it]).collect();
                    self.push(id, lir::Expression::CreateFunction { captured, body_id });
                }
            }
            mir::Expression::Parameter => unreachable!(),
            mir::Expression::Call {
                function,
                arguments,
                responsible,
            } => {
                let function = self.id_for(context, *function);
                let arguments = self.ids_for(context, arguments);
                let responsible = self.id_for(context, *responsible);
                self.push(
                    id,
                    lir::Expression::Call {
                        function,
                        arguments,
                        responsible,
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
                responsible,
            } => {
                let hir_call = self.id_for(context, *hir_call);
                let function = self.id_for(context, *function);
                let arguments = self.ids_for(context, arguments);
                let responsible = self.id_for(context, *responsible);
                self.push(
                    id,
                    lir::Expression::TraceCallStarts {
                        hir_call,
                        function,
                        arguments,
                        responsible,
                    },
                );
            }
            mir::Expression::TraceCallEnds { return_value } => {
                let return_value = self.id_for(context, *return_value);
                self.push(id, lir::Expression::TraceCallEnds { return_value });
            }
            mir::Expression::TraceExpressionEvaluated {
                hir_expression,
                value,
            } => {
                let hir_expression = self.id_for(context, *hir_expression);
                let value = self.id_for(context, *value);
                self.push(
                    id,
                    lir::Expression::TraceExpressionEvaluated {
                        hir_expression,
                        value,
                    },
                );
            }
            mir::Expression::TraceFoundFuzzableFunction {
                hir_definition,
                function,
            } => {
                let hir_definition = self.id_for(context, *hir_definition);
                let function = self.id_for(context, *function);
                self.push(
                    id,
                    lir::Expression::TraceFoundFuzzableFunction {
                        hir_definition,
                        function,
                    },
                );
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
    fn push_constant(
        &mut self,
        context: &mut LoweringContext,
        id: mir::Id,
        constant: impl Into<lir::Constant>,
    ) {
        let constant_id = context.constants.push(constant);
        context.constant_mapping.insert(id, constant_id);
        self.last_constant = Some(id);
    }

    fn push(&mut self, mir_id: mir::Id, expression: impl Into<lir::Expression>) -> lir::Id {
        let expression = expression.into();
        let is_constant = matches!(expression, lir::Expression::Constant(_));
        self.body.push(expression);

        let id = self.body.last_expression_id().unwrap();
        assert!(self.id_mapping.insert(mir_id, id).is_none());
        if !is_constant {
            assert!(self.ids_to_drop.insert(id));
        }
        id
    }

    fn maybe_dup(&mut self, id: lir::Id) {
        if !self.ids_to_drop.contains(&id) {
            return;
        }

        self.body.push(lir::Expression::Dup { id, amount: 1 });
    }
    fn finish(mut self, constant_mapping: &FxHashMap<mir::Id, lir::ConstantId>) -> lir::Body {
        if self.body.expressions().is_empty() {
            // If the top-level MIR contains only constants, its LIR body will
            // still be empty. Hence, we push a reference to the last constant
            // we encountered.
            let last_constant_id = self.last_constant.unwrap();
            self.push(last_constant_id, constant_mapping[&last_constant_id]);
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
