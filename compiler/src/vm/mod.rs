mod builtin_functions;
mod heap;
pub mod tracer;
pub mod use_provider;
mod vm;

pub use heap::{Closure, Heap, Object, Pointer};
pub use vm::{Status, TearDownResult, Vm};
