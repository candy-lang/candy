use super::pure::PurenessInsights;
use crate::mir::{Body, Expression};
use std::mem;

pub fn simplify_tail_call_tracing(body: &mut Body, pureness: &mut PurenessInsights) {
    let [.., (
        _,
        Expression::TraceCallStarts {
            hir_call: start_hir_call,
            function: start_function,
            arguments: start_arguments,
            responsible: start_responsible,
        },
    ), (
        _,
        Expression::Call {
            function: call_function,
            arguments: call_arguments,
            responsible: call_responsible,
        },
    ), (end_id, Expression::TraceCallEnds { return_value: _ })] = &mut body.expressions[..]
    else {
        return;
    };
    assert_eq!(start_function, call_function);
    assert_eq!(start_arguments, call_arguments);
    assert_eq!(start_responsible, call_responsible);

    let hir_call = *start_hir_call;
    let function = *start_function;
    let responsible = *start_responsible;
    // We're going to replace the trace call starts expression, so we can steal
    // and reuse its argument vec.
    let arguments = mem::take(start_arguments);
    let end_id = *end_id;

    // Replace trace call starts with trace tail call
    let trace_call_starts_index = body.expressions.len() - 3;
    body.expressions[trace_call_starts_index].1 = Expression::TraceTailCall {
        hir_call,
        function,
        arguments,
        responsible,
    };
    // We don't have to inform the [`PurenessInsights`] because
    // [`Expression::TraceTailCall`] and [`Expression::TraceCallStarts`] have
    // the same properties.

    // Remove trace call ends
    body.expressions.pop().unwrap();
    pureness.on_remove(end_id);
    // The trace call ends expression doesn't define any inner IDs we'd have to
    // inform the [`PurenessInsights`] about.
}
