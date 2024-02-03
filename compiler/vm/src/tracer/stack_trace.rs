use super::Tracer;
use crate::heap::{Data, Heap, HirId, InlineObject, ToDebugText};
use candy_frontend::{
    ast_to_hir::AstToHir,
    cst::CstKind,
    format::{MaxLength, Precedence},
    module::PackagesPath,
    position::{PositionConversionDb, RangeOfPosition},
};
use itertools::Itertools;
use pad::PadStr;
use std::{env::current_dir, path::Path, sync::Arc};

#[derive(Debug, Default)]
pub struct StackTracer {
    /// The outer [`Vec`] models the normal call stack.
    ///
    /// Each inner [`Vec`] contains at least one [`Call`]. Multiple calls
    /// correspond to tail calls.
    // PERF: Use something like `Smallvec<[Call; 1]>` to reduce allocations for
    // non-tail calls
    pub call_stack: Vec<Vec<Call>>,
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
        self.call_site.dup();
        self.callee.dup(heap);
        for argument in &self.arguments {
            argument.dup(heap);
        }
        self.responsible.dup();
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
        self.call_stack.push(vec![call]);
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
        self.call_stack.last_mut().unwrap().push(call);
    }
}

impl StackTracer {
    pub fn format<DB>(&self, db: &DB, packages_path: &PackagesPath) -> String
    where
        DB: AstToHir + PositionConversionDb,
    {
        let current_package_path = current_dir().ok(); // current_package.to_path(packages_path).unwrap();
        let caller_locations_and_calls = self
            .call_stack
            .iter()
            .flatten()
            .rev()
            .map(|it| Self::format_call(db, packages_path, current_package_path.as_deref(), it))
            .collect_vec();

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

    fn format_call<DB>(
        db: &DB,
        packages_path: &PackagesPath,
        current_directory: Option<&Path>,
        call: &Call,
    ) -> (String, String)
    where
        DB: AstToHir + PositionConversionDb,
    {
        let Call {
            call_site,
            callee,
            arguments,
            ..
        } = call;

        let hir_id = call_site.get();
        let module = hir_id.module.clone();
        let cst_id = if module.package.is_tooling() {
            None
        } else {
            db.hir_to_cst_id(hir_id)
        };

        let span_string = cst_id.map(|id| {
            let cst = db.find_cst(Arc::unwrap_or_clone(module.clone()), id);
            db.range_to_positions(Arc::unwrap_or_clone(module.clone()), cst.data.span)
                .format()
        });
        #[allow(clippy::map_unwrap_or)]
        let caller_location_string = hir_id
            .module
            .try_to_path(packages_path)
            .map(|path| {
                current_directory
                    .and_then(|it| path.strip_prefix(it).ok())
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .into_owned()
            })
            .map(|path| {
                span_string
                    .as_deref()
                    .map(|span_string| format!("{path}:{span_string}"))
                    .unwrap_or(path)
            })
            .unwrap_or_else(|| {
                span_string
                    .map(|span_string| format!("{hir_id}  {span_string}"))
                    .unwrap_or_else(|| hir_id.to_string())
            });

        let call_string = format!(
            "{} {}",
            cst_id
                .and_then(|id| {
                    let cst = db.find_cst(Arc::unwrap_or_clone(hir_id.module.clone()), id);
                    match cst.kind {
                        CstKind::Call { receiver, .. } => extract_receiver_name(&receiver),
                        _ => None,
                    }
                })
                .unwrap_or_else(|| callee.to_string()),
            arguments
                .iter()
                .map(|it| {
                    if let Data::HirId(id) = (*it).into() {
                        // Only occurs for `needs` calls.
                        id.to_string()
                    } else {
                        it.to_debug_text(Precedence::High, MaxLength::Unlimited)
                    }
                })
                .join(" "),
        );
        (caller_location_string, call_string)
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
