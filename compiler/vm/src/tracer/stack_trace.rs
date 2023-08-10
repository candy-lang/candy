use super::Tracer;
use crate::heap::{DisplayWithSymbolTable, Heap, HirId, InlineObject, SymbolTable};
use candy_frontend::{ast_to_hir::AstToHir, cst::CstKind, position::PositionConversionDb};
use itertools::Itertools;
use pad::PadStr;

#[derive(Debug, Default)]
pub struct StackTracer {
    pub call_stack: Vec<Call>,
}

// Stack traces are a reduced view of the tracing state that represent the stack
// trace at a given moment in time.

#[derive(Clone, Debug)]
pub struct Call {
    pub call_site: HirId,
    pub callee: InlineObject,
    pub arguments: Vec<InlineObject>,
    pub responsible: HirId,
}
impl Call {
    pub fn dup(&self, heap: &mut Heap) {
        self.call_site.dup(heap);
        self.callee.dup(heap);
        for argument in &self.arguments {
            argument.dup(heap);
        }
        self.responsible.dup(heap);
    }
    pub fn drop(&self, heap: &mut Heap) {
        self.call_site.drop(heap);
        self.callee.drop(heap);
        for argument in &self.arguments {
            argument.drop(heap);
        }
        self.responsible.drop(heap);
    }
}

impl Tracer for StackTracer {
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
        self.call_stack.push(call);
    }
    fn call_ended(&mut self, heap: &mut Heap, _return_value: InlineObject) {
        self.call_stack.pop().unwrap().drop(heap);
    }
}

impl StackTracer {
    pub fn format<DB>(&self, db: &DB, symbol_table: &SymbolTable) -> String
    where
        DB: AstToHir + PositionConversionDb,
    {
        let mut caller_locations_and_calls = vec![];

        for Call {
            call_site,
            callee,
            arguments,
            ..
        } in self.call_stack.iter().rev()
        {
            let hir_id = call_site.get();
            let module = hir_id.module.clone();
            let cst_id = if module.package.is_tooling() {
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
                .unwrap_or_else(|| "<no location>".to_owned()),
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
                    .unwrap_or_else(|| DisplayWithSymbolTable::to_string(callee, symbol_table)),
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
        CstKind::Identifier(identifier) => Some(ToString::to_string(identifier)),
        CstKind::Parenthesized { inner, .. } => extract_receiver_name(inner),
        CstKind::StructAccess { struct_, key, .. } => {
            let struct_string = extract_receiver_name(struct_)?;
            let key = extract_receiver_name(key)?;
            Some(format!("{struct_string}.{key}"))
        }
        _ => None,
    }
}
