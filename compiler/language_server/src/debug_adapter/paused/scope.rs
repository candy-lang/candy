use super::{utils::FiberIdExtension, variable::VariablesKey, PausedState};
use dap::{
    requests::ScopesArguments,
    responses::ScopesResponse,
    types::{Scope, ScopePresentationhint},
};

impl PausedState {
    pub fn scopes(&mut self, args: ScopesArguments) -> ScopesResponse {
        let stack_frame_key = self.stack_frame_ids.id_to_key(args.frame_id);
        let call = stack_frame_key.get(&self.vm_state.tracer);
        let arguments_scope = Scope {
            name: "Arguments".to_string(),
            presentation_hint: Some(ScopePresentationhint::Arguments),
            variables_reference: self
                .variables_ids
                .key_to_id(VariablesKey::Arguments(stack_frame_key.to_owned())),
            named_variables: Some(call.arguments.len() as i64),
            indexed_variables: Some(0),
            expensive: false,
            // TODO: source information for function
            source: None,
            line: None,
            column: None,
            end_line: None,
            end_column: None,
        };

        // TODO: Show channels

        let fiber = stack_frame_key.fiber_id.get(&self.vm_state.vm);
        let heap_scope = Scope {
            name: "Fiber Heap".to_string(),
            presentation_hint: None,
            variables_reference: self
                .variables_ids
                .key_to_id(VariablesKey::FiberHeap(stack_frame_key.fiber_id)),
            named_variables: Some(fiber.heap.objects_len() as i64),
            indexed_variables: Some(0),
            expensive: false,
            source: None,
            line: None,
            column: None,
            end_line: None,
            end_column: None,
        };

        ScopesResponse {
            scopes: vec![arguments_scope, heap_scope],
        }
    }
}
