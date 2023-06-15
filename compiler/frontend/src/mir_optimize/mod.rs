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
    current_expression::{CurrentExpression, ExpressionContext},
    pure::PurenessInsights,
};
use super::{hir, hir_to_mir::HirToMir, mir::Mir, tracing::TracingConfig};
use crate::{
    error::CompilerError,
    id::IdGenerator,
    mir::{Body, Expression, Id, MirError, VisibleExpressions},
    module::Module,
    string_to_rcst::ModuleError,
};
use rustc_hash::{FxHashSet, FxHasher};
use std::{
    hash::{Hash, Hasher},
    mem,
    sync::Arc,
};
use tracing::debug;

mod cleanup;
mod common_subtree_elimination;
mod complexity;
mod constant_folding;
mod constant_lifting;
mod current_expression;
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
        Arc<FxHashSet<CompilerError>>,
    ),
    ModuleError,
>;

fn optimized_mir(
    db: &dyn OptimizeMir,
    module: Module,
    tracing: TracingConfig,
) -> OptimizedMirResult {
    debug!("{module}: Compiling.");
    let (mir, errors) = db.mir(module.clone(), tracing.clone())?;
    let mut mir = (*mir).clone();
    let mut pureness = PurenessInsights::default();
    let mut errors = (*errors).clone();

    let complexity_before = mir.complexity();
    mir.optimize(db, &tracing, &mut pureness, &mut errors);
    let complexity_after = mir.complexity();

    debug!("{module}: Done. Optimized from {complexity_before} to {complexity_after}");
    Ok((Arc::new(mir), Arc::new(pureness), Arc::new(errors)))
}

impl Mir {
    pub fn optimize(
        &mut self,
        db: &dyn OptimizeMir,
        tracing: &TracingConfig,
        pureness: &mut PurenessInsights,
        errors: &mut FxHashSet<CompilerError>,
    ) {
        self.body.optimize(
            &mut VisibleExpressions::none_visible(),
            &mut self.id_generator,
            db,
            tracing,
            pureness,
            errors,
        );
        if cfg!(debug_assertions) {
            self.validate();
        }
        self.cleanup(pureness);
    }
}

impl Body {
    // Even though visible is mut, this function guarantees that the value is
    // the same after returning.
    fn optimize(
        &mut self,
        visible: &mut VisibleExpressions,
        id_generator: &mut IdGenerator<Id>,
        db: &dyn OptimizeMir,
        tracing: &TracingConfig,
        pureness: &mut PurenessInsights,
        errors: &mut FxHashSet<CompilerError>,
    ) {
        let mut index = 0;
        while index < self.expressions.len() {
            // Thoroughly optimize the expression.
            let mut expression_context = ExpressionContext {
                visible,
                expression: CurrentExpression::new(self, index),
            };
            expression_context.optimize(id_generator, db, tracing, pureness, errors);
            if cfg!(debug_assertions) {
                expression_context.validate(expression_context.visible);
            }

            let id = expression_context.expression.id();
            pureness.visit_optimized(id, &expression_context);

            module_folding::apply(
                &mut expression_context,
                id_generator,
                db,
                tracing,
                pureness,
                errors,
            );

            index = expression_context.expression.index() + 1;
            let expression = mem::replace(&mut **expression_context, Expression::Parameter);
            visible.insert(id, expression);
        }

        for (id, expression) in &mut self.expressions {
            *expression = visible.remove(*id);
        }

        common_subtree_elimination::eliminate_common_subtrees(self, pureness);
        reference_following::remove_redundant_return_references(self);
        tree_shaking::tree_shake(self, pureness);
    }
}

impl ExpressionContext<'_> {
    fn optimize(
        &mut self,
        id_generator: &mut IdGenerator<Id>,
        db: &dyn OptimizeMir,
        tracing: &TracingConfig,
        pureness: &mut PurenessInsights,
        errors: &mut FxHashSet<CompilerError>,
    ) {
        'outer: loop {
            if let Expression::Function {
                parameters,
                responsible_parameter,
                body,
                ..
            } = &mut *self.expression
            {
                for parameter in &*parameters {
                    self.visible.insert(*parameter, Expression::Parameter);
                }
                self.visible
                    .insert(*responsible_parameter, Expression::Parameter);
                pureness.enter_function(parameters, *responsible_parameter);

                body.optimize(self.visible, id_generator, db, tracing, pureness, errors);

                for parameter in &*parameters {
                    self.visible.remove(*parameter);
                }
                self.visible.remove(*responsible_parameter);
            }

            loop {
                let hashcode_before = self.do_hash();

                reference_following::follow_references(self);
                constant_folding::fold_constants(self, pureness, id_generator);

                let is_call = matches!(*self.expression, Expression::Call { .. });
                inlining::inline_tiny_functions(self, id_generator);
                inlining::inline_functions_containing_use(self, id_generator);
                if is_call && matches!(*self.expression, Expression::Function { .. }) {
                    // We inlined a function call and the resulting code starts with
                    // a function definition. We need to visit that first before
                    // continuing the optimizations.
                    continue 'outer;
                }

                constant_lifting::lift_constants(self, pureness, id_generator);

                if self.do_hash() == hashcode_before {
                    return;
                }
            }
        }
    }

    fn do_hash(&self) -> u64 {
        let mut hasher = FxHasher::default();
        self.expression.hash(&mut hasher);
        hasher.finish()
    }
}

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

    let mir = Mir::build(|body| {
        let reason = body.push_text(error.payload.to_string());
        let responsible = body.push_hir_id(hir::Id::new(module.clone(), vec![]));
        body.push_panic(reason, responsible);
    });

    Ok((
        Arc::new(mir),
        Arc::default(),
        Arc::new(FxHashSet::from_iter([error])),
    ))
}
