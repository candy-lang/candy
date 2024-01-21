use super::pure::PurenessInsights;
use crate::{
    mir::{Body, Expression},
    tracing::CallTracingMode,
    utils::VecRetainIndexed,
};
use bitvec::vec::BitVec;

pub fn remove_unnecessary_call_tracing(
    body: &mut Body,
    pureness: &mut PurenessInsights,
    call_tracing_mode: CallTracingMode,
) {
    if call_tracing_mode != CallTracingMode::OnlyForPanicTraces {
        return;
    }

    let mut keep_expressions: BitVec = BitVec::repeat(true, body.expressions.len());
    // Vec of `(index, only_pure_since_then)`
    let mut trace_call_starts = vec![];
    for (index, (id, expression)) in body.expressions.iter().enumerate() {
        match expression {
            Expression::TraceCallStarts { .. } => trace_call_starts.push((index, true)),
            Expression::TraceCallEnds { .. } => {
                let (start_index, only_pure_since_then) = trace_call_starts.pop().unwrap();
                if only_pure_since_then {
                    keep_expressions.set(start_index, false);
                    keep_expressions.set(index, false);
                }
            }
            _ => {
                if !pureness.pure_definitions().contains(*id) {
                    for (_, only_pure_since_then) in &mut trace_call_starts {
                        *only_pure_since_then = false;
                    }
                }
            }
        }
    }

    for (index, keep) in keep_expressions.iter().enumerate() {
        if !keep {
            pureness.on_remove(body.expressions[index].0);
        }
    }
    body.expressions
        .retain_indexed(|index, _| keep_expressions[index]);
}
