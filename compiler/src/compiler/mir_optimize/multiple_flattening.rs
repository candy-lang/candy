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

use crate::compiler::mir::Mir;

impl Mir {
    pub fn flatten_multiples(&mut self) {
        // For effiency reasons, flattening multiples operates directly on the
        // body's internal state and is thus defined directly in the MIR module.
        self.body.flatten_multiples();
    }
}
