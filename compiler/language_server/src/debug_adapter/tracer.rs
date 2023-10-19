use candy_frontend::hir::Id;
use candy_vm::{
    heap::{Heap, HirId, InlineObject},
    tracer::{stack_trace::Call, Tracer},
};

#[derive(Debug, Default)]
pub struct DebugTracer {
    pub root_locals: Vec<(Id, InlineObject)>,
    pub call_stack: Vec<StackFrame>,
}

#[derive(Debug)]
pub struct StackFrame {
    pub call: Call,
    pub locals: Vec<(Id, InlineObject)>,
}
impl StackFrame {
    fn new(call: Call) -> Self {
        Self {
            call,
            locals: vec![],
        }
    }

    fn drop(&self, heap: &mut Heap) {
        self.call.drop(heap);
        self.locals.iter().for_each(|(_, value)| value.drop(heap));
    }
}

impl Tracer for DebugTracer {
    fn value_evaluated(&mut self, heap: &mut Heap, expression: HirId, value: InlineObject) {
        value.dup(heap);
        self.call_stack
            .last_mut()
            .map(|it| &mut it.locals)
            .unwrap_or(&mut self.root_locals)
            .push((expression.get().clone(), value));
    }

    fn call_started(
        &mut self,
        heap: &mut Heap,
        callee: InlineObject,
        arguments: Vec<InlineObject>,
    ) {
        let call = Call { callee, arguments };
        call.dup(heap);
        self.call_stack.push(StackFrame::new(call));
    }
    fn call_ended(&mut self, heap: &mut Heap, _return_value: InlineObject) {
        self.call_stack.pop().unwrap().drop(heap);
    }
}
