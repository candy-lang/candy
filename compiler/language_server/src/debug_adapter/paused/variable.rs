use super::{stack_trace::StackFrameKey, utils::FiberIdExtension, PausedState};
use candy_vm::{
    fiber::FiberId,
    heap::{Data, InlineObject, ObjectInHeap},
};
use dap::{
    requests::VariablesArguments,
    responses::VariablesResponse,
    types::{
        Variable, VariablePresentationHint, VariablePresentationHintAttributes,
        VariablePresentationHintKind,
    },
};
use itertools::Itertools;
use std::hash::Hash;

impl PausedState {
    pub fn variables(
        &mut self,
        args: VariablesArguments,
        supports_variable_type: bool,
    ) -> VariablesResponse {
        let key = self
            .variables_ids
            .id_to_key(args.variables_reference)
            .to_owned();
        let variables = self.key_to_variables(key, supports_variable_type);
        VariablesResponse { variables }
    }
    fn key_to_variables(
        &mut self,
        key: VariablesKey,
        supports_variable_type: bool,
    ) -> Vec<Variable> {
        match &key {
            VariablesKey::Arguments(stack_frame_key) => {
                let arguments = stack_frame_key
                    .get(&self.vm_state.tracer)
                    .arguments
                    .to_owned();
                arguments
                    .iter()
                    .enumerate()
                    .map(|(index, object)| {
                        // TODO: resolve argument name
                        self.create_variable(index.to_string(), *object, supports_variable_type)
                    })
                    .collect()
            }
            VariablesKey::FiberHeap(fiber_id) => {
                let mut variables = fiber_id.get(&self.vm_state.vm).heap.iter().collect_vec();
                variables.sort_by_key(|it| it.address());
                variables
                    .into_iter()
                    .map(|object| {
                        self.create_variable(
                            format!("{:p}", object),
                            object.into(),
                            supports_variable_type,
                        )
                    })
                    .collect()
            }
            VariablesKey::Inner(object) => match Data::from(**object) {
                Data::List(list) => list
                    .items()
                    .iter()
                    .enumerate()
                    .map(|(index, object)| {
                        self.create_variable(index.to_string(), *object, supports_variable_type)
                    })
                    .collect(),
                // TODO: If the struct contains more complex keys, display this as a list of
                // key-value pairs.
                Data::Struct(struct_) => struct_
                    .iter()
                    .map(|(_, key, value)| {
                        self.create_variable(key.to_string(), value, supports_variable_type)
                    })
                    .collect(),
                it => panic!("Tried to get inner variables of {it}."),
            },
        }
    }
    fn create_variable(
        &mut self,
        name: String,
        object: InlineObject,
        supports_variable_type: bool,
    ) -> Variable {
        let data = Data::from(object);

        let presentation_hint_kind = match data {
            Data::Closure(_) | Data::Builtin(_) => VariablePresentationHintKind::Method,
            Data::SendPort(_) | Data::ReceivePort(_) => VariablePresentationHintKind::Event,
            _ => VariablePresentationHintKind::Data,
        };
        let presentation_hint = VariablePresentationHint {
            kind: Some(presentation_hint_kind),
            // TODO: Add `Constant` if applicable
            attributes: Some(vec![
                VariablePresentationHintAttributes::Static,
                VariablePresentationHintAttributes::ReadOnly,
            ]),
            // TODO: Set `Private` by default and `Public` for exported assignments
            visibility: None,
            lazy: Some(false),
        };

        let (inner_variables_object, named_variables, indexed_variables) = match data {
            // TODO: support tag, closure, and ports
            Data::List(list) => (Some(**list), 0, list.len()),
            Data::Struct(struct_) => (Some(**struct_), struct_.len(), 0),
            _ => (None, 0, 0),
        };
        let variables_reference = inner_variables_object
            .map(|object| {
                self.variables_ids
                    .key_to_id(VariablesKey::Inner(ObjectInHeap(object)))
            })
            .unwrap_or_default();

        Variable {
            name,
            value: object.to_string(),
            type_field: if supports_variable_type {
                let kind: &str = data.into();
                Some(kind.to_string())
            } else {
                None
            },
            presentation_hint: Some(presentation_hint),
            evaluate_name: None,
            variables_reference,
            named_variables: Some(named_variables as i64),
            indexed_variables: Some(indexed_variables as i64),
            memory_reference: None, // TODO: support memory reference
        }
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum VariablesKey {
    Arguments(StackFrameKey),
    FiberHeap(FiberId),
    Inner(ObjectInHeap),
}
