use super::Tracer;
use crate::heap::{Heap, HirId, InlineObject};
use candy_frontend::{hir::Id, module::Module};
use rustc_hash::FxHashMap;
use std::sync::Arc;

#[derive(Debug)]
pub struct EvaluatedValuesTracer {
    module: Module,
    evaluated_values: FxHashMap<Id, InlineObject>,
}
impl EvaluatedValuesTracer {
    #[must_use]
    pub fn new(module: Module) -> Self {
        Self {
            module,
            evaluated_values: FxHashMap::default(),
        }
    }

    #[must_use]
    pub const fn values(&self) -> &FxHashMap<Id, InlineObject> {
        &self.evaluated_values
    }
}
impl Tracer for EvaluatedValuesTracer {
    fn value_evaluated(&mut self, heap: &mut Heap, expression: HirId, value: InlineObject) {
        let id = expression.get();
        if Arc::unwrap_or_clone(id.module.clone()) != self.module {
            return;
        }

        value.dup(heap);
        self.evaluated_values.insert(id.clone(), value);
    }
}
