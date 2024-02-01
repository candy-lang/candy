//! The Low-Level Intermediate Representation (LIR) contains explicit reference
//! counting instructions. Because constants are not reference counted, they are
//! tracked separately from other values.
//!
//! The LIR concerns itself with these concepts:
//!
//! - constants: Values that are entirely known at compile-time.
//! - reference count: When running Candy code in the VM, each non-constant heap
//!   value stores a reference count (a non-negative number).
//! - create: Refers to allocating memory for a value and writing sensible bytes
//!   into that memory. (Most) Candy values live on the heap and are
//!   reference-counted. Initially, the reference count is one.
//! - dup: Increase the reference count by one.
//! - drop: Decrease the reference count by one and free the value if the
//!   reference count is zero.
//! - free: Free the underlying memory and drop contained values.
//!
//! When calling a function, the function is responsible for descreasing the
//! reference count of the arguments by one. The reference counts of captured
//! variables are not changed – they are only dropped once the function itself
//! is freed. The responsibility parameter doesn't need to be dropped – all
//! responsibilities are guaranteed to be constants, so they are not reference
//! counted anyway.

pub use self::{body::*, constant::*, expression::*, id::*};
use crate::rich_ir::{RichIrBuilder, ToRichIr, TokenType};
use enumset::EnumSet;

mod body;
mod constant;
mod expression;
mod id;

// TODO: `impl Hash for Lir`
// TODO: `impl ToRichIr for Lir`
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Lir {
    constants: Constants,
    bodies: Bodies,
}
impl Lir {
    #[must_use]
    pub const fn new(constants: Constants, bodies: Bodies) -> Self {
        Self { constants, bodies }
    }

    #[must_use]
    pub const fn constants(&self) -> &Constants {
        &self.constants
    }
    #[must_use]
    pub const fn bodies(&self) -> &Bodies {
        &self.bodies
    }
}

impl ToRichIr for Lir {
    fn build_rich_ir(&self, builder: &mut RichIrBuilder) {
        builder.push("# Constants", TokenType::Comment, EnumSet::empty());
        builder.push_newline();
        self.constants.build_rich_ir(builder);
        builder.push_newline();
        builder.push_newline();

        builder.push("# Bodies", TokenType::Comment, EnumSet::empty());
        builder.push_newline();
        self.bodies
            .build_rich_ir_with_constants(builder, &self.constants);
    }
}
