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

use self::pure::PurenessInsights;
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
        'expression_loop: while index < self.expressions.len() {
            let id = self.expressions[index].0;
            let mut expression =
                mem::replace(&mut self.expressions[index].1, Expression::Parameter);

            // Thoroughly optimize the expression.
            expression.optimize(visible, id_generator, db, tracing, pureness, errors);
            if cfg!(debug_assertions) {
                expression.validate(visible);
            }

            if self.fold_multiple(id, &mut expression, index).is_some() {
                // We replaced the expression with other expressions, so instead
                // of continuing to the next expression, we should try to
                // optimize the newly inserted expressions next.
                continue 'expression_loop;
            }
            pureness.visit_optimized(id, &expression);

            module_folding::apply(
                &mut expression,
                visible,
                id_generator,
                db,
                tracing,
                pureness,
                errors,
            );
            if let Some(index_after_module) = self.fold_multiple(id, &mut expression, index) {
                // A module folding actually happened. Because the inserted
                // module's MIR is already optimized and doesn't depend on any
                // context outside of itself, we don't need to analyze it again.
                while index < index_after_module {
                    let id = self.expressions[index].0;
                    let expression =
                        mem::replace(&mut self.expressions[index].1, Expression::Parameter);
                    visible.insert(id, expression);
                    index += 1;
                }
                continue 'expression_loop;
            }

            visible.insert(id, expression);
            index += 1;
        }

        for (id, expression) in &mut self.expressions {
            *expression = visible.remove(*id);
        }

        common_subtree_elimination::eliminate_common_subtrees(self, pureness);
        reference_following::remove_redundant_return_references(self);
        tree_shaking::tree_shake(self, pureness);
    }

    // If an `Expression::Multiple` was actually folded, this returns the index
    // of the expression after the newly inserted ones.
    fn fold_multiple(
        &mut self,
        id: Id,
        expression: &mut Expression,
        index: usize,
    ) -> Option<usize> {
        let Expression::Multiple(expressions) = expression else { return None; };
        let return_value = expressions.return_value();
        let num_expressions = expressions.expressions.len();
        self.expressions.splice(
            index..(index + 1),
            expressions
                .expressions
                .drain(..)
                .chain([(id, Expression::Reference(return_value))]),
        );
        Some(index + num_expressions + 1)
    }
}

impl Expression {
    fn optimize(
        &mut self,
        visible: &mut VisibleExpressions,
        id_generator: &mut IdGenerator<Id>,
        db: &dyn OptimizeMir,
        tracing: &TracingConfig,
        pureness: &mut PurenessInsights,
        errors: &mut FxHashSet<CompilerError>,
    ) {
        if let Expression::Function {
            parameters,
            responsible_parameter,
            body,
            ..
        } = self
        {
            for parameter in &*parameters {
                visible.insert(*parameter, Expression::Parameter);
            }
            visible.insert(*responsible_parameter, Expression::Parameter);
            pureness.enter_function(parameters, *responsible_parameter);

            body.optimize(visible, id_generator, db, tracing, pureness, errors);

            for parameter in &*parameters {
                visible.remove(*parameter);
            }
            visible.remove(*responsible_parameter);
        }

        loop {
            let hashcode_before = self.do_hash();

            reference_following::follow_references(self, visible);
            constant_folding::fold_constants(self, visible, pureness, id_generator);
            inlining::inline_tiny_functions(self, visible, id_generator);
            inlining::inline_functions_containing_use(self, visible, id_generator);
            constant_lifting::lift_constants(self, pureness, id_generator);

            if self.do_hash() == hashcode_before {
                break;
            }
        }

        // TODO: If this is a call to the `needs` function with `True` as the
        // first argument, optimize it away. This is not correct – calling
        // `needs True 3 4` should panic instead. But we figured this is
        // temporarily fine until we have data flow.
        if let Expression::Call { function, arguments, .. } = self
            && let Expression::Function { original_hirs, .. } = visible.get(*function)
            && original_hirs.contains(&hir::Id::needs())
            && arguments.len() == 3
            && let Expression::Tag { symbol, value: None  } = visible.get(arguments[0])
            && symbol == "True" {
            *self = Expression::nothing();
        }
    }

    fn do_hash(&self) -> u64 {
        let mut hasher = FxHasher::default();
        self.hash(&mut hasher);
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
