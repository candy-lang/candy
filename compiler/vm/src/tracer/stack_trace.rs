use super::{
    full::{FullTracer, StoredFiberEvent, StoredVmEvent},
    FiberId,
};
use crate::heap::Pointer;
use candy_frontend::{
    ast_to_hir::AstToHir, cst::CstKind, module::Package, position::PositionConversionDb,
};
use itertools::Itertools;
use pad::PadStr;
use rustc_hash::FxHashMap;
use tracing::debug;

// Stack traces are a reduced view of the tracing state that represent the stack
// trace at a given moment in time.

#[derive(Clone)]
pub struct Call {
    pub call_site: Pointer,
    pub callee: Pointer,
    pub arguments: Vec<Pointer>,
    pub responsible: Pointer,
}

impl FullTracer {
    pub fn stack_traces(&self) -> FxHashMap<FiberId, Vec<Call>> {
        let mut stacks: FxHashMap<FiberId, Vec<Call>> = FxHashMap::default();
        for timed_event in &self.events {
            let StoredVmEvent::InFiber { fiber, event } = &timed_event.event else { continue; };
            let stack = stacks.entry(*fiber).or_default();
            match event {
                StoredFiberEvent::CallStarted {
                    call_site,
                    callee,
                    arguments,
                    responsible,
                } => {
                    stack.push(Call {
                        call_site: *call_site,
                        callee: *callee,
                        arguments: arguments.clone(),
                        responsible: *responsible,
                    });
                }
                StoredFiberEvent::CallEnded { .. } => {
                    stack.pop().unwrap();
                }
                _ => {}
            }
        }
        stacks
    }
    pub fn format_stack_trace<DB>(&self, db: &DB, stack: &[Call]) -> String
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
            let hir_id = self.heap.get_hir_id(*call_site);
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
                    .unwrap_or_else(|| callee.format(&self.heap)),
                arguments.iter().map(|arg| arg.format(&self.heap)).join(" "),
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
    /// When a VM panics, some child fiber might be responsible for that. This
    /// function returns a formatted stack trace spanning all fibers in the
    /// chain from the panicking root fiber until the concrete failing needs.
    pub fn format_panic_stack_trace_to_root_fiber<DB>(&self, db: &DB) -> String
    where
        DB: AstToHir + PositionConversionDb,
    {
        let mut panicking_fiber_chain = vec![FiberId::root()];
        for timed_event in self.events.iter().rev() {
            if let StoredVmEvent::FiberPanicked {
                fiber,
                panicked_child,
            } = timed_event.event
            {
                if fiber == *panicking_fiber_chain.last().unwrap() {
                    match panicked_child {
                        Some(child) => panicking_fiber_chain.push(child),
                        None => break,
                    }
                }
            }
        }

        let stack_traces = self.stack_traces();
        debug!("Stack traces: {:?}", stack_traces.keys().collect_vec());
        panicking_fiber_chain
            .into_iter()
            .rev()
            .map(|fiber| match stack_traces.get(&fiber) {
                Some(stack_trace) => self.format_stack_trace(db, stack_trace),
                None => "(there's no stack trace for this fiber)".to_string(),
            })
            .join("\n(fiber boundary)\n")
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
