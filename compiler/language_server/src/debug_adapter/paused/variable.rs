use super::{memory::MemoryReference, stack_trace::StackFrameKey, PausedState};
use crate::database::Database;
use candy_frontend::hir::{self, Expression, HirDb};
use candy_vm::heap::{Data, DataDiscriminants, InlineObject, ObjectInHeap, Tag};
use dap::{
    requests::VariablesArguments,
    responses::VariablesResponse,
    types::{
        Variable, VariablePresentationHint, VariablePresentationHintAttributes,
        VariablePresentationHintKind, VariablesArgumentsFilter,
    },
};
use itertools::Itertools;
use rustc_hash::FxHashMap;
use std::hash::Hash;

impl PausedState {
    #[allow(unused_parens)]
    pub fn variables(
        &mut self,
        db: &Database,
        args: &VariablesArguments,
        supports_variable_type: bool,
    ) -> VariablesResponse {
        let should_include_indexed = matches!(
            args.filter,
            (Some(VariablesArgumentsFilter::Indexed) | None),
        );
        let should_include_named =
            matches!(args.filter, (Some(VariablesArgumentsFilter::Named) | None));

        let mut start = args.start.unwrap_or_default();
        let mut count = args
            .count
            .and_then(|it| if it == 0 { None } else { Some(it) })
            .unwrap_or(usize::MAX);

        let key = self
            .variables_ids
            .id_to_key(args.variables_reference.try_into().unwrap())
            .clone();
        let mut variables = vec![];
        match &key {
            VariablesKey::Arguments(stack_frame_key) => {
                let call = &stack_frame_key.get(self.vm_ref()).unwrap().call;
                match Data::from(call.callee) {
                    Data::Function(function) => {
                        if should_include_named {
                            let functions = self
                                .vm
                                .as_ref()
                                .unwrap()
                                .vm
                                .byte_code()
                                .functions_behind(function.body());
                            assert_eq!(functions.len(), 1);
                            let function: &hir::Id = functions.iter().next().unwrap();

                            let Expression::Function(hir::Function { parameters, .. }) =
                                db.find_expression(function.clone()).unwrap()
                            else {
                                panic!("Function's HIR is not a function: {function}");
                            };

                            variables.extend(
                                parameters
                                    .iter()
                                    .map(|it| ToString::to_string(&it.keys.last().unwrap()))
                                    .zip_eq(call.arguments.clone())
                                    .skip(start)
                                    .take(count)
                                    .map(|(parameter, argument)| {
                                        self.create_variable(
                                            parameter,
                                            argument,
                                            supports_variable_type,
                                        )
                                    }),
                            );
                        }
                    }
                    Data::Builtin(_) => {
                        if should_include_indexed {
                            let arguments = call.arguments.clone();
                            variables.extend(
                                arguments[start..].iter().take(count).enumerate().map(
                                    |(index, object)| {
                                        // TODO: resolve argument name
                                        self.create_variable(
                                            ToString::to_string(&(start + index)),
                                            *object,
                                            supports_variable_type,
                                        )
                                    },
                                ),
                            );
                        }
                    }
                    it => panic!("Unexpected callee: {it}"),
                };
            }
            VariablesKey::Locals(stack_frame_key) => {
                let locals = stack_frame_key.get_locals(self.vm_ref());
                if should_include_named && !locals.is_empty() {
                    let body = db.containing_body_of(locals.first().unwrap().0.clone());
                    let locals = locals
                        .iter()
                        .filter_map(|(id, value)| {
                            body.identifiers.get(id).map(|it| (it.as_str(), *value))
                        })
                        .collect_vec();
                    let total_name_counts = locals.iter().map(|(name, _)| *name).counts();

                    let mut name_counts = FxHashMap::<_, usize>::default();
                    let locals = locals
                        .into_iter()
                        .map(|(name, value)| {
                            let count = name_counts
                                .entry(name)
                                .and_modify(|it| *it += 1)
                                .or_default();
                            (name, value, *count)
                        })
                        .skip(start)
                        .take(count)
                        .map(|(name, value, count)| {
                            self.create_variable(
                                if count == *total_name_counts.get(name).unwrap() - 1 {
                                    name.to_string()
                                } else {
                                    format!("{name} v{count}")
                                },
                                value,
                                supports_variable_type,
                            )
                        });
                    variables.extend(locals);
                }
            }
            VariablesKey::Heap => {
                if should_include_named {
                    let mut vars = self.heap_ref().iter().collect_vec();
                    vars.sort_by_key(|it| it.address());
                    variables.extend(vars[start..].iter().take(count).map(|object| {
                        self.create_variable(
                            format!("{:p}", object),
                            (*object).into(),
                            supports_variable_type,
                        )
                    }));
                }
            }
            VariablesKey::Inner(object) => match Data::from(**object) {
                Data::Tag(Tag::Heap(tag)) => {
                    if should_include_named {
                        if start == 0 && count > 0 {
                            variables.push(Variable {
                                name: "Symbol".to_string(),
                                value: tag.symbol().get().to_string(),
                                type_field: if supports_variable_type {
                                    Some("Symbol".to_string())
                                } else {
                                    None
                                },
                                presentation_hint: Some(Self::presentation_hint_for(
                                    DataDiscriminants::Tag,
                                )),
                                evaluate_name: None,
                                variables_reference: 0,
                                named_variables: Some(0),
                                indexed_variables: Some(0),
                                memory_reference: None,
                            });
                        }
                        count = count.saturating_sub(1);

                        if count > 0 {
                            let name = "Value".to_string();
                            variables.push(self.create_variable(
                                name,
                                tag.value(),
                                supports_variable_type,
                            ));
                        }
                    }
                }
                Data::List(list) => {
                    if should_include_named {
                        if start == 0 && count > 0 {
                            variables.push(Self::create_length_variable(
                                list.len(),
                                supports_variable_type,
                            ));
                        }
                        start = start.saturating_sub(1);
                        count = count.saturating_sub(1);
                    }
                    if should_include_indexed {
                        variables.extend(list.items()[start..].iter().take(count).enumerate().map(
                            |(index, object)| {
                                self.create_variable(
                                    ToString::to_string(&(start + index)),
                                    *object,
                                    supports_variable_type,
                                )
                            },
                        ));
                    }
                }
                Data::Struct(struct_) => {
                    // TODO: If the struct contains more complex keys, display
                    // this as a list of key-value pairs.
                    if should_include_named {
                        if start == 0 {
                            variables.push(Self::create_length_variable(
                                struct_.len(),
                                supports_variable_type,
                            ));
                        }
                        start = start.saturating_sub(1);
                        count = count.saturating_sub(1);

                        let fields = struct_
                            .keys()
                            .iter()
                            .copied()
                            .zip_eq(struct_.values().iter().copied())
                            .sorted()
                            .collect_vec();
                        variables.extend(fields.into_iter().skip(start).take(count).map(
                            |(key, value)| {
                                self.create_variable(key.to_string(), value, supports_variable_type)
                            },
                        ));
                    }
                }
                it => panic!("Tried to get inner variables of {it}."),
            },
        }

        VariablesResponse { variables }
    }

    fn create_length_variable(length: usize, supports_variable_type: bool) -> Variable {
        Variable {
            name: "<length>".to_string(),
            value: ToString::to_string(&length),
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
            // TODO: support closure and ports
            Data::Tag(Tag::Heap(tag)) => (Some(*tag), 2, 0),
            // One more field than the length since we add the “<length>” entry.
            Data::List(list) => (Some(**list), 1, list.len()),
            Data::Struct(struct_) => (Some(**struct_), struct_.len() + 1, 0),
            _ => (None, 0, 0),
        };
        let variables_reference = inner_variables_object
            .map(|object| {
                self.variables_ids
                    .key_to_id(VariablesKey::Inner(ObjectInHeap(object)))
                    .get()
            })
            .unwrap_or_default();

        Variable {
            name,
            value: object.to_string(),
            type_field: Self::type_field_for(data.into(), supports_variable_type),
            presentation_hint: Some(Self::presentation_hint_for(data.into())),
            evaluate_name: None,
            variables_reference,
            named_variables: Some(named_variables),
            indexed_variables: Some(indexed_variables),
            memory_reference: Some(MemoryReference::new(object).to_dap()),
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
            DataDiscriminants::Function | DataDiscriminants::Builtin => {
                VariablePresentationHintKind::Method
            }
            DataDiscriminants::Handle => VariablePresentationHintKind::Event,
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
    Locals(StackFrameKey),
    Heap,
    Inner(ObjectInHeap),
}
