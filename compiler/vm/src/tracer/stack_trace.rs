use super::{FiberEnded, FiberEndedReason, FiberId, FiberTracer, Tracer};
use crate::heap::{Heap, HirId, InlineObject};
use candy_frontend::{
    ast_to_hir::AstToHir, cst::CstKind, module::Package, position::PositionConversionDb,
};
use itertools::Itertools;
use pad::PadStr;
use rustc_hash::FxHashMap;
use std::mem;

#[derive(Debug, Default)]
pub struct StackTracer<'h> {
    panic_chain: Option<Vec<Call<'h>>>,
}

#[derive(Debug, Default)]
pub struct FiberStackTracer<'h> {
    pub call_stack: Vec<Call<'h>>,
    panic_chains: FxHashMap<FiberId, Vec<Call<'h>>>,
}

// Stack traces are a reduced view of the tracing state that represent the stack
// trace at a given moment in time.
#[derive(Debug)]
pub struct Call<'h> {
    pub call_site: HirId<'h>,
    pub callee: InlineObject<'h>,
    pub arguments: Vec<InlineObject<'h>>,
    pub responsible: HirId<'h>,
}
impl<'h> Call<'h> {
    fn dup(&self, heap: &mut Heap<'h>) {
        self.call_site.dup();
        self.callee.dup(heap);
        for argument in &self.arguments {
            argument.dup(heap);
        }
        self.responsible.dup();
    }
    fn drop(&self, heap: &mut Heap<'h>) {
        self.call_site.drop(heap);
        self.callee.drop(heap);
        for argument in &self.arguments {
            argument.drop(heap);
        }
        self.responsible.drop(heap);
    }
}

impl<'h> Tracer<'h> for StackTracer<'h> {
    type ForFiber = FiberStackTracer<'h>;

    fn root_fiber_created(&mut self) -> Self::ForFiber {
        FiberStackTracer::default()
    }
    fn root_fiber_ended<'a>(&mut self, mut ended: FiberEnded<'a, 'h, Self::ForFiber>) {
        assert!(self.panic_chain.is_none());

        let FiberEndedReason::Panicked(panic) = ended.reason else { return; };
        self.panic_chain = Some(ended.tracer.take_panic_call_stack(panic.panicked_child));
        ended.tracer.drop(ended.heap);
    }
}

impl<'h> StackTracer<'h> {
    pub fn panic_chain(&self) -> Option<&[Call<'h>]> {
        self.panic_chain.as_deref()
    }

    /// When a VM panics, some child fiber might be responsible for that. This
    /// function returns a formatted stack trace spanning all fibers in the
    /// chain from the panicking root fiber until the concrete failing needs.
    pub fn format_panic_stack_trace_to_root_fiber<DB>(&self, db: &DB) -> String
    where
        DB: AstToHir + PositionConversionDb,
    {
        let panic_chain = self.panic_chain.as_ref().expect("VM didn't panic (yet)");
        self.format_stack_trace(db, panic_chain)
    }

    fn format_stack_trace<DB>(&self, db: &DB, stack: &[Call<'h>]) -> String
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
            let hir_id = call_site.get();
            let module = hir_id.module.clone();
            let is_tooling = matches!(module.package, Package::Tooling(_));
            let cst_id = if is_tooling {
                None
            } else {
                db.hir_to_cst_id(hir_id.clone())
            };
            let cst = cst_id.map(|id| db.find_cst(module.clone(), id));
            let span = cst.map(|cst| db.range_to_positions(module.clone(), cst.data.span));
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
                    .unwrap_or_else(|| format!("{callee}")),
                arguments.iter().map(|arg| format!("{arg:?}")).join(" "),
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

impl<'h> FiberTracer<'h> for FiberStackTracer<'h> {
    fn child_fiber_created(&mut self, _child: FiberId) -> Self {
        FiberStackTracer::default()
    }
    fn child_fiber_ended<'a>(&mut self, mut ended: FiberEnded<'a, 'h, Self>) {
        let FiberEndedReason::Panicked(panic) = ended.reason else { return; };
        self.panic_chains.insert(
            ended.id,
            ended.tracer.take_panic_call_stack(panic.panicked_child),
        );
        ended.tracer.drop(ended.heap);
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
        self.call_stack.push(call);
    }
    fn call_ended(&mut self, heap: &mut Heap<'h>, _return_value: InlineObject<'h>) {
        self.call_stack.pop().unwrap().drop(heap);
    }

    fn dup_all_stored_objects(&self, heap: &mut Heap<'h>) {
        for call in &self.call_stack {
            call.dup(heap);
        }
        for call in self.panic_chains.values().flatten() {
            call.dup(heap);
        }
    }
}

impl<'h> FiberStackTracer<'h> {
    fn take_panic_call_stack(&mut self, panicked_child: Option<FiberId>) -> Vec<Call<'h>> {
        let mut call_stack = mem::take(&mut self.call_stack);
        if let Some(panicked_child) = panicked_child {
            let mut existing_panic_chain = self.panic_chains.remove(&panicked_child).unwrap();
            call_stack.append(&mut existing_panic_chain);
        }
        call_stack
    }
    fn drop(self, heap: &mut Heap<'h>) {
        for call in self.call_stack {
            call.drop(heap);
        }
        for call in self.panic_chains.values().flatten() {
            call.drop(heap);
        }
    }
}
