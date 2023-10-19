//! Optimizations are a necessity for Candy code to run reasonably fast. For
//! example, without optimizations, if two modules import a third module using
//! `use "..foo"`, then the `foo` module is instantiated twice completely
//! separately. Because this module can in turn depend on other modules, this
//! approach would lead to exponential code blowup.
//!
//! When optimizing code in general, there are two main objectives:
//!
//! - Making the code fast.
//! - Making the code small.
//!
//! Some optimizations benefit both of these objectives. For example, removing
//! ignored computations from the program makes it smaller, but also means
//! there's less code to be executed. Other optimizations further one objective,
//! but harm the other. For example, inlining functions (basically copying their
//! code to where they're used), can make the code bigger, but also potentially
//! faster because there are less function calls to be performed.
//!
//! Depending on the use case, the tradeoff between both objectives changes. To
//! put you in the right mindset, here are just two use cases:
//!
//! - Programming for a microcontroller with 1 MB of ROM available for the
//!   program. In this case, you want your code to be as fast as possible while
//!   still fitting in 1 MB. Interestingly, the importance of code size is a
//!   step function: There's no benefit in only using 0.5 MB, but 1.1 MB makes
//!   the program completely unusable.
//!
//! - Programming for a WASM module to be downloaded. In this case, you might
//!   have some concrete measurements on how performance and download size
//!   affect user retention.
//!
//! It should be noted that we can't judge performance statically. Although some
//! optimizations such as inlining typically improve performance, there are rare
//! cases where they don't. For example, inlining a function that's used in
//! multiple places means the CPU's branch predictor can't benefit from the
//! knowledge gained by previous function executions. Inlining might also make
//! your program bigger, causing more cache misses. Thankfully, Candy is not yet
//! optimized enough for us to care about such details.
//!
//! This module contains several optimizations. All of them operate on the MIR.
//! Some are called "obvious". Those are optimizations that typically improve
//! both performance and code size. Whenever they can be applied, they should be
//! applied.

use self::{
    current_expression::{Context, CurrentExpression},
    data_flow::{
        flow_value::FlowValue, insights::DataFlowInsights, operation::Panic, timeline::Timeline,
    },
    pure::PurenessInsights,
    utils::ReferenceCounts,
};
use super::{hir, hir_to_mir::HirToMir, mir::Mir, tracing::TracingConfig};
use crate::{
    error::CompilerError,
    id::IdGenerator,
    mir::{Body, BodyBuilder, Expression, MirError},
    module::Module,
    rich_ir::ToRichIr,
    string_to_rcst::ModuleError,
    utils::DoHash,
};
use rustc_hash::FxHashSet;
use std::{mem, sync::Arc};
use tracing::debug;

mod cleanup;
mod common_subtree_elimination;
mod complexity;
mod constant_folding;
mod constant_lifting;
mod current_expression;
mod data_flow;
mod inlining;
mod module_folding;
mod pure;
mod reference_following;
mod tree_shaking;
mod utils;
mod validate;

#[salsa::query_group(OptimizeMirStorage)]
pub trait OptimizeMir: HirToMir {
    #[salsa::cycle(recover_from_cycle)]
    fn optimized_mir(&self, module: Module, tracing: TracingConfig) -> OptimizedMirResult;
}

pub type OptimizedMirResult = Result<
    (
        Arc<Mir>,
        Arc<PurenessInsights>,
        Arc<DataFlowInsights>,
        Arc<FxHashSet<CompilerError>>,
    ),
    ModuleError,
>;

#[allow(clippy::needless_pass_by_value)]
fn optimized_mir(
    db: &dyn OptimizeMir,
    module: Module,
    tracing: TracingConfig,
) -> OptimizedMirResult {
    debug!("{module}: Compiling.");
    let (mir, errors) = db.mir(module.clone(), tracing.clone())?;
    let mut mir = (*mir).clone();

    let complexity_before = mir.complexity();
    // TODO: Track references to current expressions in the remaining
    // expressions as an `FxHashMap<Id, usize>` (from expression ID to number of
    // occurrences). This can be used for tree-shaking directly after each
    // expression.
    let mut context = Context::new(
        db,
        &tracing,
        (*errors).clone(),
        &mut mir.id_generator,
        &mir.body,
    );
    context.optimize_body(&mut mir.body);
    let Context {
        mut pureness,
        data_flow,
        errors,
        ..
    } = context;

    if cfg!(debug_assertions) {
        mir.validate();
    }
    let data_flow_insights = mir.cleanup(&mut pureness, data_flow);
    let complexity_after = mir.complexity();

    debug!("{module}: Done. Optimized from {complexity_before} to {complexity_after}");
    Ok((
        Arc::new(mir),
        Arc::new(pureness),
        Arc::new(data_flow_insights),
        Arc::new(errors),
    ))
}

impl Context<'_> {
    fn optimize_body(&mut self, body: &mut Body) {
        // Even though `self.visible` is mutable, this function guarantees that
        // the value is the same after returning.
        let mut index = 0;
        while index < body.expressions.len() {
            // Thoroughly optimize the expression.
            let mut expression = CurrentExpression::new(body, index);
            let original_reference_counts = expression.reference_counts();

            self.optimize_expression(&mut expression);
            if cfg!(debug_assertions) {
                expression.validate(&self.visible);
            }

            let id = expression.id();
            {
                let mut body = Body::default();
                body.expressions.push((id, (*expression).to_owned()));
                body.to_rich_ir(false).print_to_console();
            }

            // TODO: Remove pureness when data flow takes care of it.
            self.pureness.visit_optimized(id, &*expression);

            module_folding::apply(self, &mut expression);
            self.data_flow
                .visit_optimized(id, &*expression, &original_reference_counts);

            {
                println!("Data Flow Insights:");
                self.data_flow
                    .innermost_scope_to_rich_ir()
                    .print_to_console();
                println!();
            }

            let new_id = expression.id();
            index = expression.index() + 1;
            let expression = mem::replace(&mut *expression, Expression::Parameter);
            self.visible.insert(new_id, expression);

            if self.data_flow.is_unconditional_panic() {
                for (_, expression) in body.expressions.drain(index..) {
                    self.data_flow
                        .on_expression_passed(id, &expression.reference_counts());
                }
            }
        }

        for (id, expression) in &mut body.expressions {
            *expression = self.visible.remove(*id);
        }

        common_subtree_elimination::eliminate_common_subtrees(body, &self.pureness);
        tree_shaking::tree_shake(body, &self.pureness);
        reference_following::remove_redundant_return_references(body);
    }

    fn optimize_expression(&mut self, expression: &mut CurrentExpression) {
        'outer: loop {
            if let Expression::Function {
                parameters,
                responsible_parameter,
                body,
                ..
            } = &mut **expression
            {
                for parameter in &*parameters {
                    self.visible.insert(*parameter, Expression::Parameter);
                }
                self.visible
                    .insert(*responsible_parameter, Expression::Parameter);
                self.pureness
                    .enter_function(parameters, *responsible_parameter);
                self.data_flow
                    .enter_function(parameters.to_owned(), body.return_value());

                self.optimize_body(body);

                let parameters = parameters.to_owned();
                let return_value = body.return_value();
                for parameter in &parameters {
                    self.visible.remove(*parameter);
                }
                self.visible.remove(*responsible_parameter);

                let constants = constant_lifting::lift_constants(self, body);
                self.data_flow
                    .on_constants_lifted(constants.iter().map(|(id, _)| *id));
                expression.prepend_optimized(&mut self.visible, constants);
                self.data_flow
                    .exit_function(expression.id(), &parameters, return_value);

                return;
            }

            loop {
                let hashcode_before = expression.do_hash();

                reference_following::follow_references(self, expression);
                // TODO: Remove constant folding when data flow takes care of it.
                constant_folding::fold_constants(self, expression);

                let is_call = matches!(**expression, Expression::Call { .. });
                inlining::inline_tiny_functions(self, expression);
                inlining::inline_needs_function(self, expression);
                inlining::inline_functions_containing_use(self, expression);
                if is_call && matches!(**expression, Expression::Function { .. }) {
                    // We inlined a function call and the resulting code starts with
                    // a function definition. We need to visit that first before
                    // continuing the optimizations.
                    continue 'outer;
                }

                if expression.do_hash() == hashcode_before {
                    return;
                }
            }
        }
    }
}

#[allow(clippy::unnecessary_wraps)]
fn recover_from_cycle(
    _db: &dyn OptimizeMir,
    cycle: &[String],
    module: &Module,
    _tracing: &TracingConfig,
) -> OptimizedMirResult {
    let error = CompilerError::for_whole_module(
        module.clone(),
        MirError::ModuleHasCycle {
            cycle: cycle.to_vec(),
        },
    );

    let mut builder = BodyBuilder::new(IdGenerator::start_at(1));
    let mut timeline = Timeline::default();

    let reason = builder.push_text(error.payload.to_string());
    timeline.insert(reason, FlowValue::Text(error.payload.to_string()));

    let responsible = builder.push_hir_id(hir::Id::new(module.clone(), vec![]));

    builder.push_panic(reason, responsible);
    let panic = Panic {
        reason,
        responsible,
    };

    let (id_generator, body) = builder.finish();
    let mir = Mir { id_generator, body };
    let data_flow_insights = DataFlowInsights::new(vec![], vec![], timeline, Err(panic));

    Ok((
        Arc::new(mir),
        Arc::default(),
        Arc::new(data_flow_insights),
        Arc::new(FxHashSet::from_iter([error])),
    ))
}
