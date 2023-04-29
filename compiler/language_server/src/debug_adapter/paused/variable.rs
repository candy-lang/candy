use super::{stack_trace::StackFrameKey, utils::FiberIdExtension, PausedState};
use candy_vm::{
    fiber::FiberId,
    heap::{Data, DataDiscriminants, InlineObject, ObjectInHeap},
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
                Data::List(list) => {
                    let mut variables = vec![Self::create_length_variable(
                        list.len(),
                        supports_variable_type,
                    )];
                    variables.extend(list.items().iter().enumerate().map(|(index, object)| {
                        self.create_variable(index.to_string(), *object, supports_variable_type)
                    }));
                    variables
                }
                Data::Struct(struct_) => {
                    // TODO: If the struct contains more complex keys, display
                    // this as a list of key-value pairs.
                    let mut variables = vec![Self::create_length_variable(
                        struct_.len(),
                        supports_variable_type,
                    )];
                    variables.extend(struct_.iter().map(|(_, key, value)| {
                        self.create_variable(key.to_string(), value, supports_variable_type)
                    }));
                    variables
                }
                it => panic!("Tried to get inner variables of {it}."),
            },
        }
    }
    fn create_length_variable(length: usize, supports_variable_type: bool) -> Variable {
        Variable {
            name: "<length>".to_string(),
            value: length.to_string(),
            type_field: Self::type_field_for(DataDiscriminants::Int, supports_variable_type),
            presentation_hint: Some(Self::presentation_hint_for(DataDiscriminants::Int)),
            evaluate_name: None,
            variables_reference: 0,
            named_variables: Some(0),
            indexed_variables: Some(0),
            memory_reference: None,
        }
    }
    fn create_variable(
        &mut self,
        name: String,
        object: InlineObject,
        supports_variable_type: bool,
    ) -> Variable {
        let data = Data::from(object);

        let (inner_variables_object, named_variables, indexed_variables) = match data {
            // TODO: support tag, closure, and ports
            // One more fields than the length since we add the “<length>” entry.
            Data::List(list) => (Some(**list), 0, list.len() + 1),
            Data::Struct(struct_) => (Some(**struct_), struct_.len() + 1, 0),
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
            type_field: Self::type_field_for(data.into(), supports_variable_type),
            presentation_hint: Some(Self::presentation_hint_for(data.into())),
            evaluate_name: None,
            variables_reference,
            named_variables: Some(named_variables as i64),
            indexed_variables: Some(indexed_variables as i64),
            memory_reference: None, // TODO: support memory reference
        }
    }
    fn type_field_for(kind: DataDiscriminants, supports_variable_type: bool) -> Option<String> {
        if supports_variable_type {
            let kind: &str = kind.into();
            Some(kind.to_string())
        } else {
            None
        }
    }
    fn presentation_hint_for(kind: DataDiscriminants) -> VariablePresentationHint {
        let kind = match kind {
            DataDiscriminants::Closure | DataDiscriminants::Builtin => {
                VariablePresentationHintKind::Method
            }
            DataDiscriminants::SendPort | DataDiscriminants::ReceivePort => {
                VariablePresentationHintKind::Event
            }
            _ => VariablePresentationHintKind::Data,
        };
        VariablePresentationHint {
            kind: Some(kind),
            // TODO: Add `Constant` if applicable
            attributes: Some(vec![
                VariablePresentationHintAttributes::Static,
                VariablePresentationHintAttributes::ReadOnly,
            ]),
            // TODO: Set `Private` by default and `Public` for exported assignments
            visibility: None,
            lazy: Some(false),
        }
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum VariablesKey {
    Arguments(StackFrameKey),
    FiberHeap(FiberId),
    Inner(ObjectInHeap),
}
