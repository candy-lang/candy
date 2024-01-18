use candy_frontend::hir::Id;
use candy_vm::{
    heap::{Heap, HirId, InlineObject},
    tracer::{stack_trace::Call, Tracer},
};

#[derive(Debug, Default)]
pub struct DebugTracer {
    pub root_locals: Vec<(Id, InlineObject)>,
    /// Analogous to [`StackTracer::call_stack`].
    pub call_stack: Vec<Vec<StackFrame>>,
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
            .and_then(|it| it.last_mut())
            .map(|it| &mut it.locals)
            .unwrap_or(&mut self.root_locals)
            .push((expression.get().clone(), value));
    }

    fn call_started(
        &mut self,
        heap: &mut Heap,
        call_site: HirId,
        callee: InlineObject,
        arguments: Vec<InlineObject>,
        responsible: HirId,
    ) {
        let call = Call {
            call_site,
            callee,
            arguments,
            responsible,
        };
        call.dup(heap);
        self.call_stack.push(vec![StackFrame::new(call)]);
    }
    fn call_ended(&mut self, heap: &mut Heap, _return_value: Option<InlineObject>) {
        for call in self.call_stack.pop().unwrap() {
            call.drop(heap);
        }
    }
    fn tail_call(
        &mut self,
        heap: &mut Heap,
        call_site: HirId,
        callee: InlineObject,
        arguments: Vec<InlineObject>,
        responsible: HirId,
    ) {
        let call = Call {
            call_site,
            callee,
            arguments,
            responsible,
        };
        call.dup(heap);
        self.call_stack
            .last_mut()
            .unwrap()
            .push(StackFrame::new(call));
    }
}
