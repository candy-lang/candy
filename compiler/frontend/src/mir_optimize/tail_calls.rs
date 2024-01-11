use crate::mir::{Body, Expression, Mir};
use std::mem;

pub fn simplify_tail_call_tracing(mir: &mut Mir) {
    // Since this runs as a final pass over the MIR after all other
    // optimizations, we don't have to update any [`PurenessInsights`].
    visit_body(&mut mir.body);
}

fn visit_body(body: &mut Body) {
    for (_, expression) in &mut body.expressions {
        if let Expression::Function { body, .. } = expression {
            visit_body(body);
            simplify_body(body);
        }
    }
}
fn simplify_body(body: &mut Body) {
    let [.., (call_id, Expression::Call { .. }), (
        _,
        Expression::TraceCallEnds {
            return_value: end_return_value,
        },
    ), (_, Expression::Reference(reference_target))] = &mut body.expressions[..]
    else {
        return;
    };

    if call_id != end_return_value || end_return_value != reference_target {
        // There's a call at the end of the function, but we return something
        // else. This does not form a tail call.
        return;
    }

    // Remove the trace call ends and reference
    body.expressions.truncate(body.expressions.len() - 2);

    // Find the matching trace call starts
    let mut nesting = 0;
    let mut trace_call_starts = None;
    for (index, (_, expression)) in body.expressions.iter_mut().enumerate().rev().skip(1) {
        match expression {
            Expression::TraceCallStarts {
                hir_call,
                function,
                arguments,
                responsible,
            } => {
                if nesting > 0 {
                    nesting -= 1;
                    continue;
                }

                trace_call_starts = Some((
                    index,
                    *hir_call,
                    *function,
                    mem::take(arguments),
                    *responsible,
                ));
                break;
            }
            Expression::TraceCallEnds { .. } => nesting += 1,
            _ => {}
        }
    }
    let (index, hir_call, function, arguments, responsible) = trace_call_starts.unwrap();

    body.expressions[index].1 = Expression::TraceTailCall {
        hir_call,
        function,
        arguments,
        responsible,
    };
}
