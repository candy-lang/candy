use super::{variable::VariablesKey, PausedState};
use dap::{
    requests::ScopesArguments,
    responses::ScopesResponse,
    types::{Scope, ScopePresentationhint},
};

impl PausedState {
    pub fn scopes(&mut self, args: &ScopesArguments) -> ScopesResponse {
        let stack_frame_key = self
            .stack_frame_ids
            .id_to_key(args.frame_id.try_into().unwrap());
        let stack_frame = stack_frame_key.get(&self.vm.as_ref().unwrap().vm);

        let mut scopes = vec![];
        if let Some(stack_frame) = stack_frame {
            scopes.push(Scope {
                name: "Arguments".to_string(),
                presentation_hint: Some(ScopePresentationhint::Arguments),
                variables_reference: self
                    .variables_ids
                    .key_to_id(VariablesKey::Arguments(stack_frame_key.clone())),
                named_variables: Some(stack_frame.call.arguments.len()),
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
        let locals = stack_frame_key.get_locals(&self.vm.as_ref().unwrap().vm);
        scopes.push(Scope {
            name: "Locals".to_string(),
            presentation_hint: Some(ScopePresentationhint::Locals),
            variables_reference: self
                .variables_ids
                .key_to_id(VariablesKey::Locals(stack_frame_key.clone())),
            named_variables: Some(locals.len()),
            indexed_variables: Some(0),
            expensive: false,
            // TODO: source information for function
            source: None,
            line: None,
            column: None,
            end_line: None,
            end_column: None,
        });

        scopes.push(Scope {
            name: "Heap".to_string(),
            presentation_hint: None,
            variables_reference: self.variables_ids.key_to_id(VariablesKey::Heap),
            named_variables: Some(self.heap_ref().objects().len()),
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
