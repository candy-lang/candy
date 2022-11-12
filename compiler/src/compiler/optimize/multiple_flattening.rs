//! Multiple flattening lifts `Expression::Multiple` into the parent body.
//!
//! For convenience reasons, other optimizations such as [module folding] and
//! [inlining] may insert `Expression::Multiple`s in the code. This optimization
//! removes those expressions by inlining their content into the parent body.
//!
//! Here's a before-and-after example:
//!
//! ```mir
//! $0 =        |
//!   $1 = ...  |  $1 = ...
//!   $2 = ...  |  $2 = ...
//!             |  $0 = $2
//! ```
//!
//! [module folding]: super::module_folding
//! [inlining]: super::inlining

use crate::{
    builtin_functions::BuiltinFunction,
    compiler::mir::{Expression, Mir},
};

impl Mir {
    pub fn flatten_multiples(&mut self) {
        // For effiency reasons, flattening multiples operates directly on the
        // body's internal state and is thus defined directly in the MIR module.
        self.body.flatten_multiples();
    }
}

#[test]
fn test_multiple_flattening() {
    // $0 =
    //   $1 = builtinEquals
    //
    // # becomes:
    // $0 = builtinEquals
    // $1 = $0
    let mut mir = Mir::build(|body| {
        body.push_multiple(|body| {
            body.push(Expression::Builtin(BuiltinFunction::Equals));
        });
    });
    mir.flatten_multiples();
    mir.normalize_ids();
    assert_eq!(
        mir,
        Mir::build(|body| {
            let inlined = body.push(Expression::Builtin(BuiltinFunction::Equals));
            body.push(Expression::Reference(inlined));
        }),
    );
}
