use super::{utils::FiberIdExtension, variable::VariablesKey, PausedState};
use dap::{
    requests::ScopesArguments,
    responses::ScopesResponse,
    types::{Scope, ScopePresentationhint},
};

impl PausedState {
    pub fn scopes(&mut self, args: ScopesArguments) -> ScopesResponse {
        let stack_frame_key = self.stack_frame_ids.id_to_key(args.frame_id);
        let stack_frame = stack_frame_key.get(&self.vm_state.tracer);

        let mut scopes = vec![];
        if let Some(stack_frame) = stack_frame {
            scopes.push(Scope {
                name: "Arguments".to_string(),
                presentation_hint: Some(ScopePresentationhint::Arguments),
                variables_reference: self
                    .variables_ids
                    .key_to_id(VariablesKey::Arguments(stack_frame_key.to_owned())),
                named_variables: Some(stack_frame.call.arguments.len() as i64),
                indexed_variables: Some(0),
                expensive: false,
                // TODO: source information for function
                source: None,
                line: None,
                column: None,
                end_line: None,
                end_column: None,
            });
        }
        let locals = stack_frame_key.get_locals(&self.vm_state.tracer);
        scopes.push(Scope {
            name: "Locals".to_string(),
            presentation_hint: Some(ScopePresentationhint::Locals),
            variables_reference: self
                .variables_ids
                .key_to_id(VariablesKey::Locals(stack_frame_key.to_owned())),
            named_variables: Some(locals.len() as i64),
            indexed_variables: Some(0),
            expensive: false,
            // TODO: source information for function
            source: None,
            line: None,
            column: None,
            end_line: None,
            end_column: None,
        });

        // TODO: Show channels

        let fiber = stack_frame_key.fiber_id.get(&self.vm_state.vm);
        scopes.push(Scope {
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
        });

        ScopesResponse { scopes }
    }
}
