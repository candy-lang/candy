use crate::{
    error::CompilerError,
    hir,
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
use std::{mem, sync::Arc};
use tracing::debug;

#[salsa::query_group(MirToLirStorage)]
pub trait MirToLir: OptimizeMir {
    fn lir(&self, module: Module, tracing: TracingConfig) -> LirResult;
}

pub type LirResult = Result<(Arc<Lir>, Arc<FxHashSet<CompilerError>>), ModuleError>;

fn lir(db: &dyn MirToLir, module: Module, tracing: TracingConfig) -> LirResult {
    let (mir, _, errors) = db.optimized_mir(module.clone(), tracing)?;

    debug!("Lowering MIR to LIR: {module}");
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
    current_body: CurrentBody,
    last_constant: Option<mir::Id>,
}
impl LoweringContext {
    fn compile_function(
        &mut self,
        original_hirs: FxHashSet<hir::Id>,
        captured: &[mir::Id],
        parameters: &[mir::Id],
        responsible_parameter: mir::Id,
        body: &mir::Body,
    ) -> lir::BodyId {
        debug!(
            "compile_function({:?}, {:?}, {:?}, {:?}, {:?})",
            original_hirs, captured, parameters, responsible_parameter, body,
        );
        let inner_body = CurrentBody::new(captured, parameters, responsible_parameter);
        let outer_body = mem::replace(&mut self.current_body, inner_body);

        for (id, expression) in body.iter() {
            self.compile_expression(id, expression);
        }

        if self.current_body.expressions.is_empty() {
            // If the MIR body only contains constants, its LIR body will still
            // be empty. Hence, we push a reference to the last constant we
            // encountered.
            let last_constant_id = self.last_constant.unwrap();
            self.current_body
                .push(last_constant_id, self.constant_mapping[&last_constant_id]);
        }

        let inner_body = mem::replace(&mut self.current_body, outer_body);
        debug!("Body compiled");
        self.bodies.push(inner_body.finish(original_hirs))
    }
    fn compile_expression(&mut self, id: mir::Id, expression: &mir::Expression) {
        debug!("compile_expression({:?}, {:?})", id, expression);
        match expression {
            mir::Expression::Int(int) => self.push_constant(id, int.clone()),
            mir::Expression::Text(text) => self.push_constant(id, text.clone()),
            mir::Expression::Tag { symbol, value } => {
                if let Some(value) = value {
                    if let Some(constant_id) = self.constant_for(*value) {
                        self.push_constant(
                            id,
                            lir::Constant::Tag {
                                symbol: symbol.clone(),
                                value: Some(constant_id),
                            },
                        );
                    } else {
                        self.current_body.push(
                            id,
                            lir::Expression::CreateTag {
                                symbol: symbol.clone(),
                                value: self.current_body.id_mapping[value],
                            },
                        );
                    }
                } else {
                    self.push_constant(
                        id,
                        lir::Constant::Tag {
                            symbol: symbol.clone(),
                            value: None,
                        },
                    );
                }
            }
            mir::Expression::Builtin(builtin) => self.push_constant(id, *builtin),
            mir::Expression::List(items) => {
                if let Some(items) = items
                    .iter()
                    .map(|item| self.constant_for(*item))
                    .collect::<Option<Vec<_>>>()
                {
                    self.push_constant(id, items);
                } else {
                    let items = self.ids_for(items);
                    self.current_body.push(id, items);
                }
            }
            mir::Expression::Struct(fields) => {
                if let Some(fields) = fields
                    .iter()
                    .map(|(key, value)| try {
                        (self.constant_for(*key)?, self.constant_for(*value)?)
                    })
                    .collect::<Option<FxHashMap<_, _>>>()
                {
                    self.push_constant(id, fields);
                } else {
                    let fields = fields
                        .iter()
                        .map(|(key, value)| (self.id_for(*key), self.id_for(*value)))
                        .collect_vec();
                    self.current_body.push(id, fields);
                }
            }
            mir::Expression::Reference(referenced_id) => {
                let referenced_id = self.id_for(*referenced_id);
                self.current_body.push(id, referenced_id);
            }
            mir::Expression::HirId(hir_id) => self.push_constant(id, hir_id.clone()),
            mir::Expression::Function {
                original_hirs,
                parameters,
                responsible_parameter,
                body,
            } => {
                let captured = expression
                    .captured_ids()
                    .into_iter()
                    .filter(|captured| !self.constant_mapping.contains_key(captured))
                    .sorted()
                    .collect_vec();

                let body_id = self.compile_function(
                    original_hirs.clone(),
                    &captured,
                    parameters,
                    *responsible_parameter,
                    body,
                );
                if captured.is_empty() {
                    self.push_constant(id, body_id);
                } else {
                    let captured = captured
                        .iter()
                        .map(|it| self.current_body.id_mapping[it])
                        .collect();
                    self.current_body
                        .push(id, lir::Expression::CreateFunction { captured, body_id });
                }
            }
            mir::Expression::Parameter => {
                panic!("The MIR should not contain any parameter expressions.")
            }
            mir::Expression::Call {
                function,
                arguments,
                responsible,
            } => {
                let function = self.id_for(*function);
                let arguments = self.ids_for(arguments);
                let responsible = self.id_for(*responsible);
                self.current_body.push(
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
                self.current_body
                    .push(id, lir::Expression::CreateStruct(vec![]));
            }
            mir::Expression::Panic {
                reason,
                responsible,
            } => {
                let reason = self.id_for(*reason);
                let responsible = self.id_for(*responsible);
                self.current_body.push(
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
                let hir_call = self.id_for(*hir_call);
                let function = self.id_for(*function);
                let arguments = self.ids_for(arguments);
                let responsible = self.id_for(*responsible);
                self.current_body.push(
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
                let return_value = self.id_for(*return_value);
                self.current_body
                    .push(id, lir::Expression::TraceCallEnds { return_value });
            }
            mir::Expression::TraceExpressionEvaluated {
                hir_expression,
                value,
            } => {
                let hir_expression = self.id_for(*hir_expression);
                let value = self.id_for(*value);
                self.current_body.push(
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
                let hir_definition = self.id_for(*hir_definition);
                let function = self.id_for(*function);
                self.current_body.push(
                    id,
                    lir::Expression::TraceFoundFuzzableFunction {
                        hir_definition,
                        function,
                    },
                );
            }
        }
    }

    fn ids_for(&mut self, ids: &[mir::Id]) -> Vec<lir::Id> {
        ids.iter().map(|it| self.id_for(*it)).collect()
    }
    fn id_for(&mut self, id: mir::Id) -> lir::Id {
        if let Some(constant_id) = self.constant_for(id) {
            self.current_body.push(id, constant_id)
        } else {
            self.current_body.id_mapping[&id]
        }
    }

    fn push_constant(&mut self, id: mir::Id, constant: impl Into<lir::Constant>) {
        let constant_id = self.constants.push(constant);
        self.constant_mapping.insert(id, constant_id);
        self.last_constant = Some(id);
    }
    fn constant_for(&self, id: mir::Id) -> Option<lir::ConstantId> {
        self.constant_mapping.get(&id).copied()
    }
}

#[derive(Clone, Debug, Default)]
struct CurrentBody {
    id_mapping: FxHashMap<mir::Id, lir::Id>,
    captured_count: usize,
    parameter_count: usize,
    expressions: Vec<lir::Expression>,
}
impl CurrentBody {
    fn new(captured: &[mir::Id], parameters: &[mir::Id], responsible_parameter: mir::Id) -> Self {
        let captured_count = captured.len();
        let parameter_count = parameters.len();
        let id_mapping = captured
            .iter()
            .chain(parameters.iter())
            .copied()
            .chain([responsible_parameter])
            .enumerate()
            .map(|(index, id)| (id, lir::Id::from_usize(index)))
            .collect();
        Self {
            id_mapping,
            captured_count,
            parameter_count,
            expressions: Vec::new(),
        }
    }

    fn push(&mut self, mir_id: mir::Id, expression: impl Into<lir::Expression>) -> lir::Id {
        let expression = expression.into();
        debug!("Pushing {mir_id:?}: {expression:?}");
        self.expressions.push(expression.into());
        let id = lir::Id::from_usize(
            self.captured_count + self.parameter_count + self.expressions.len(),
        );
        assert!(self.id_mapping.insert(mir_id, id).is_none());
        id
    }
    fn finish(self, original_hirs: FxHashSet<hir::Id>) -> lir::Body {
        lir::Body::new(
            original_hirs,
            self.captured_count,
            self.parameter_count,
            self.expressions,
        )
    }
}
