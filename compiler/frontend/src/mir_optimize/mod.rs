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

mod cleanup;
mod common_subtree_elimination;
mod complexity;
mod constant_folding;
mod constant_lifting;
mod inlining;
mod module_folding;
mod reference_following;
mod tree_shaking;
mod utils;
mod validate;

use super::{hir, hir_to_mir::HirToMir, mir::Mir, tracing::TracingConfig};
use crate::{
    error::CompilerError,
    hir_to_mir::MirResult,
    id::IdGenerator,
    mir::{Body, Expression, Id, MirError, VisibleExpressions},
    module::Module,
};
use rustc_hash::{FxHashSet, FxHasher};
use std::{
    hash::{Hash, Hasher},
    mem,
    sync::Arc,
};
use tracing::debug;

#[salsa::query_group(OptimizeMirStorage)]
pub trait OptimizeMir: HirToMir {
    #[salsa::cycle(recover_from_cycle)]
    fn optimized_mir(&self, module: Module, tracing: TracingConfig) -> MirResult;
}

fn optimized_mir(db: &dyn OptimizeMir, module: Module, tracing: TracingConfig) -> MirResult {
    debug!("{module}: Compiling.");
    let (mir, errors) = db.mir(module.clone(), tracing.clone())?;
    let mut mir = (*mir).clone();
    let mut errors = (*errors).clone();

    let complexity_before = mir.complexity();
    mir.optimize(db, &tracing, &mut errors);
    let complexity_after = mir.complexity();

    debug!("{module}: Done. Optimized from {complexity_before} to {complexity_after}");
    Ok((Arc::new(mir), Arc::new(errors)))
}

impl Mir {
    pub fn optimize(
        &mut self,
        db: &dyn OptimizeMir,
        tracing: &TracingConfig,
        errors: &mut FxHashSet<CompilerError>,
    ) {
        self.body.optimize(
            &mut VisibleExpressions::none_visible(),
            &mut self.id_generator,
            db,
            tracing,
            errors,
        );
        if cfg!(debug_assertions) {
            self.validate();
        }
        self.cleanup();
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
        errors: &mut FxHashSet<CompilerError>,
    ) {
        let mut index = 0;
        'expression_loop: while index < self.expressions.len() {
            let id = self.expressions[index].0;
            let mut expression =
                mem::replace(&mut self.expressions[index].1, Expression::Parameter);

            // Thoroughly optimize the expression.
            expression.optimize(visible, id_generator, db, tracing, errors);
            if cfg!(debug_assertions) {
                expression.validate(visible);
            }

            if self.fold_multiple(id, &mut expression, index).is_some() {
                // We replaced the expression with other expressions, so instead
                // of continuing to the next expression, we should try to
                // optimize the newly inserted expressions next.
                continue 'expression_loop;
            }

            module_folding::apply(&mut expression, visible, id_generator, db, tracing, errors);
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

        common_subtree_elimination::eliminate_common_subtrees(self);
        tree_shaking::tree_shake(self);
        reference_following::remove_redundant_return_references(self);
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
        errors: &mut FxHashSet<CompilerError>,
    ) {
        loop {
            let hashcode_before = self.do_hash();

            reference_following::follow_references(self, visible);
            constant_folding::fold_constants(self, visible, id_generator);
            inlining::inline_tiny_functions(self, visible, id_generator);
            inlining::inline_functions_containing_use(self, visible, id_generator);
            constant_lifting::lift_constants(self, id_generator);

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

                body.optimize(visible, id_generator, db, tracing, errors);

                for parameter in &*parameters {
                    visible.remove(*parameter);
                }
                visible.remove(*responsible_parameter);
            }

            if self.do_hash() == hashcode_before {
                return;
            }
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
) -> MirResult {
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

    Ok((Arc::new(mir), Arc::new(FxHashSet::from_iter([error]))))
}
