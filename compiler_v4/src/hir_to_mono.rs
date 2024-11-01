use crate::{
    ast_to_hir::TypeUnifier,
    hir::{self, Hir, NamedType, ParameterType, Type},
    id::IdGenerator,
    mono::{self, Mono},
    type_solver::{goals::SolverSolution, values::SolverVariable},
    utils::HashMapExtension,
};
use itertools::Itertools;
use rustc_hash::FxHashMap;
use std::{collections::hash_map::Entry, mem};

pub fn hir_to_mono(hir: &Hir) -> Mono {
    Context::lower(hir)
}

struct Context<'h> {
    hir: &'h Hir,
    type_declarations: FxHashMap<Box<str>, Option<mono::TypeDeclaration>>,
    assignments: FxHashMap<Box<str>, Option<mono::Assignment>>,
    assignment_initialization_order: Vec<Box<str>>,
    functions: FxHashMap<Box<str>, Option<mono::Function>>,
}
impl<'h> Context<'h> {
    #[must_use]
    fn lower(hir: &'h Hir) -> Mono {
        let mut context = Self {
            hir,
            type_declarations: FxHashMap::default(),
            assignments: FxHashMap::default(),
            assignment_initialization_order: vec![],
            functions: FxHashMap::default(),
        };
        let main_function = context.lower_function(hir.main_function_id, &FxHashMap::default());
        Mono {
            type_declarations: context
                .type_declarations
                .into_iter()
                .map(|(name, declaration)| (name, declaration.unwrap()))
                .collect(),
            assignments: context
                .assignments
                .into_iter()
                .map(|(name, assignment)| (name, assignment.unwrap()))
                .collect(),
            assignment_initialization_order: context
                .assignment_initialization_order
                .into_boxed_slice(),
            functions: context
                .functions
                .into_iter()
                .map(|(name, function)| (name, function.unwrap()))
                .collect(),
            main_function,
        }
    }

    fn lower_assignment(&mut self, id: hir::Id) -> Box<str> {
        let assignment = &self.hir.assignments[&id];
        let name = assignment.name.to_string().into_boxed_str();
        match self.assignments.entry(name.clone()) {
            Entry::Occupied(_) => return name.clone(),
            Entry::Vacant(entry) => entry.insert(None),
        };

        let type_ = self.lower_type(&assignment.type_);
        let (_, body) = BodyBuilder::build(self, &FxHashMap::default(), |builder| {
            builder.lower_expressions(&assignment.body.expressions);
        });
        let assignment = mono::Assignment { type_, body };
        *self.assignments.get_mut(&name).unwrap() = Some(assignment);
        self.assignment_initialization_order.push(name.clone());
        name.clone()
    }

    fn lower_function(
        &mut self,
        id: hir::Id,
        substitutions: &FxHashMap<ParameterType, Type>,
    ) -> Box<str> {
        let function = self.hir.functions.get(&id).unwrap_or_else(|| {
            let (trait_, function) = self
                .hir
                .traits
                .iter()
                .find_map(|(trait_, trait_definition)| {
                    trait_definition
                        .functions
                        .get(&id)
                        .map(|function| (trait_, function))
                })
                .unwrap();
            let self_type = function
                .signature
                .parameters
                .first()
                .unwrap()
                .type_
                .substitute(substitutions);
            let impl_ = self
                .hir
                .impls
                .iter()
                .find(|impl_| {
                    if &impl_.trait_.name != trait_ {
                        return false;
                    }

                    let mut unifier = TypeUnifier::new(&impl_.type_parameters);
                    if unifier.unify(&self_type, &impl_.type_) != Ok(true) {
                        return false;
                    }

                    let trait_declaration = &self.hir.traits[trait_];

                    let self_goal =
                        substitutions
                            .get(&ParameterType::self_type())
                            .map(|self_type| {
                                trait_declaration
                                    .solver_goal
                                    .substitute_all(&FxHashMap::from_iter([(
                                        SolverVariable::self_(),
                                        self_type.clone().try_into().unwrap(),
                                    )]))
                            });

                    let solver_substitutions = substitutions
                        .iter()
                        .filter_map(|(parameter_type, type_)| try {
                            (
                                SolverVariable::new(parameter_type.clone()),
                                type_.clone().try_into().ok()?,
                            )
                        })
                        .collect();

                    self_goal
                        .iter()
                        .chain(trait_declaration.solver_subgoals.iter())
                        .map(|subgoal| {
                            let solution = self
                                .hir
                                .solver_environment
                                .solve(&subgoal.substitute_all(&solver_substitutions), &[]);
                            match solution {
                                SolverSolution::Unique(solution) => Some(solution.used_rule),
                                SolverSolution::Ambiguous => panic!(),
                                SolverSolution::Impossible => None,
                            }
                        })
                        .collect::<Option<Vec<_>>>()
                        .is_some()
                })
                .unwrap();
            &impl_.functions[&id]
        });

        let name = self.mangle_function(
            &function.signature.name,
            &function
                .signature
                .parameters
                .iter()
                .map(|it| it.type_.substitute(substitutions))
                .collect_vec(),
        );
        match self.functions.entry(name.clone()) {
            Entry::Occupied(_) => return name,
            Entry::Vacant(entry) => entry.insert(None),
        };

        let (parameters, body) = match &function.body {
            hir::BodyOrBuiltin::Body(body) => {
                let (parameters, body) = BodyBuilder::build(self, substitutions, |builder| {
                    builder.add_parameters(&function.signature.parameters);
                    builder.lower_expressions(&body.expressions);
                });
                (parameters, mono::BodyOrBuiltin::Body(body))
            }
            hir::BodyOrBuiltin::Builtin(builtin_function) => {
                let (parameters, _) = BodyBuilder::build(self, substitutions, |builder| {
                    builder.add_parameters(&function.signature.parameters);
                });
                (parameters, mono::BodyOrBuiltin::Builtin(*builtin_function))
            }
        };
        let return_type = function.signature.return_type.substitute(substitutions);
        let function = mono::Function {
            parameters,
            return_type: self.lower_type(&return_type),
            body,
        };
        *self.functions.get_mut(&name).unwrap() = Some(function);
        name
    }
    fn mangle_function(&mut self, name: &str, parameter_types: &[hir::Type]) -> Box<str> {
        let mut result = if name == "main" && parameter_types.is_empty() {
            // Avoid name clash with the main function in C.
            // It would be cleaner to do this in `mono_to_c`, but it's easier to do it here.
            "main$"
        } else {
            name
        }
        .to_string();
        for parameter_type in parameter_types {
            result.push('$');
            result.push_str(&self.lower_type(parameter_type));
        }
        result.into_boxed_str()
    }

    fn lower_type(&mut self, type_: &hir::Type) -> Box<str> {
        let mangled_name = Self::mangle_type(type_);
        let entry = match self.type_declarations.entry(mangled_name.clone()) {
            Entry::Occupied(_) => return mangled_name,
            Entry::Vacant(entry) => entry,
        };

        let declaration = match type_ {
            hir::Type::Named(NamedType {
                name,
                type_arguments,
            }) => {
                let Some(declaration) = &self.hir.type_declarations.get(name) else {
                    // Builtin type
                    return mangled_name;
                };
                match &declaration.kind {
                    hir::TypeDeclarationKind::Struct { fields } => {
                        entry.insert(None);
                        let environment = hir::Type::build_environment(
                            &declaration.type_parameters,
                            type_arguments,
                        );
                        if let Some(fields) = fields.as_ref() {
                            let fields = fields
                                .iter()
                                .map(|(name, type_)| {
                                    (
                                        name.clone(),
                                        self.lower_type(&type_.substitute(&environment)),
                                    )
                                })
                                .collect();
                            mono::TypeDeclaration::Struct { fields }
                        } else {
                            mono::TypeDeclaration::Builtin {
                                name: name.clone(),
                                type_arguments: type_arguments
                                    .iter()
                                    .map(|it| self.lower_type(it))
                                    .collect(),
                            }
                        }
                    }
                    hir::TypeDeclarationKind::Enum { variants } => {
                        entry.insert(None);
                        let environment = hir::Type::build_environment(
                            &declaration.type_parameters,
                            type_arguments,
                        );
                        let variants = variants
                            .iter()
                            .map(|(name, value_type)| mono::EnumVariant {
                                name: name.clone(),
                                value_type: value_type
                                    .as_ref()
                                    .map(|it| self.lower_type(&it.substitute(&environment))),
                            })
                            .collect();
                        mono::TypeDeclaration::Enum { variants }
                    }
                }
            }
            hir::Type::Parameter(parameter_type) => {
                panic!("Type parameter `{parameter_type}` should have been monomorphized.")
            }
            hir::Type::Self_ { base_type } => {
                panic!("Self type (base type: {base_type}) should have been monomorphized.")
            }
            hir::Type::Error => unreachable!(),
        };
        *self.type_declarations.get_mut(&mangled_name).unwrap() = Some(declaration);
        mangled_name
    }
    fn mangle_type(type_: &hir::Type) -> Box<str> {
        let mut result = String::new();
        Self::mangle_type_helper(&mut result, type_);
        result.into_boxed_str()
    }
    fn mangle_type_helper(result: &mut String, type_: &hir::Type) {
        match type_ {
            hir::Type::Named(type_) => {
                result.push_str(&type_.name);
                if !type_.type_arguments.is_empty() {
                    result.push_str("$of$");
                    for type_ in type_.type_arguments.iter() {
                        Self::mangle_type_helper(result, type_);
                        result.push('$');
                    }
                    result.push_str("end$");
                }
            }
            hir::Type::Parameter(parameter_type) => {
                panic!("Type parameter `{parameter_type}` should have been monomorphized.")
            }
            hir::Type::Self_ { base_type } => {
                panic!("Self type (base type: {base_type}) should have been monomorphized.")
            }
            hir::Type::Error => result.push_str("Never"),
        }
    }
}
struct BodyBuilder<'c, 'h> {
    context: &'c mut Context<'h>,
    environment: &'c FxHashMap<hir::ParameterType, hir::Type>,
    parameters: Vec<mono::Parameter>,
    body: mono::Body,
    id_generator: IdGenerator<mono::Id>,
    id_mapping: FxHashMap<hir::Id, mono::Id>,
}
impl<'c, 'h> BodyBuilder<'c, 'h> {
    #[must_use]
    fn build(
        context: &'c mut Context<'h>,
        environment: &'c FxHashMap<hir::ParameterType, hir::Type>,
        fun: impl FnOnce(&mut BodyBuilder),
    ) -> (Box<[mono::Parameter]>, mono::Body) {
        let mut builder = Self {
            context,
            environment,
            parameters: vec![],
            body: mono::Body::default(),
            id_generator: IdGenerator::default(),
            id_mapping: FxHashMap::default(),
        };
        fun(&mut builder);
        (builder.parameters.into_boxed_slice(), builder.body)
    }
    #[must_use]
    fn build_inner(&mut self, fun: impl FnOnce(&mut BodyBuilder)) -> mono::Body {
        let mut builder = BodyBuilder {
            context: self.context,
            environment: self.environment,
            parameters: vec![],
            body: mono::Body::default(),
            id_generator: IdGenerator::default(),
            id_mapping: FxHashMap::default(),
        };
        builder.id_mapping = self.id_mapping.clone();
        builder.id_generator = mem::take(&mut self.id_generator);

        fun(&mut builder);
        assert!(builder.parameters.is_empty());

        self.id_generator = builder.id_generator;
        builder.body
    }

    fn add_parameters(&mut self, parameters: &[hir::Parameter]) {
        for parameter in parameters {
            self.add_parameter(parameter);
        }
    }
    fn add_parameter(&mut self, parameter: &hir::Parameter) {
        let id = self.id_generator.generate();
        self.id_mapping.force_insert(parameter.id, id);
        self.parameters.push(mono::Parameter {
            id,
            name: parameter.name.clone(),
            type_: self
                .context
                .lower_type(&parameter.type_.substitute(self.environment)),
        });
    }

    fn lower_expressions(&mut self, expressions: &[(hir::Id, Option<Box<str>>, hir::Expression)]) {
        for (id, name, expression) in expressions {
            self.lower_expression(*id, name.clone(), expression);
        }
    }
    fn lower_expression(
        &mut self,
        id: hir::Id,
        name: Option<Box<str>>,
        expression: &hir::Expression,
    ) {
        match &expression.kind {
            hir::ExpressionKind::Int(int) => {
                self.push(id, name, mono::ExpressionKind::Int(*int), &expression.type_);
            }
            hir::ExpressionKind::Text(text) => {
                self.push(
                    id,
                    name,
                    mono::ExpressionKind::Text(text.clone()),
                    &expression.type_,
                );
            }
            hir::ExpressionKind::CreateStruct { struct_, fields } => {
                let struct_ = self.context.lower_type(struct_);
                let fields = self.lower_ids(fields);
                self.push(
                    id,
                    name,
                    mono::ExpressionKind::CreateStruct { struct_, fields },
                    &expression.type_,
                );
            }
            hir::ExpressionKind::StructAccess { struct_, field } => {
                let struct_ = self.lower_id(*struct_);
                self.push(
                    id,
                    name,
                    mono::ExpressionKind::StructAccess {
                        struct_,
                        field: field.clone(),
                    },
                    &expression.type_,
                );
            }
            hir::ExpressionKind::CreateEnum {
                enum_,
                variant,
                value,
            } => {
                let enum_ = self.context.lower_type(enum_);
                let value = value.map(|it| self.lower_id(it));
                self.push(
                    id,
                    name,
                    mono::ExpressionKind::CreateEnum {
                        enum_,
                        variant: variant.clone(),
                        value,
                    },
                    &expression.type_,
                );
            }
            hir::ExpressionKind::Reference(referenced_id) => {
                let kind = if let Some(referenced_id) = self.id_mapping.get(referenced_id) {
                    mono::ExpressionKind::LocalReference(*referenced_id)
                } else {
                    mono::ExpressionKind::GlobalAssignmentReference(
                        self.context.lower_assignment(*referenced_id),
                    )
                };
                self.push(id, name, kind, &expression.type_);
            }
            hir::ExpressionKind::Call {
                function,
                substitutions,
                arguments,
            } => {
                let function = self
                    .context
                    .lower_function(*function, &self.merge_substitutions(substitutions));
                let arguments = self.lower_ids(arguments);
                self.push(
                    id,
                    name,
                    mono::ExpressionKind::Call {
                        function,
                        arguments,
                    },
                    &expression.type_,
                );
            }
            hir::ExpressionKind::Switch {
                value,
                enum_,
                cases,
            } => {
                let value = self.lower_id(*value);
                let enum_ = self.context.lower_type(enum_);
                let cases = cases
                    .iter()
                    .map(|case| mono::SwitchCase {
                        variant: case.variant.clone(),
                        value_id: case.value_id.map(|id| self.lower_id(id)),
                        body: BodyBuilder::build_inner(self, |builder| {
                            builder.lower_expressions(&case.body.expressions);
                        }),
                    })
                    .collect();
                self.push(
                    id,
                    name,
                    mono::ExpressionKind::Switch {
                        value,
                        enum_,
                        cases,
                    },
                    &expression.type_,
                );
            }
            hir::ExpressionKind::Error => todo!(),
        }
    }
    fn lower_ids(&mut self, hir_ids: &[hir::Id]) -> Box<[mono::Id]> {
        hir_ids.iter().map(|id| self.lower_id(*id)).collect()
    }
    fn lower_id(&mut self, hir_id: hir::Id) -> mono::Id {
        self.id_mapping.get(&hir_id).copied().unwrap_or_else(|| {
            let name = self.context.lower_assignment(hir_id);
            let assignment = &self.context.hir.assignments[&hir_id];
            self.push(
                None,
                None,
                mono::ExpressionKind::GlobalAssignmentReference(name),
                &assignment.type_,
            )
        })
    }

    fn merge_substitutions(
        &self,
        inner: &FxHashMap<ParameterType, Type>,
    ) -> FxHashMap<ParameterType, Type> {
        inner
            .iter()
            .map(|(key, value)| (key.clone(), self.merge_substitution(value)))
            .collect()
    }
    fn merge_substitution(&self, type_: &hir::Type) -> Type {
        match type_ {
            hir::Type::Named(NamedType {
                name,
                type_arguments,
            }) => hir::Type::Named(NamedType {
                name: name.clone(),
                type_arguments: type_arguments
                    .iter()
                    .map(|it| self.merge_substitution(it))
                    .collect(),
            }),
            hir::Type::Parameter(parameter_type) => self.environment[parameter_type].clone(),
            hir::Type::Self_ { .. } | hir::Type::Error => unreachable!(),
        }
    }

    fn push(
        &mut self,
        hir_id: impl Into<Option<hir::Id>>,
        name: impl Into<Option<Box<str>>>,
        kind: mono::ExpressionKind,
        type_: &hir::Type,
    ) -> mono::Id {
        let id = self.id_generator.generate();
        if let Some(hir_id) = hir_id.into() {
            self.id_mapping.force_insert(hir_id, id);
        }
        let type_ = self.context.lower_type(&type_.substitute(self.environment));
        self.body
            .expressions
            .push((id, name.into(), mono::Expression { kind, type_ }));
        id
    }
}
