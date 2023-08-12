use super::Tracer;
use crate::heap::{Heap, HirId, InlineObject};
use candy_frontend::{hir::Id, module::Module};
use rustc_hash::FxHashMap;

#[derive(Debug)]
pub struct EvaluatedValuesTracer {
    module: Module,
    evaluated_values: FxHashMap<Id, InlineObject>,
}
impl EvaluatedValuesTracer {
    pub fn new(module: Module) -> Self {
        EvaluatedValuesTracer {
            module,
            evaluated_values: FxHashMap::default(),
        }
    }

    pub fn values(&self) -> &FxHashMap<Id, InlineObject> {
        &self.evaluated_values
    }
}
impl Tracer for EvaluatedValuesTracer {
    fn value_evaluated(&mut self, heap: &mut Heap, expression: HirId, value: InlineObject) {
        let id = expression.get();
        if id.module != self.module {
            return;
        }

        value.dup(heap);
        self.evaluated_values.insert(id.to_owned(), value);
    }
}
