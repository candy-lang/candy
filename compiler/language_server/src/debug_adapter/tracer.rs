use candy_frontend::hir::Id;
use candy_vm::{
    fiber::FiberId,
    heap::{Heap, HirId, InlineObject},
    tracer::{stack_trace::Call, FiberEnded, FiberTracer, Tracer},
};

#[derive(Debug, Default)]
pub struct DebugTracer;

impl<'h> Tracer<'h> for DebugTracer {
    type ForFiber = FiberDebugTracer<'h>;

    fn root_fiber_created(&mut self) -> Self::ForFiber {
        FiberDebugTracer::default()
    }
    fn root_fiber_ended<'a>(&mut self, ended: FiberEnded<'a, 'h, Self::ForFiber>) {
        ended.tracer.drop(ended.heap);
    }
}

#[derive(Debug)]
pub struct StackFrame<'h> {
    pub call: Call<'h>,
    pub locals: Vec<(Id, InlineObject<'h>)>,
}
impl<'h> StackFrame<'h> {
    fn new(call: Call<'h>) -> Self {
        Self {
            call,
            locals: vec![],
        }
    }

    fn dup(&self, heap: &mut Heap<'h>) {
        self.call.dup(heap);
        self.locals.iter().for_each(|(_, value)| value.dup(heap));
    }
    fn drop(&self, heap: &mut Heap<'h>) {
        self.call.drop(heap);
        self.locals.iter().for_each(|(_, value)| value.drop(heap));
    }
}

#[derive(Debug, Default)]
pub struct FiberDebugTracer<'h> {
    pub root_locals: Vec<(Id, InlineObject<'h>)>,
    pub call_stack: Vec<StackFrame<'h>>,
}
impl<'h> FiberTracer<'h> for FiberDebugTracer<'h> {
    fn child_fiber_created(&mut self, _child: FiberId) -> Self {
        FiberDebugTracer::default()
    }
    fn child_fiber_ended<'a>(&mut self, ended: FiberEnded<'a, 'h, Self>) {
        ended.tracer.drop(ended.heap);
    }

    fn value_evaluated(
        &mut self,
        heap: &mut Heap<'h>,
        expression: HirId<'h>,
        value: InlineObject<'h>,
    ) {
        value.dup(heap);
        self.call_stack
            .last_mut()
            .map(|it| &mut it.locals)
            .unwrap_or(&mut self.root_locals)
            .push((expression.get().to_owned(), value));
    }

    fn call_started(
        &mut self,
        heap: &mut Heap<'h>,
        call_site: HirId<'h>,
        callee: InlineObject<'h>,
        arguments: Vec<InlineObject<'h>>,
        responsible: HirId<'h>,
    ) {
        let call = Call {
            call_site,
            callee,
            arguments,
            responsible,
        };
        call.dup(heap);
        self.call_stack.push(StackFrame::new(call));
    }
    fn call_ended(&mut self, heap: &mut Heap<'h>, _return_value: InlineObject<'h>) {
        self.call_stack.pop().unwrap().drop(heap);
    }

    fn dup_all_stored_objects(&self, heap: &mut Heap<'h>) {
        self.root_locals
            .iter()
            .for_each(|(_, value)| value.dup(heap));
        for frame in &self.call_stack {
            frame.dup(heap);
        }
    }
}

impl<'h> FiberDebugTracer<'h> {
    fn drop(self, heap: &mut Heap<'h>) {
        self.root_locals
            .into_iter()
            .for_each(|(_, value)| value.drop(heap));
        for frame in self.call_stack {
            frame.drop(heap);
        }
    }
}
