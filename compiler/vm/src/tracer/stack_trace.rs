use super::{FiberId, FiberTracer, Tracer};
use crate::{
    channel::ChannelId,
    heap::{Heap, Pointer},
};
use candy_frontend::{
    ast_to_hir::AstToHir, cst::CstKind, module::Package, position::PositionConversionDb,
};
use itertools::Itertools;
use pad::PadStr;
use rustc_hash::FxHashMap;
use tracing::debug;

// A tracer that makes it possible to display stack traces, including all the
// arguments of called functions.
#[derive(Default)]
pub struct StackTracer {
    stacks: FxHashMap<FiberId, Vec<Call>>,
    children_responsible_for_panic: FxHashMap<FiberId, FiberId>,
}

pub struct FiberStackTracer {
    id: FiberId,
    stack: Vec<Call>,
}

#[derive(Clone)]
pub struct Call {
    pub call_site: Pointer,
    pub callee: Pointer,
    pub arguments: Vec<Pointer>,
    pub responsible: Pointer,
}

impl Tracer for StackTracer {
    type ForFiber = FiberStackTracer;

    fn fiber_created(&mut self, _: FiberId) {}
    fn fiber_done(&mut self, fiber: FiberId) {
        self.stacks.remove(&fiber);
    }
    fn fiber_panicked(&mut self, fiber: FiberId, responsible_child: Option<FiberId>) {
        if let Some(child) = responsible_child {
            self.children_responsible_for_panic.insert(fiber, child);
        }
    }
    fn fiber_canceled(&mut self, fiber: FiberId) {
        self.stacks.remove(&fiber);
    }
    fn fiber_execution_started(&mut self, _: FiberId) {}
    fn fiber_execution_ended(&mut self, _: FiberId) {}
    fn channel_created(&mut self, _: ChannelId) {}

    fn tracer_for_fiber(&mut self, fiber: FiberId) -> Self::ForFiber {
        FiberStackTracer {
            id: fiber,
            stack: vec![],
        }
    }

    fn integrate_fiber_tracer(&mut self, tracer: Self::ForFiber, from: &Heap, to: &mut Heap) {
        let mapping = from.clone_to_other_heap(to);
        self.stacks.insert(
            tracer.id,
            tracer
                .stack
                .into_iter()
                .map(|call| Call {
                    call_site: mapping[&call.call_site],
                    callee: mapping[&call.callee],
                    arguments: call.arguments.iter().map(|arg| mapping[arg]).collect(),
                    responsible: mapping[&call.responsible],
                })
                .collect_vec(),
        );
    }
}

impl FiberTracer for FiberStackTracer {
    fn value_evaluated(&mut self, _: Pointer, _: Pointer, _: &mut Heap) {}
    fn found_fuzzable_closure(&mut self, _: Pointer, _: Pointer, _: &mut Heap) {}

    fn call_started(
        &mut self,
        call_site: Pointer,
        callee: Pointer,
        arguments: Vec<Pointer>,
        responsible: Pointer,
        heap: &mut Heap,
    ) {
        heap.dup(call_site);
        heap.dup(callee);
        for argument in &arguments {
            heap.dup(*argument);
        }
        heap.dup(responsible);

        self.stack.push(Call {
            call_site,
            callee,
            arguments,
            responsible,
        });
    }

    fn call_ended(&mut self, return_value: Pointer, heap: &mut Heap) {
        let Call {
            call_site,
            callee,
            arguments,
            responsible,
        } = self.stack.pop().unwrap();

        heap.drop(call_site);
        heap.drop(callee);
        for argument in arguments {
            heap.drop(argument);
        }
        heap.drop(responsible);
    }
}

impl StackTracer {
    /// When a VM panics, some child fiber might be responsible for that. This
    /// function returns a formatted stack trace spanning all fibers in the
    /// chain from the panicking root fiber until the concrete failing needs.
    pub fn format_panic_stack_trace_to_root_fiber<DB>(&self, db: &DB, heap: &Heap) -> String
    where
        DB: AstToHir + PositionConversionDb,
    {
        let mut panicking_fiber_chain = vec![FiberId::root()];
        while let Some(child) = self
            .children_responsible_for_panic
            .get(panicking_fiber_chain.last().unwrap())
        {
            panicking_fiber_chain.push(*child);
        }

        // debug!("Stack traces: {:?}", stack_traces.keys().collect_vec());
        panicking_fiber_chain
            .into_iter()
            .rev()
            .map(|fiber| match self.stacks.get(&fiber) {
                Some(stack_trace) => self.format_stack_trace(db, heap, stack_trace),
                None => "(there's no stack trace for this fiber)".to_string(),
            })
            .join("\n(fiber boundary)\n")
    }

    pub fn format_stack_trace<DB>(&self, db: &DB, heap: &Heap, stack: &[Call]) -> String
    where
        DB: AstToHir + PositionConversionDb,
    {
        let mut caller_locations_and_calls = vec![];

        for Call {
            call_site,
            callee,
            arguments,
            ..
        } in stack.iter().rev()
        {
            let hir_id = heap.get_hir_id(*call_site);
            let module = hir_id.module.clone();
            let is_tooling = matches!(module.package, Package::Tooling(_));
            let cst_id = if is_tooling {
                None
            } else {
                db.hir_to_cst_id(hir_id.clone())
            };
            let cst = cst_id.map(|id| db.find_cst(module.clone(), id));
            let span = cst.map(|cst| db.range_to_positions(module.clone(), cst.span));
            let caller_location_string = format!(
                "{hir_id} {}",
                span.map(|it| format!(
                    "{}:{} – {}:{}",
                    it.start.line, it.start.character, it.end.line, it.end.character,
                ))
                .unwrap_or_else(|| "<no location>".to_string())
            );
            let call_string = format!(
                "{} {}",
                cst_id
                    .and_then(|id| {
                        let cst = db.find_cst(hir_id.module.clone(), id);
                        match cst.kind {
                            CstKind::Call { receiver, .. } => extract_receiver_name(&receiver),
                            _ => None,
                        }
                    })
                    .unwrap_or_else(|| callee.format(heap)),
                arguments.iter().map(|arg| arg.format(heap)).join(" "),
            );
            caller_locations_and_calls.push((caller_location_string, call_string));
        }

        let longest_location = caller_locations_and_calls
            .iter()
            .map(|(location, _)| location.len())
            .max()
            .unwrap_or_default();

        caller_locations_and_calls
            .into_iter()
            .map(|(location, call)| format!("{} {}", location.pad_to_width(longest_location), call))
            .join("\n")
    }
}

fn extract_receiver_name(cst_kind: &CstKind) -> Option<String> {
    match cst_kind {
        CstKind::TrailingWhitespace { child, .. } => extract_receiver_name(child),
        CstKind::Identifier(identifier) => Some(identifier.to_string()),
        CstKind::Parenthesized { inner, .. } => extract_receiver_name(inner),
        CstKind::StructAccess { struct_, key, .. } => {
            let struct_string = extract_receiver_name(struct_)?;
            let key = extract_receiver_name(key)?;
            Some(format!("{struct_string}.{key}"))
        }
        _ => None,
    }
}
