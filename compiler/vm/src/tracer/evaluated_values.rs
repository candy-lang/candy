use super::{FiberEnded, FiberId, FiberTracer, Tracer};
use crate::heap::{Heap, HirId, InlineObject};
use candy_frontend::{hir::Id, module::Module};
use rustc_hash::FxHashMap;

#[derive(Debug)]
pub struct EvaluatedValuesTracer {
    module: Module,
    evaluated_values: Option<FxHashMap<Id, InlineObject>>,
}
impl EvaluatedValuesTracer {
    pub fn new(module: Module) -> Self {
        EvaluatedValuesTracer {
            module,
            evaluated_values: None,
        }
    }

    pub fn values(&self) -> &FxHashMap<Id, InlineObject> {
        self.evaluated_values
            .as_ref()
            .expect("VM didn't finish execution yet.")
    }
}
impl Tracer for EvaluatedValuesTracer {
    type ForFiber = FiberEvaluatedValuesTracer;

    fn root_fiber_created(&mut self) -> Self::ForFiber {
        FiberEvaluatedValuesTracer {
            module: self.module.clone(),
            evaluated_values: FxHashMap::default(),
        }
    }
    fn root_fiber_ended(&mut self, ended: FiberEnded<Self::ForFiber>) {
        assert!(self.evaluated_values.is_none());
        self.evaluated_values = Some(ended.tracer.evaluated_values);
    }
}

#[derive(Debug)]
pub struct FiberEvaluatedValuesTracer {
    module: Module,
    evaluated_values: FxHashMap<Id, InlineObject>,
}
impl FiberTracer for FiberEvaluatedValuesTracer {
    fn child_fiber_created(&mut self, _child: FiberId) -> Self {
        FiberEvaluatedValuesTracer {
            module: self.module.clone(),
            evaluated_values: FxHashMap::default(),
        }
    }
    fn child_fiber_ended(&mut self, mut ended: FiberEnded<Self>) {
        self.evaluated_values
            .extend(ended.tracer.evaluated_values.drain());
    }

    fn value_evaluated(&mut self, heap: &mut Heap, expression: HirId, value: InlineObject) {
        let id = expression.get();
        if id.module != self.module {
            return;
        }

        value.dup(heap);
        self.evaluated_values.insert(id.to_owned(), value);
    }

    fn dup_all_stored_objects(&self, heap: &mut Heap) {
        for value in self.evaluated_values.values() {
            value.dup(heap);
        }
    }
}
