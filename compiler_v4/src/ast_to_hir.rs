use crate::{
    ast::{
        Ast, AstArguments, AstAssignment, AstBody, AstCall, AstDeclaration, AstEnum, AstExpression,
        AstFunction, AstImpl, AstParameter, AstResult, AstStatement, AstString, AstStruct,
        AstSwitch, AstTextPart, AstTrait, AstType, AstTypeArguments, AstTypeParameter,
        AstTypeParameters,
    },
    error::CompilerError,
    hir::{
        self, Assignment, Body, BodyOrBuiltin, BuiltinFunction, Expression, ExpressionKind,
        Function, FunctionSignature, Hir, Id, Impl, NamedType, Parameter, ParameterType,
        SliceOfTypeParameter, SwitchCase, Trait, TraitDefinition, TraitFunction, Type,
        TypeDeclaration, TypeDeclarationKind, TypeParameter,
    },
    id::IdGenerator,
    position::Offset,
    type_solver::{
        goals::{Environment, SolverGoal, SolverRule, SolverSolution},
        values::{SolverValue, SolverVariable},
    },
    utils::HashMapExtension,
};
use itertools::{Itertools, Position};
use petgraph::{
    algo::toposort,
    graph::{DiGraph, NodeIndex},
};
use rustc_hash::{FxHashMap, FxHashSet};
use std::{collections::hash_map::Entry, iter, mem, ops::Range, path::Path};
use strum::VariantArray;

pub fn ast_to_hir(path: &Path, ast: &Ast) -> (Hir, Vec<CompilerError>) {
    let mut context = Context::new(path, ast);
    context.add_builtin_functions();
    context.lower_declarations();
    context.into_hir()
}

#[derive(Debug)]
struct Context<'a> {
    path: &'a Path,
    ast: &'a Ast,
    id_generator: IdGenerator<Id>,
    // TODO: merge structs and enums into this map, but split in final HIR
    traits: FxHashMap<Box<str>, TraitDeclaration<'a>>,
    impls: Vec<ImplDeclaration<'a>>,
    environment: Environment,
    global_identifiers: FxHashMap<Box<str>, Named>,
    assignments: FxHashMap<Id, AssignmentDeclaration<'a>>,
    assignment_dependency_graph: DiGraph<Id, ()>,
    functions: FxHashMap<Id, FunctionDeclaration<'a>>,
    errors: Vec<CompilerError>,
    hir: Hir,
}
#[derive(Clone, Debug)]
struct TraitDeclaration<'a> {
    type_parameters: Box<[TypeParameter]>,
    solver_goal: SolverGoal,
    solver_subgoals: Box<[SolverGoal]>,
    functions: FxHashMap<Id, FunctionDeclaration<'a>>,
}
impl<'a> TraitDeclaration<'a> {
    #[must_use]
    fn into_definition(self) -> TraitDefinition {
        TraitDefinition {
            type_parameters: self.type_parameters,
            functions: self
                .functions
                .into_iter()
                .map(|(id, function)| (id, function.into_trait_function()))
                .collect(),
        }
    }
}
#[derive(Clone, Debug)]
struct ImplDeclaration<'a> {
    type_parameters: Box<[TypeParameter]>,
    type_: Type,
    self_type: NamedType,
    trait_: hir::Result<Trait>,
    functions: FxHashMap<Id, FunctionDeclaration<'a>>,
}
impl<'a> ImplDeclaration<'a> {
    #[must_use]
    fn into_impl(self) -> Impl {
        Impl {
            type_parameters: self.type_parameters,
            type_: self.type_,
            trait_: self.trait_,
            functions: self
                .functions
                .into_iter()
                .map(|(id, function)| (id, function.into_function()))
                .collect(),
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
enum Named {
    Assignment(Id),
    Functions(Vec<Id>),
}
#[derive(Debug)]
struct AssignmentDeclaration<'a> {
    ast: &'a AstAssignment,
    type_: Type,
    graph_index: NodeIndex,
    body: Option<Body>,
}
#[derive(Clone, Debug)]
struct FunctionDeclaration<'a> {
    ast: Option<&'a AstFunction>,
    name: Box<str>,
    type_parameters: Box<[TypeParameter]>,
    parameters: Box<[Parameter]>,
    return_type: Type,
    body: Option<BodyOrBuiltin>,
}
impl<'a> FunctionDeclaration<'a> {
    fn signature_to_string(&self) -> String {
        format!(
            "{}{}({})",
            self.name,
            if self.type_parameters.is_empty() {
                String::new()
            } else {
                format!(
                    "[{}]",
                    self.type_parameters.iter().map(|it| &it.name).join(", ")
                )
            },
            self.parameters
                .iter()
                .map(|it| format!("{}: {}", it.name, it.type_))
                .join(", "),
        )
    }

    fn call_signature_to_string(function_name: &str, argument_types: &[Type]) -> String {
        format!("{}({})", function_name, argument_types.iter().join(", "))
    }

    #[must_use]
    fn into_trait_function(mut self) -> TraitFunction {
        let body = mem::take(&mut self.body);
        TraitFunction {
            signature: self.into_function_signature(),
            body,
        }
    }
    #[must_use]
    fn into_function(mut self) -> Function {
        let body = mem::take(&mut self.body).unwrap();
        Function {
            signature: self.into_function_signature(),
            body,
        }
    }
    #[must_use]
    fn into_function_signature(self) -> FunctionSignature {
        FunctionSignature {
            name: self.name,
            type_parameters: self.type_parameters,
            parameters: self.parameters,
            return_type: self.return_type,
        }
    }
}

impl<'a> Context<'a> {
    fn new(path: &'a Path, ast: &'a Ast) -> Self {
        Self {
            path,
            ast,
            id_generator: IdGenerator::start_at(BuiltinFunction::VARIANTS.len()),
            traits: FxHashMap::default(),
            impls: vec![],
            // Placeholder until `lower_declarations(…)` runs:
            environment: Environment { rules: vec![] },
            global_identifiers: FxHashMap::default(),
            assignments: FxHashMap::default(),
            assignment_dependency_graph: DiGraph::new(),
            functions: FxHashMap::default(),
            errors: vec![],
            hir: Hir::default(),
        }
    }

    fn into_hir(mut self) -> (Hir, Vec<CompilerError>) {
        self.hir.main_function_id = self.find_main_function().unwrap_or_default();

        if let Err(cycle) = toposort(&self.assignment_dependency_graph, None) {
            let id = *self
                .assignment_dependency_graph
                .node_weight(cycle.node_id())
                .unwrap();
            self.add_error(
                // TODO: report actual error location
                Offset(0)..Offset(0),
                // TODO: print full cycle
                format!(
                    "Cycle in global assignments including `{}`",
                    self.global_identifiers
                        .iter()
                        .find(|(_, named)| *named == &Named::Assignment(id))
                        .unwrap()
                        .0,
                ),
            );
        }

        self.hir.traits = self
            .traits
            .into_iter()
            .map(|(name, trait_)| (name, trait_.into_definition()))
            .collect();
        self.hir.impls = self
            .impls
            .into_iter()
            .map(ImplDeclaration::into_impl)
            .collect();

        let mut assignments = FxHashMap::default();
        let mut functions = FxHashMap::default();
        for (name, named) in self.global_identifiers {
            match named {
                Named::Assignment(id) => {
                    let AssignmentDeclaration { type_, body, .. } =
                        self.assignments.remove(&id).unwrap();
                    assignments.force_insert(
                        id,
                        Assignment {
                            name: name.clone(),
                            type_,
                            body: body.unwrap(),
                        },
                    );
                }
                Named::Functions(ids) => {
                    for id in ids {
                        functions.force_insert(
                            id,
                            self.functions
                                .remove(&id)
                                .unwrap_or_else(|| panic!("Missing {id}"))
                                .into_function(),
                        );
                    }
                }
            };
        }
        self.hir.assignments = assignments;
        self.hir.functions = functions;

        (self.hir, self.errors)
    }
    fn find_main_function(&mut self) -> Option<Id> {
        if let Some(named) = self.global_identifiers.get("main") {
            match named {
                Named::Assignment(assignment) => {
                    let span = self.assignments[assignment]
                        .ast
                        .name
                        .value()
                        .unwrap()
                        .span
                        .clone();
                    self.add_error(span, "`main` must be a function");
                    None
                }
                Named::Functions(ids) => {
                    assert!(!ids.is_empty());

                    let function = &self.functions[ids.first().unwrap()];
                    if ids.len() > 1 {
                        self.add_error(
                            function.ast.unwrap().name.value().unwrap().span.clone(),
                            "Main function may not be overloaded",
                        );
                        None
                    } else if !function.parameters.is_empty() {
                        self.add_error(
                            function.ast.unwrap().name.value().unwrap().span.clone(),
                            "Main function must not have parameters",
                        );
                        None
                    } else if function.return_type != Type::Error
                        && function.return_type != NamedType::int().into()
                    {
                        self.add_error(
                            function.ast.unwrap().name.value().unwrap().span.clone(),
                            "Main function must return an Int",
                        );
                        None
                    } else {
                        Some(ids[0])
                    }
                }
            }
        } else {
            self.add_error(Offset(0)..Offset(0), "Program is missing a main function");
            None
        }
    }

    fn add_builtin_functions(&mut self) {
        for builtin_function in BuiltinFunction::VARIANTS {
            let id = builtin_function.id();
            let signature = builtin_function.signature();
            let type_parameters = signature
                .type_parameters
                .into_vec()
                .into_iter()
                .map(|name| TypeParameter {
                    name,
                    upper_bound: None,
                })
                .collect::<Box<_>>();
            let parameters = signature
                .parameters
                .into_vec()
                .into_iter()
                .map(|(name, type_)| Parameter {
                    id: self.id_generator.generate(),
                    name,
                    type_,
                })
                .collect::<Box<_>>();
            self.functions.force_insert(
                id,
                FunctionDeclaration {
                    ast: None,
                    name: signature.name.clone(),
                    type_parameters,
                    parameters,
                    return_type: signature.return_type,
                    body: Some(BodyOrBuiltin::Builtin(*builtin_function)),
                },
            );
            self.global_identifiers
                .force_insert(signature.name, Named::Functions(vec![id]));
        }
    }

    fn lower_declarations(&mut self) {
        // TODO: remove these vecs?
        let mut assignments_to_lower = vec![];
        let mut functions_to_lower = vec![];
        for declaration in self.ast {
            let AstDeclaration::Trait(trait_) = declaration else {
                continue;
            };
            self.lower_trait_definition(trait_);
        }
        for declaration in self.ast {
            match declaration {
                AstDeclaration::Struct(struct_) => self.lower_struct(struct_),
                AstDeclaration::Enum(enum_) => self.lower_enum(enum_),
                AstDeclaration::Trait(_) => {}
                AstDeclaration::Impl(impl_) => self.lower_impl(impl_),
                AstDeclaration::Assignment(assignment) => {
                    if let Some(id) = self.lower_assignment_signature(assignment) {
                        assignments_to_lower.push(id);
                    }
                }
                AstDeclaration::Function(function) => {
                    if let Some((id, function)) =
                        self.lower_top_level_function_signature(&[], None, function)
                    {
                        self.functions.force_insert(id, function);
                        functions_to_lower.push(id);
                    }
                }
            }
        }

        println!("{}", self.environment);
        // let solution = self.find_unique_solver_solution_for(
        //     &NamedType {
        //         name: "List".into(),
        //         type_arguments: vec![NamedType {
        //             name: "Int".into(),
        //             type_arguments: Box::default(),
        //         }
        //         .into()]
        //         .into_boxed_slice(),
        //     }
        //     .into(),
        //     &NamedType {
        //         name: "Equal".into(),
        //         type_arguments: Box::default(),
        //     }
        //     .into(),
        // );
        // if let Some(solution) = solution {
        //     println!("{solution}");
        // } else {
        //     println!("no solution");
        // }

        for trait_name in self.traits.keys().cloned().collect_vec() {
            let trait_ = &self.traits[&trait_name];
            let self_type = NamedType {
                name: trait_name.clone(),
                type_arguments: trait_.type_parameters.type_(),
            };
            for (id, mut function) in trait_.functions.clone() {
                self.lower_function(Some(&self_type), &mut function, true);
                self.traits
                    .get_mut(&trait_name)
                    .unwrap()
                    .functions
                    .insert(id, function)
                    .unwrap();
            }
        }
        for index in 0..self.impls.len() {
            let impl_ = &self.impls[index];
            let self_type = impl_.self_type.clone();
            for (id, mut function) in impl_.functions.clone() {
                self.lower_function(Some(&self_type), &mut function, false);
                self.impls[index].functions.insert(id, function).unwrap();
            }
        }
        for id in assignments_to_lower {
            self.lower_assignment(id);
        }
        for id in functions_to_lower {
            let mut function = self.functions.get(&id).unwrap().clone();
            self.lower_function(None, &mut function, false);
            self.functions.insert(id, function).unwrap();
        }
    }

    fn lower_struct(&mut self, struct_type: &'a AstStruct) {
        let Some(name) = struct_type.name.value() else {
            return;
        };

        let type_parameters =
            self.lower_type_parameters(&[], None, struct_type.type_parameters.as_ref());
        let self_type = NamedType {
            name: name.string.clone(),
            type_arguments: type_parameters.type_(),
        };

        let fields = struct_type
            .fields
            .iter()
            .filter_map(|field| {
                let name = field.name.value()?;

                let type_ =
                    self.lower_type(&type_parameters, Some(&self_type), field.type_.value());
                Some((name.string.clone(), type_))
            })
            .collect();

        if self.traits.contains_key(&name.string) {
            self.add_error(
                name.span.clone(),
                format!("Duplicate type name: `{}`", name.string),
            );
            return;
        }
        match self.hir.type_declarations.entry(name.string.clone()) {
            Entry::Occupied(_) => {
                self.add_error(
                    name.span.clone(),
                    format!("Duplicate type name: `{}`", name.string),
                );
            }
            Entry::Vacant(entry) => {
                entry.insert(TypeDeclaration {
                    type_parameters,
                    kind: TypeDeclarationKind::Struct { fields },
                });
            }
        };
    }
    fn lower_enum(&mut self, enum_type: &'a AstEnum) {
        let Some(name) = enum_type.name.value() else {
            return;
        };

        let type_parameters =
            self.lower_type_parameters(&[], None, enum_type.type_parameters.as_ref());
        let self_type = NamedType {
            name: name.string.clone(),
            type_arguments: type_parameters.type_(),
        };

        let variants = enum_type
            .variants
            .iter()
            .filter_map(|variant| {
                let name = variant.name.value()?;

                let type_ = variant
                    .type_
                    .as_ref()
                    .map(|it| self.lower_type(&type_parameters, Some(&self_type), it.value()));
                Some((name.string.clone(), type_))
            })
            .collect();

        if self.traits.contains_key(&name.string) {
            self.add_error(
                name.span.clone(),
                format!("Duplicate type name: `{}`", name.string),
            );
            return;
        }
        match self.hir.type_declarations.entry(name.string.clone()) {
            Entry::Occupied(_) => {
                self.add_error(
                    name.span.clone(),
                    format!("Duplicate type name: `{}`", name.string),
                );
            }
            Entry::Vacant(entry) => {
                entry.insert(TypeDeclaration {
                    type_parameters,
                    kind: TypeDeclarationKind::Enum { variants },
                });
            }
        };
    }

    fn lower_trait_definition(&mut self, trait_: &'a AstTrait) {
        let Some(name) = trait_.name.value() else {
            return;
        };

        let type_parameters =
            self.lower_type_parameters(&[], None, trait_.type_parameters.as_ref());

        let solver_goal = SolverGoal {
            trait_: name.string.clone(),
            parameters: vec![SolverVariable::self_().into()].into_boxed_slice(),
        };
        let solver_subgoals = type_parameters
            .iter()
            .filter_map(|type_parameter| type_parameter.clone().try_into().ok())
            .collect();

        let self_type = NamedType {
            name: name.string.clone(),
            type_arguments: type_parameters.type_(),
        };
        let functions =
            self.lower_function_signatures(&type_parameters, Some(&self_type), &trait_.functions);

        if self.hir.type_declarations.contains_key(&name.string) {
            self.add_error(
                name.span.clone(),
                format!("Duplicate type name: `{}`", name.string),
            );
            return;
        }
        match self.traits.entry(name.string.clone()) {
            Entry::Occupied(_) => {
                self.add_error(
                    name.span.clone(),
                    format!("Duplicate type name: `{}`", name.string),
                );
            }
            Entry::Vacant(entry) => {
                entry.insert(TraitDeclaration {
                    type_parameters,
                    solver_goal,
                    solver_subgoals,
                    functions,
                });
            }
        };
    }
    fn lower_trait(
        &mut self,
        type_parameters: &[TypeParameter],
        self_type: Option<&NamedType>,
        type_: impl Into<Option<&AstType>>,
    ) -> hir::Result<Trait> {
        let type_: Option<&AstType> = type_.into();
        let Some(type_) = type_ else {
            return hir::Err;
        };

        let Some(name) = type_.name.value() else {
            return hir::Err;
        };

        if &*name.string == "Self" {
            self.add_error(name.span.clone(), "`Self` is not a trait.");
            return hir::Err;
        }

        if type_parameters.iter().any(|it| it.name == name.string) {
            self.add_error(
                name.span.clone(),
                format!("`{}` is a type parameter, not a trait.", name.string),
            );
            return hir::Err;
        }

        let type_arguments = type_
            .type_arguments
            .as_ref()
            .map_or_else(Box::default, |it| {
                it.arguments
                    .iter()
                    .map(|it| self.lower_type(type_parameters, self_type, &it.type_))
                    .collect::<Box<_>>()
            });

        if &*name.string == "Int" {
            self.add_error(name.span.clone(), "Int is a type, not a trait.");
            return hir::Err;
        }
        if &*name.string == "Text" {
            self.add_error(name.span.clone(), "Text is a type, not a trait.");
            return hir::Err;
        }

        let type_parameters = match self.ast.iter().find_map(|it| match it {
            AstDeclaration::Trait(AstTrait {
                name: it_name,
                type_parameters,
                ..
            }) if it_name.value().map(|it| &it.string) == Some(&name.string) => {
                Some(Ok(type_parameters
                    .as_ref()
                    .map_or::<&[AstTypeParameter], _>(&[], |it| &it.parameters)))
            }
            AstDeclaration::Struct(AstStruct { name: it_name, .. })
            | AstDeclaration::Enum(AstEnum { name: it_name, .. })
                if it_name.value().map(|it| &it.string) == Some(&name.string) =>
            {
                self.add_error(name.span.clone(), "Types can't be used as traits.");
                Some(Err(()))
            }
            _ => None,
        }) {
            Some(Ok(type_parameters)) => type_parameters,
            Some(Err(())) => return hir::Err,
            None => {
                self.add_error(name.span.clone(), format!("Unknown trait: `{}`", **name));
                return hir::Err;
            }
        };

        let type_arguments: Box<[Type]> = if type_arguments.len() == type_parameters.len() {
            type_arguments
        } else {
            self.add_error(
                type_.type_arguments.as_ref().unwrap().span.clone(),
                format!(
                    "Expected {} type {}, got {}.",
                    type_parameters.len(),
                    if type_parameters.len() == 1 {
                        "argument"
                    } else {
                        "arguments"
                    },
                    type_arguments.len(),
                ),
            );
            if type_arguments.len() < type_parameters.len() {
                let missing_count = type_parameters.len() - type_arguments.len();
                type_arguments
                    .into_vec()
                    .into_iter()
                    .chain(iter::repeat_n(Type::Error, missing_count))
                    .collect()
            } else {
                let mut type_arguments = type_arguments.into_vec();
                type_arguments.truncate(type_parameters.len());
                type_arguments.into_boxed_slice()
            }
        };

        hir::Ok(Trait {
            name: name.string.clone(),
            type_arguments,
        })
    }
    fn lower_impl(&mut self, impl_: &'a AstImpl) {
        let type_parameters = self.lower_type_parameters(&[], None, impl_.type_parameters.as_ref());

        let Some(ast_type) = impl_.type_.value() else {
            return;
        };
        let type_ = self.lower_type(&type_parameters, None, ast_type);
        let Type::Named(self_type) = type_.clone() else {
            self.add_error(
                ast_type
                    .name
                    .value()
                    .map_or_else(|| Offset(0)..Offset(0), |it| it.span.clone()),
                "Expected a named type.",
            );
            return;
        };

        let Some(trait_) = impl_.trait_.value() else {
            return;
        };
        let trait_ = self.lower_trait(&type_parameters, Some(&self_type), trait_);

        if let Type::Named(type_) = &type_
            && let Ok(solver_type) = SolverValue::try_from(type_.clone())
            && let hir::Ok(Trait {
                name: trait_name, ..
            }) = &trait_
        {
            let trait_declaration = &self.traits[trait_name];

            let rule = SolverRule {
                goal: SolverGoal {
                    trait_: trait_name.clone(),
                    parameters: trait_declaration
                        .type_parameters
                        .iter()
                        .map(|it| SolverVariable::new(it.type_()).into())
                        .chain([solver_type.into()])
                        .collect(),
                },
                subgoals: type_parameters
                    .iter()
                    .filter_map(|type_parameter| type_parameter.clone().try_into().ok())
                    .collect(),
            };
            self.environment.rules.push(rule);
        };

        let functions =
            self.lower_function_signatures(&type_parameters, Some(&self_type), &impl_.functions);

        self.impls.push(ImplDeclaration {
            type_parameters,
            type_,
            self_type,
            trait_,
            functions,
        });
    }

    fn lower_type_parameters(
        &mut self,
        outer_type_parameters: &[TypeParameter],
        self_type: Option<&NamedType>,
        type_parameters: Option<&AstTypeParameters>,
    ) -> Box<[TypeParameter]> {
        type_parameters.map_or_else(Box::default, |it| {
            it.parameters
                .iter()
                .filter_map(|it| {
                    let name = it.name.value()?;
                    if outer_type_parameters
                        .iter()
                        .any(|it| it.name == name.string)
                    {
                        self.add_error(
                            name.span.clone(),
                            format!("Duplicate type parameter name: `{}`", name.string),
                        );
                        return None;
                    }

                    let upper_bound = it
                        .upper_bound
                        .as_ref()
                        .and_then(|it| it.value())
                        .map(|it| self.lower_trait(outer_type_parameters, self_type, it));
                    Some(TypeParameter {
                        name: name.string.clone(),
                        upper_bound,
                    })
                })
                .collect()
        })
    }
    fn lower_type(
        &mut self,
        type_parameters: &[TypeParameter],
        self_type: Option<&NamedType>,
        type_: impl Into<Option<&AstType>>,
    ) -> Type {
        let type_: Option<&AstType> = type_.into();
        let Some(type_) = type_ else {
            return Type::Error;
        };

        let Some(name) = type_.name.value() else {
            return Type::Error;
        };

        if &*name.string == "Self" {
            return self_type.map_or_else(
                || {
                    self.add_error(
                        name.span.clone(),
                        "`Self` can only be used in traits and impls",
                    );
                    Type::Error
                },
                |self_type| Type::Self_ {
                    base_type: self_type.clone(),
                },
            );
        }

        if let Some(type_parameter) = type_parameters.iter().find(|it| it.name == name.string) {
            if let Some(type_arguments) = &type_.type_arguments {
                self.add_error(
                    type_arguments.span.clone(),
                    "Type parameters can't have type arguments",
                );
            }
            return type_parameter.type_().into();
        }

        let type_arguments = type_
            .type_arguments
            .as_ref()
            .map_or_else(Box::default, |it| {
                it.arguments
                    .iter()
                    .map(|it| self.lower_type(type_parameters, self_type, &it.type_))
                    .collect::<Box<_>>()
            });

        if &*name.string == "Int" {
            if !type_arguments.is_empty() {
                self.add_error(
                    type_.type_arguments.as_ref().unwrap().span.clone(),
                    "Int does not take type arguments",
                );
            }
            return NamedType::int().into();
        }
        if &*name.string == "Text" {
            if !type_arguments.is_empty() {
                self.add_error(
                    type_.type_arguments.as_ref().unwrap().span.clone(),
                    "Text does not take type arguments",
                );
            }
            return NamedType::text().into();
        }

        let type_parameters = match self.ast.iter().find_map(|it| match it {
            AstDeclaration::Struct(AstStruct {
                name: it_name,
                type_parameters,
                ..
            })
            | AstDeclaration::Enum(AstEnum {
                name: it_name,
                type_parameters,
                ..
            }) if it_name.value().map(|it| &it.string) == Some(&name.string) => {
                Some(Ok(type_parameters
                    .as_ref()
                    .map_or::<&[AstTypeParameter], _>(&[], |it| &it.parameters)))
            }
            AstDeclaration::Trait(AstTrait { name: it_name, .. })
                if it_name.value().map(|it| &it.string) == Some(&name.string) =>
            {
                self.add_error(name.span.clone(), "Traits can't be used as types");
                Some(Err(()))
            }
            _ => None,
        }) {
            Some(Ok(type_parameters)) => type_parameters,
            Some(Err(())) => return Type::Error,
            None => {
                self.add_error(name.span.clone(), format!("Unknown type: `{}`", **name));
                return Type::Error;
            }
        };

        let type_arguments: Box<[Type]> = if type_arguments.len() == type_parameters.len() {
            type_arguments
        } else {
            self.add_error(
                type_.type_arguments.as_ref().unwrap().span.clone(),
                format!(
                    "Expected {} type {}, got {}.",
                    type_parameters.len(),
                    if type_parameters.len() == 1 {
                        "argument"
                    } else {
                        "arguments"
                    },
                    type_arguments.len(),
                ),
            );
            if type_arguments.len() < type_parameters.len() {
                let missing_count = type_parameters.len() - type_arguments.len();
                type_arguments
                    .into_vec()
                    .into_iter()
                    .chain(iter::repeat_n(Type::Error, missing_count))
                    .collect()
            } else {
                let mut type_arguments = type_arguments.into_vec();
                type_arguments.truncate(type_parameters.len());
                type_arguments.into_boxed_slice()
            }
        };

        NamedType {
            name: name.string.clone(),
            type_arguments,
        }
        .into()
    }

    fn lower_assignment_signature(&mut self, assignment: &'a AstAssignment) -> Option<Id> {
        let name = assignment.name.value()?;

        let id = self.id_generator.generate();
        // TODO: infer type
        let type_ = assignment
            .type_
            .as_ref()
            .map_or(Type::Error, |it| self.lower_type(&[], None, it.value()));

        match self.global_identifiers.entry(name.string.clone()) {
            Entry::Occupied(mut entry) => {
                self.errors.push(CompilerError {
                    path: self.path.to_path_buf(),
                    span:
                    name.span.clone(),
                    message: match entry.get_mut() {
                        Named::Functions(_) => "A top-level assignment can't have the same name as a top-level function.".to_string(),
                        Named::Assignment(_) => "Top-level assignments can't have the same name.".to_string(),
                    },
                });
                return None;
            }
            Entry::Vacant(entry) => {
                entry.insert(Named::Assignment(id));
            }
        }

        let graph_index = self.assignment_dependency_graph.add_node(id);

        self.assignments.force_insert(
            id,
            AssignmentDeclaration {
                ast: assignment,
                type_,
                graph_index,
                body: None,
            },
        );
        Some(id)
    }
    fn lower_assignment(&mut self, id: Id) {
        let declaration = self.assignments.get(&id).unwrap();
        let value = declaration.ast.value.clone();
        let type_ = declaration.type_.clone();
        let graph_index = declaration.graph_index;

        let (hir_body, global_assignment_dependencies) =
            BodyBuilder::build(self, &[], None, |builder| {
                if let Some(value) = value.value() {
                    builder.lower_expression(value, Some(&type_));
                } else {
                    builder.push_error();
                }
            });

        for dependency_id in global_assignment_dependencies {
            let dependency = self.assignments.get(&dependency_id).unwrap();
            self.assignment_dependency_graph
                .add_edge(graph_index, dependency.graph_index, ());
        }

        self.assignments.get_mut(&id).unwrap().body = Some(hir_body);
    }

    fn lower_function_signatures(
        &mut self,
        outer_type_parameters: &[TypeParameter],
        self_type: Option<&NamedType>,
        functions: &'a [AstFunction],
    ) -> FxHashMap<Id, FunctionDeclaration<'a>> {
        functions
            .iter()
            .filter_map(|function| {
                self.lower_function_signature(outer_type_parameters, self_type, function)
            })
            .collect()
    }
    fn lower_top_level_function_signature(
        &mut self,
        outer_type_parameters: &[TypeParameter],
        self_type: Option<&NamedType>,
        function: &'a AstFunction,
    ) -> Option<(Id, FunctionDeclaration<'a>)> {
        let (id, function) =
            self.lower_function_signature(outer_type_parameters, self_type, function)?;
        match self.global_identifiers.entry(function.name.clone()) {
            Entry::Occupied(mut entry) => match entry.get_mut() {
                Named::Functions(functions) => {
                    // TODO: check for invalid overloads
                    functions.push(id);
                }
                Named::Assignment(_) => {
                    self.add_error(
                        function.ast.unwrap().display_span.clone(),
                        "A top-level function can't have the same name as a top-level assignment.",
                    );
                    return None;
                }
            },
            Entry::Vacant(entry) => {
                entry.insert(Named::Functions(vec![id]));
            }
        }
        Some((id, function))
    }
    fn lower_function_signature(
        &mut self,
        outer_type_parameters: &[TypeParameter],
        self_type: Option<&NamedType>,
        function: &'a AstFunction,
    ) -> Option<(Id, FunctionDeclaration<'a>)> {
        let name = function.name.value()?;

        let id = self.id_generator.generate();

        let type_parameters = self.lower_type_parameters(
            outer_type_parameters,
            self_type,
            function.type_parameters.as_ref(),
        );
        let all_type_parameters = outer_type_parameters
            .iter()
            .chain(type_parameters.iter())
            .cloned()
            .collect::<Box<_>>();

        let parameters =
            self.lower_parameters(&all_type_parameters, self_type, &function.parameters);
        let return_type = function.return_type.as_ref().map_or_else(
            || NamedType::nothing().into(),
            |it| self.lower_type(&all_type_parameters, self_type, it),
        );
        Some((
            id,
            FunctionDeclaration {
                ast: Some(function),
                name: name.string.clone(),
                type_parameters,
                parameters,
                return_type,
                body: None,
            },
        ))
    }
    fn lower_parameters(
        &mut self,
        type_parameters: &[TypeParameter],
        self_type: Option<&NamedType>,
        parameters: &'a [AstParameter],
    ) -> Box<[Parameter]> {
        let mut parameter_names = FxHashSet::default();
        parameters
            .iter()
            .filter_map(|parameter| try {
                let name = parameter.name.value()?.clone();
                if !parameter_names.insert(name.clone()) {
                    self.add_error(
                        name.span.clone(),
                        format!("Duplicate parameter name: {}", *name),
                    );
                    return None;
                }

                let type_ = self.lower_type(type_parameters, self_type, parameter.type_.value());

                let id = self.id_generator.generate();
                Parameter {
                    id,
                    name: name.string,
                    type_,
                }
            })
            .collect()
    }
    fn lower_function(
        &mut self,
        self_type: Option<&NamedType>,
        function: &mut FunctionDeclaration<'a>,
        body_is_optional: bool,
    ) {
        if body_is_optional && function.ast.unwrap().body.is_none() {
            return;
        }

        let (hir_body, _) =
            BodyBuilder::build(self, &function.type_parameters, self_type, |builder| {
                for parameter in function.parameters.iter() {
                    builder.push_parameter(parameter.clone());
                }

                if let Some(body) = function.ast.unwrap().body.as_ref() {
                    builder.lower_statements(&body.body, Some(&function.return_type));
                } else {
                    builder.context.add_error(
                        function.ast.unwrap().display_span.clone(),
                        "No function body provided",
                    );
                    builder.push_panic("No function body provided");
                }
            });

        function.body = Some(BodyOrBuiltin::Body(hir_body));
    }
    fn get_function(&self, id: Id) -> (&FunctionDeclaration<'a>, Option<&TraitDeclaration<'a>>) {
        self.functions
            .get(&id)
            .map(|function| (function, None))
            .or_else(|| {
                self.traits.iter().find_map(|(_, trait_)| {
                    trait_
                        .functions
                        .get(&id)
                        .map(|function| (function, Some(trait_)))
                })
            })
            .unwrap()
    }

    fn is_assignable_to(from: &Type, to: &Type) -> bool {
        match (from, to) {
            (Type::Error, _) | (_, Type::Error) => true,
            (Type::Named(from), Type::Named(to)) => {
                from.name == to.name
                    && from
                        .type_arguments
                        .iter()
                        .zip_eq(to.type_arguments.iter())
                        .all(|(from, to)| Self::is_assignable_to(from, to))
            }
            (Type::Parameter(from), Type::Parameter(to)) => from == to,
            // TODO: Self type
            _ => false,
        }
    }

    fn get_all_functions_matching_name(&mut self, name: &str) -> Vec<Id> {
        self.functions
            .iter()
            .chain(self.traits.iter().flat_map(|(_, trait_)| &trait_.functions))
            .filter(|(_, function)| &*function.name == name)
            .map(|(id, _)| *id)
            .collect()
    }
    // fn instanceFunctionsWithSubstitutions(
    //     &mut self,
    //     type_: &Type,
    // ) -> FxHashMap<Id, FxHashMap<Type, Type>> {
    //     self.getAllImplsWithSubstitutionsFor(type_)
    //         .into_iter()
    //         .flat_map(|(impl_, substitutions)| {
    //             impl_
    //                 .functions
    //                 .keys()
    //                 .map(|id| (*id, substitutions.clone()))
    //                 .collect_vec()
    //         })
    //         .collect()
    // }
    // fn instanceFunctionsWithSubstitutionsMatchingName(
    //     &mut self,
    //     type_: &Type,
    //     name: &str,
    // ) -> FxHashMap<Id, FxHashMap<Type, Type>> {
    //     self.instanceFunctionsWithSubstitutions(type_)
    //         .into_iter()
    //         .filter(|(id, _)| &*self.get_function(*id).name == name)
    //         .collect()
    // }
    // fn leafInstanceFunctionsWithSubstitutionsMatchingName(
    //     &mut self,
    //     type_: &Type,
    //     name: &str,
    // ) -> FxHashMap<Id, FxHashMap<Type, Type>> {
    //     let functions = self.instanceFunctionsWithSubstitutionsMatchingName(type_, name);
    //     // TODO
    //     // self.leafFunctionsWithSubstitutionsFromSet(&functions)
    //     functions
    // }

    // fn leafFunctionsWithSubstitutionsFromSet(
    //     &mut self,
    //     functions: &FxHashMap<Id, FxHashMap<Type, Type>>,
    // ) -> FxHashMap<Id, FxHashMap<Type, Type>> {
    //     let parent_functions = functions
    //         .iter()
    //         .map(|(id, _)| id)
    //         .filter_map(|id| self.parentFunction(*id))
    //         .map(|(id, _)| id)
    //         .collect::<FxHashSet<_>>();
    //     functions
    //         .iter()
    //         .filter(|(id, _)| parent_functions.contains(id))
    //         .map(|(id, substitutions)| (*id, substitutions.clone()))
    //         .collect()
    // }

    // fn parentFunction(&mut self, id: Id) -> Option<(Id, FxHashMap<Type, Type>)> {
    //     let parent_type = if let Some(parent_impl) =
    //         self.impls.iter().find(|it| it.functions.contains_key(&id))
    //     {
    //         parent_impl.trait_.clone()
    //     } else if self
    //         .traits
    //         .values()
    //         .any(|it| it.functions.contains_key(&id))
    //     {
    //         // TODO: use upper bound
    //         return None;
    //     } else {
    //         return None;
    //     };

    //     let function = self.get_function(id).clone();

    //     let mut function_candidates = self
    //         .instanceFunctionsWithSubstitutions(&parent_type)
    //         .into_iter()
    //         .filter(|(other_id, _)| function.is_assignable_to(self.get_function(*other_id)))
    //         .map(|(id, substitutions)| (id, substitutions))
    //         .collect_vec();

    //     if function_candidates.len() > 1 {
    //         self.add_error(
    //             function.ast.unwrap().display_span.clone(),
    //             "Multiple functions with the same name and signature found in parent trait",
    //         );
    //         return Some(function_candidates.pop().unwrap());
    //     }
    //     function_candidates.pop()
    // }

    // fn trait_upper_bound_to_type_solver_roles(trait_: &AstTrait) -> FxHashSet<SolverRule> {
    //     // Lower the constraints. For example, in the impl `impl[T: Equals] Foo[T]: Equals`, the
    //     // `solverConstraints` are a list containing `Equals(?T)`.
    //     let rules = FxHashSet::default();
    //     let solverConstraints = vec![];
    //     for constraint in hirImpl.constraints(context).entries() {
    //         let result = hirInlineTypeToSolverTypeAndGoalsAndRules(context, constraint.second);
    //         rules.addAll(result.third.items());
    //         solverConstraints.appendAll(
    //             (result.second as Iterable<SolverGoal>).map<SolverGoal>({
    //             it.substituteAll(
    //                 Map.of1<SolverVariable, SolverType>(
    //                 Tuple(
    //                     result.first as SolverVariable,
    //                     SolverVariable(Some<HirParameterType>(constraint.first)),
    //                 ),
    //                 ),
    //             )
    //             }),
    //         );
    //     }

    //     // Lower the base type. For example, in the impl `impl Iterable[Int]: Foo`, the base type
    //     // `Iterable[Int]` gets lowered to `?0` with the goal `Iterable(Int, ?0)`.
    //     let baseType = hirImpl.baseType(context)
    //     let result = hirInlineTypeToSolverTypeAndGoalsAndRules(context, baseType)
    //     rules.addAll(result.third.items())
    //     solverConstraints.appendAll(result.second)
    //     let solverBase = result.first

    //     // Lower the trait. For example, in the impl `impl Foo: Iterable[Int]`, the implemented trait
    //     // `Iterable[Int]` gets lowered to `?0` with the goals `Iterable(Int, ?0)`. Impls that don't
    //     // implement a trait – like `impl Foo { ... }` – don't correspond to a `SolverRule` and cause
    //     // this function to return `None`.
    //     let traitType = hirImpl.implementedTrait(context)
    //     if (traitType is None) { return Tuple(Set.empty<SolverRule>(), List.empty<CompilerError>()) }
    //     if (traitType.unwrap() is HirErrorType) {
    //     return Tuple(Set.empty<SolverRule>(), List.empty<CompilerError>())
    //     }
    //     let traitType = (traitType.unwrap() as HirNamedType)
    //     let result = hirInlineTypeToSolverTypeAndGoalsAndRules(context, traitType)
    //     let solverTrait = (result.first as SolverVariable) // Traits get lowered to `SolverVariable`s.
    //     let traitGoals = result.second
    //     rules.addAll(result.third.items())
    //     if ((traitGoals as Iterable).length() > 1) {
    //     // TODO(never, marcelgarus): We can't implement a trait that contains another trait yet, like
    //     // `Iterable<Equals>`. This does work in Rust (with boxing and explicit dynamism), so we'll
    //     // probably have to look at how to put general-purpose logic implications in our solver (like
    //     // `A B -> C D` instead of only having one implicative result like `A B -> C` in the solver).
    //     // Or we need to somehow reduce this to multiple simple rules.
    //     // For now, we're probably fine with only implementing "simple" traits like `Iterable<Int>`.
    //     // (As "primitive" types like `List` etc. are also traits, resolving this todo is somewhat of
    //     // a priority.)
    //     return Tuple(
    //         Set.empty<SolverRule>(),
    //         List.of1<CompilerError>(CannotImplementTraitOfTraitCompilerError(hirImpl)),
    //     )
    //     }
    //     assert((traitGoals as Iterable).length() == 1, "Should never happen.")
    //     let traitGoal = (traitGoals as Iterable<SolverGoal>).single().unwrap()

    //     // Let's play this through for the impl `impl[T: Equals] Iterable[T]: Equals`.
    //     //
    //     // These would be the values calculated above:
    //     //
    //     // * `constraints`: list with goal `Equals(?T)`
    //     // * `solverBase`: type `?0` and goal `Iterable(?T, ?0)`
    //     // * `solverTrait`: type `?1` and goal `Equals(?1)`
    //     //
    //     // The goal that this impl wants to achieve can be calculated by taking the `solverTrait`, which
    //     // is guaranteed to have only one goal `Equals(?1)`, and replacing the `solverTrait`'s type `?1`
    //     // with the `solverBase` type `?0` – which results in `Equals(?0)`.
    //     // To achieve that goal, we have to satisfy the goals of the `solverBase` and the `constraints`.
    //     // So, our total `SolverRule` would look like this: `Equals(?0) <- Iterable(?T, ?0), Equals(?T)`
    //     Tuple(
    //     Set.of1<SolverRule>(
    //         SolverRule(
    //         hirImpl,
    //         traitGoal.substituteAll(Map.of1<SolverVariable, SolverType>(Tuple(solverTrait, solverBase))),
    //         solverConstraints,
    //         ),
    //     ).union(rules),
    //     List.empty<CompilerError>(),
    //     )
    // }
    // fn hirInlineTypeToSolverTypeAndGoalsAndRules() {

    // }

    // fn find_unique_solver_solution_for(
    //     &mut self,
    //     base: &Type,
    //     trait_: &Type,
    // ) -> Option<SolverSolutionUnique> {
    //     assert!(
    //         trait_ != &Type::Error,
    //         "Can't reveal the impl for the error type",
    //     );

    //     let base_type = Self::type_to_solver_type(base);

    //     let (trait_type, mut trait_goals) = Self::trait_to_solver_type_and_goals(trait_);
    //     let SolverType::Variable(trait_type) = trait_type else {
    //         panic!("This shouldn't happen. Trait should be lowered to a SolverVariable.");
    //     };
    //     if trait_goals.len() != 1 {
    //         self.add_error(
    //             Offset(0)..Offset(0),
    //             "Trying to find impl for trait with trait as parameter",
    //         );
    //         return None;
    //     }
    //     let trait_goal = trait_goals.pop().unwrap();

    //     let solution = self.environment.solve(
    //         &trait_goal.substitute_all(&FxHashMap::from_iter([(trait_type, base_type)])),
    //         &[],
    //     );
    //     match solution {
    //         SolverSolution::Unique(solution) => Some(solution),
    //         SolverSolution::Ambiguous | SolverSolution::Impossible => None,
    //     }
    // }

    fn add_error(&mut self, span: Range<Offset>, message: impl Into<String>) {
        self.errors.push(CompilerError {
            path: self.path.to_path_buf(),
            span,
            message: message.into(),
        });
    }
}

struct BodyBuilder<'c, 'a> {
    context: &'c mut Context<'a>,
    global_assignment_dependencies: FxHashSet<Id>,
    type_parameters: &'c [TypeParameter],
    self_type: Option<&'c NamedType>,
    local_identifiers: Vec<(Box<str>, Id, Type)>,
    body: Body,
}
impl<'c, 'a> BodyBuilder<'c, 'a> {
    #[must_use]
    fn build(
        context: &'c mut Context<'a>,
        type_parameters: &'c [TypeParameter],
        self_type: Option<&'c NamedType>,
        fun: impl FnOnce(&mut BodyBuilder),
    ) -> (Body, FxHashSet<Id>) {
        let mut builder = Self {
            context,
            global_assignment_dependencies: FxHashSet::default(),
            type_parameters,
            self_type,
            local_identifiers: vec![],
            body: Body::default(),
        };
        fun(&mut builder);
        (builder.body, builder.global_assignment_dependencies)
    }
    #[must_use]
    fn build_inner(&mut self, fun: impl FnOnce(&mut BodyBuilder)) -> Body {
        BodyBuilder::build(
            self.context,
            self.type_parameters,
            self.self_type,
            |builder| {
                builder.local_identifiers = self.local_identifiers.clone();
                fun(builder);
                self.global_assignment_dependencies
                    .extend(&builder.global_assignment_dependencies);
            },
        )
        .0
    }

    fn lower_statements(
        &mut self,
        statements: &[AstStatement],
        context_type: Option<&Type>,
    ) -> (Id, Type) {
        let mut last_expression = None;
        for (position, statement) in statements.iter().with_position() {
            let statement_context_type = if matches!(position, Position::Last | Position::Only) {
                context_type
            } else {
                None
            };

            match statement {
                AstStatement::Assignment(assignment) => {
                    let Some(name) = assignment.name.value().cloned() else {
                        continue;
                    };

                    let type_ = assignment.type_.as_ref().map(|it| {
                        self.context
                            .lower_type(self.type_parameters, self.self_type, it.value())
                    });

                    let (id, type_) = if let Some(value) = assignment.value.value() {
                        self.lower_expression(value, type_.as_ref())
                    } else {
                        (self.push_error(), Type::Error)
                    };
                    self.push(name.string.clone(), ExpressionKind::Reference(id), type_);
                    last_expression = None;
                }
                AstStatement::Expression(expression) => {
                    last_expression =
                        Some(self.lower_expression(expression, statement_context_type));
                }
            }
        }

        let last_expression = last_expression.unwrap_or_else(|| {
            // TODO: check return type
            let id = self.push_nothing();
            (id, NamedType::nothing().into())
        });
        let return_type = context_type.cloned().unwrap_or(last_expression.1);
        (last_expression.0, return_type)
    }

    fn lower_expression(
        &mut self,
        expression: &AstExpression,
        context_type: Option<&Type>,
    ) -> (Id, Type) {
        match self.lower_expression_raw(expression, context_type) {
            LoweredExpression::Expression { id, type_ } => {
                if let Some(context_type) = context_type
                    && !Context::is_assignable_to(&type_, context_type)
                {
                    // TODO: report actual error location
                    self.context.add_error(
                        Offset(0)..Offset(0),
                        format!("Expected type `{context_type:?}`, got `{type_:?}`."),
                    );
                    (self.push_error(), Type::Error)
                } else {
                    (id, type_)
                }
            }
            LoweredExpression::NamedTypeReference(_)
            | LoweredExpression::TypeParameterReference { .. } => {
                // TODO: report actual error location
                self.context
                    .add_error(Offset(0)..Offset(0), "Type must be instantiated.");
                (self.push_error(), Type::Error)
            }
            LoweredExpression::EnumVariantReference { enum_, variant } => {
                // TODO: report actual error location
                self.context.add_error(
                    Offset(0)..Offset(0),
                    format!("Enum variant `{enum_:?}.{variant}` must be instantiated."),
                );
                (self.push_error(), Type::Error)
            }
            LoweredExpression::Error => (self.push_error(), Type::Error),
        }
    }
    fn lower_expression_raw(
        &mut self,
        expression: &AstExpression,
        context_type: Option<&Type>,
    ) -> LoweredExpression {
        match expression {
            AstExpression::Identifier(identifier) => {
                let Some(identifier) = identifier.identifier.value() else {
                    return LoweredExpression::Error;
                };
                self.lower_identifier(identifier)
            }
            AstExpression::Int(int) => self.push_lowered(
                None,
                int.value
                    .value()
                    .map_or(ExpressionKind::Error, |it| ExpressionKind::Int(*it)),
                NamedType::int(),
            ),
            AstExpression::Text(text) => {
                let text = text
                    .parts
                    .iter()
                    .map::<Id, _>(|it| match it {
                        AstTextPart::Text(text) => {
                            self.push(None, ExpressionKind::Text(text.clone()), NamedType::text())
                        }
                        AstTextPart::Interpolation { expression, .. } => {
                            if let Some(expression) = expression.value() {
                                self.lower_expression(expression, Some(&NamedType::text().into()))
                                    .0
                            } else {
                                self.push_error()
                            }
                        }
                    })
                    .collect_vec()
                    .into_iter()
                    .reduce(|lhs, rhs| {
                        self.push(
                            None,
                            ExpressionKind::Call {
                                function: BuiltinFunction::TextConcat.id(),
                                used_goal: None,
                                substitutions: FxHashMap::default(),
                                arguments: [lhs, rhs].into(),
                            },
                            NamedType::text(),
                        )
                    })
                    .unwrap_or_else(|| {
                        self.push(None, ExpressionKind::Text("".into()), NamedType::text())
                    });
                LoweredExpression::Expression {
                    id: text,
                    type_: NamedType::text().into(),
                }
            }
            AstExpression::Parenthesized(parenthesized) => {
                return parenthesized
                    .inner
                    .value()
                    .map_or(LoweredExpression::Error, |it| {
                        self.lower_expression_raw(it, context_type)
                    });
            }
            AstExpression::Call(call) => {
                fn lower_arguments(
                    builder: &mut BodyBuilder,
                    call: &AstCall,
                    arguments: &AstResult<AstArguments>,
                    parameter_types: Option<&[Type]>,
                ) -> Option<Box<[(Id, Type)]>> {
                    let arguments = arguments
                        .arguments_or_default()
                        .iter()
                        .enumerate()
                        .map(|(index, argument)| {
                            builder.lower_expression(
                                &argument.value,
                                parameter_types.and_then(|it| it.get(index)),
                            )
                        })
                        .collect::<Box<_>>();
                    if let Some(parameter_types) = parameter_types
                        && arguments.len() != parameter_types.len()
                    {
                        builder.context.add_error(
                            if arguments.len() < parameter_types.len() {
                                // TODO: report actual error location
                                call.arguments.value().map_or(Offset(0)..Offset(0), |it| {
                                    it.opening_parenthesis_span.clone()
                                })
                            } else {
                                let arguments = &call.arguments.value().unwrap().arguments;
                                arguments[parameter_types.len()].span.start
                                    ..arguments.last().unwrap().span.end
                            },
                            format!(
                                "Expected {} argument(s), got {}.",
                                parameter_types.len(),
                                arguments.len(),
                            ),
                        );
                        return None;
                    }
                    Some(arguments)
                }

                match &*call.receiver {
                    AstExpression::Navigation(navigation)
                        if let Some(key) = navigation.key.value() =>
                    {
                        let receiver = self.lower_expression(&navigation.receiver, None);
                        let arguments = lower_arguments(self, call, &call.arguments, None).unwrap();
                        let arguments = iter::once(receiver)
                            .chain(arguments.into_vec())
                            .collect_vec();
                        return self.lower_call(key, call.type_arguments.as_ref(), &arguments);
                    }
                    AstExpression::Identifier(identifier) => {
                        let Some(identifier) = identifier.identifier.value() else {
                            return LoweredExpression::Error;
                        };

                        if identifier.string.chars().next().unwrap().is_lowercase() {
                            let arguments =
                                lower_arguments(self, call, &call.arguments, None).unwrap();
                            return self.lower_call(
                                identifier,
                                call.type_arguments.as_ref(),
                                &arguments,
                            );
                        }

                        match self.lower_identifier(identifier) {
                            LoweredExpression::Expression { .. } => todo!("support lambdas"),
                            LoweredExpression::NamedTypeReference(type_) => {
                                match self.context.hir.type_declarations.get(&type_) {
                                    Some(TypeDeclaration {
                                        type_parameters,
                                        kind: TypeDeclarationKind::Struct { fields },
                                    }) => {
                                        if !type_parameters.is_empty() {
                                            todo!("Use type solver");
                                        }

                                        let fields = lower_arguments(
                                            self,
                                            call,
                                            &call.arguments,
                                            Some(
                                                &fields
                                                    .iter()
                                                    .map(|(_, type_)| type_.clone())
                                                    .collect_vec(),
                                            ),
                                        );
                                        let type_ = Type::Named(NamedType {
                                            name: type_.clone(),
                                            type_arguments: Box::default(),
                                        });
                                        fields.map_or(LoweredExpression::Error, |fields| {
                                            self.push_lowered(
                                                None,
                                                ExpressionKind::CreateStruct {
                                                    struct_: type_.clone(),
                                                    fields: fields
                                                        .iter()
                                                        .map(|(id, _)| *id)
                                                        .collect(),
                                                },
                                                type_,
                                            )
                                        })
                                    }
                                    Some(TypeDeclaration {
                                        kind: TypeDeclarationKind::Enum { .. },
                                        ..
                                    }) => {
                                        // TODO: report actual error location
                                        self.context.add_error(
                                            Offset(0)..Offset(0),
                                            "Enum variant is missing.",
                                        );
                                        LoweredExpression::Error
                                    }
                                    None => {
                                        // TODO: report actual error location
                                        self.context.add_error(
                                            Offset(0)..Offset(0),
                                            format!(
                                                "Can't instantiate builtin type {type_} directly."
                                            ),
                                        );
                                        LoweredExpression::Error
                                    }
                                }
                            }
                            LoweredExpression::TypeParameterReference(type_) => {
                                // TODO: report actual error location
                                self.context.add_error(
                                    Offset(0)..Offset(0),
                                    format!("Can't instantiate type parameter {type_} directly."),
                                );
                                LoweredExpression::Error
                            }
                            LoweredExpression::EnumVariantReference {
                                enum_: enum_type,
                                variant,
                            } => {
                                let Type::Named(enum_named_type) = &enum_type else {
                                    unreachable!();
                                };
                                let enum_ = self
                                    .context
                                    .hir
                                    .type_declarations
                                    .get(&enum_named_type.name)
                                    .unwrap();
                                let TypeDeclarationKind::Enum { variants } = &enum_.kind else {
                                    unreachable!();
                                };
                                let variant_type = variants
                                    .iter()
                                    .find(|(name, _)| name == &variant)
                                    .unwrap()
                                    .1
                                    .as_ref()
                                    .unwrap()
                                    .clone();
                                let variant_type =
                                    variant_type.substitute(&Type::build_environment(
                                        &enum_.type_parameters,
                                        &enum_named_type.type_arguments,
                                    ));
                                let parameter_types = [variant_type];
                                let arguments = lower_arguments(
                                    self,
                                    call,
                                    &call.arguments,
                                    Some(parameter_types.as_slice()),
                                );
                                arguments.map_or(LoweredExpression::Error, |arguments| {
                                    self.push_lowered(
                                        None,
                                        ExpressionKind::CreateEnum {
                                            enum_: enum_type.clone(),
                                            variant,
                                            value: arguments.first().map(|(id, _)| *id),
                                        },
                                        enum_type,
                                    )
                                })
                            }
                            LoweredExpression::Error => LoweredExpression::Error,
                        }
                    }
                    _ => todo!("Support calling other expressions"),
                }

                // let receiver = self.lower_expression_raw(&call.receiver, None);

                // match receiver {
                //     LoweredExpression::Expression { .. } => {
                //         // TODO: report actual error location
                //         self.context
                //             .add_error(Offset(0)..Offset(0), "Cannot call this type");
                //         LoweredExpression::Error
                //     }
                //     LoweredExpression::FunctionReferences {
                //         receiver,
                //         function_ids,
                //     } => {
                //         assert!(!function_ids.is_empty());

                //         self.lower_call(name, type_arguments, arguments)

                //         // let parameter_types = function
                //         //     .parameters
                //         //     .iter()
                //         //     .map(|it| it.type_.clone())
                //         //     .collect_vec();
                //         // let return_type = function.return_type.clone();

                //         //   if full_matches.is_empty() then return error[LookupFunSolution, Str]({
                //         //     var out = string_builder().&
                //         //     out.
                //         //       "This call doesn't work:{newline}
                //         //       ' > {call_signature(name, type_args, arg_types)}{newline}{newline}"
                //         //     if name_matches.is_empty()
                //         //     then out.'"There are no defintions named "{{name}}"."'
                //         //     else {
                //         //       out."These definitions have the same name, but arguments don't match:"
                //         //       for match in name_matches do
                //         //         out."{newline} - {AstDef.fun_(match).signature()}"
                //         //     }
                //         //     out.to_str()
                //         //   })
                //         //   if full_matches.len.is_greater_than(1) then return error[LookupFunSolution, Str]({
                //         //     var out = string_builder().&
                //         //     out.
                //         //       "This call doesn't work:{newline}
                //         //       ' > {call_signature(name, type_args, arg_types)}{newline}{newline}
                //         //       'Multiple definitions match:"
                //         //     for match in full_matches do {
                //         //       var padded_signature = "{AstDef.fun_(match.fun_).signature()}"
                //         //         .pad_right(30, # )
                //         //       out."{newline} - {padded_signature}"
                //         //       if match.type_env.is_not_empty() then {
                //         //         out." with "
                //         //         var first = true
                //         //         for entry in match.type_env do {
                //         //           if first then first = false else out.", "
                //         //           out."{entry.key} = {entry.value}"
                //         //         }
                //         //       }
                //         //     }
                //         //     out.to_str()
                //         //   })
                //         //   ok[LookupFunSolution, Str](full_matches.get(0))
                //     } //     LoweredExpression::Error => LoweredExpression::Error,
                // }
            }
            AstExpression::Navigation(navigation) => {
                let receiver = self.lower_expression_raw(&navigation.receiver, None);

                let Some(key) = navigation.key.value() else {
                    return LoweredExpression::Error;
                };

                match receiver {
                    LoweredExpression::Expression {
                        id: receiver_id,
                        type_: receiver_type,
                    } => match &receiver_type {
                        Type::Named(named_type) => {
                            let type_ = &self.context.hir.type_declarations.get(&named_type.name);
                            if let Some(TypeDeclaration {
                                type_parameters,
                                kind: TypeDeclarationKind::Struct { fields },
                            }) = type_
                                && let Some((_, field_type)) =
                                    fields.iter().find(|(name, _)| name == &key.string)
                            {
                                return self.push_lowered(
                                    None,
                                    ExpressionKind::StructAccess {
                                        struct_: receiver_id,
                                        field: key.string.clone(),
                                    },
                                    field_type.substitute(&Type::build_environment(
                                        type_parameters,
                                        &named_type.type_arguments,
                                    )),
                                );
                            }

                            // let functions = receiver
                            //     .map<List<HirFunction>>({ receiver =>
                            //     if (receiver is HirValueExpressionUri) {
                            //         let receiverType = this.typeOf(receiver as HirValueExpressionUri)
                            //         if (receiverType is HirErrorType) { return Tuple(this, receiver) }
                            //         if (receiverType is HirNamedType) {
                            //         return (receiverType as HirInlineType)
                            //             .leafInstanceFunctionsWithSubstitutionsMatchingName(
                            //             context.global.context,
                            //             name,
                            //             context.global.function.package(),
                            //             )
                            //         }
                            //     } else {
                            //         let receiver = (receiver as HirTypeExpression)
                            //         todo("soon, Support static function calls")
                            //     }

                            //     // TODO(soon, marcelgarus): Handle function call with a receiver.
                            //     let result = context.register(
                            //         HirStringValueExpression(
                            //         "<compiler-generated> TODO(soon, marcelgarus): Handle function call with a receiver.",
                            //         ),
                            //     )
                            //     context = result.first
                            //     return context.register(
                            //         HirFunctionCallValueExpression(
                            //         None<HirValueExpressionUri | HirTypeExpression>(),
                            //         HirFunction(
                            //             HirInnerModule(HirTopLevelModule(Package.core(context.global.context)), "Panic"),
                            //             "panic",
                            //             0,
                            //         ),
                            //         Map.empty<String, HirInlineType>(),
                            //         Map.of1<String, HirValueExpressionUri>(Tuple("message", result.second)),
                            //         ),
                            //     )
                            //     })
                            //     .orElse({
                            //     context.resolve(name).items()
                            //         .cast<HirValueExpressionUri | HirFunction>()
                            //         .where({
                            //         if (it is HirValueExpressionUri) {
                            //             todo("soon: Callable expressions are not yet supported: {function}")
                            //         } else {
                            //             (it as HirFunction).parent is HirModule
                            //         }
                            //         })
                            //         .cast<HirFunction>()
                            //         .toList()
                            //     });

                            // TODO: merge with global function resolution
                            // if let Some(Named::Functions(function_ids)) =
                            //     self.context.global_identifiers.get(&key.string)
                            // {
                            //     let function_ids = function_ids
                            //         .iter()
                            //         .map(|id| (*id, &self.context.functions[id]))
                            //         .filter(|(_, it)| {
                            //             !it.parameters.is_empty()
                            //                 && it.parameters[0].type_ == receiver_type
                            //         })
                            //         .map(|(id, _)| id)
                            //         .collect::<Box<_>>();
                            //     if !function_ids.is_empty() {
                            //         return LoweredExpression::FunctionReferences {
                            //             receiver: Some((receiver_id, receiver_type.clone())),
                            //             function_ids,
                            //         };
                            //     }
                            // }

                            self.context.add_error(
                                key.span.clone(),
                                format!(
                                    "Value of type `{receiver_type:?}` doesn't have a field `{}`",
                                    key.string
                                ),
                            );
                            LoweredExpression::Error
                        }
                        Type::Parameter(type_) => {
                            self.context.add_error(
                                key.span.clone(),
                                format!(
                                    "Navigation on value of type parameter type `{}` is not supported yet.",type_.name
                                ),
                            );
                            LoweredExpression::Error
                        }
                        Type::Self_ { .. } => todo!(),
                        Type::Error => todo!(),
                    },
                    LoweredExpression::NamedTypeReference(type_) => {
                        let declaration = self.context.hir.type_declarations.get(&type_).unwrap();
                        match &declaration.kind {
                            TypeDeclarationKind::Struct { .. } => {
                                self.context.add_error(
                                    key.span.clone(),
                                    format!(
                                        "Struct type `{type_:?}` doesn't have a field `{}`",
                                        key.string,
                                    ),
                                );
                                LoweredExpression::Error
                            }
                            TypeDeclarationKind::Enum { variants } => {
                                if !declaration.type_parameters.is_empty() {
                                    todo!();
                                }
                                let type_ = NamedType {
                                    name: type_.clone(),
                                    type_arguments: Box::default(),
                                }
                                .into();

                                if let Some((_, value_type)) =
                                    variants.iter().find(|(name, _)| name == &key.string)
                                {
                                    if value_type.is_some() {
                                        LoweredExpression::EnumVariantReference {
                                            enum_: type_,
                                            variant: key.string.clone(),
                                        }
                                    } else {
                                        self.push_lowered(
                                            None,
                                            ExpressionKind::CreateEnum {
                                                enum_: type_.clone(),
                                                variant: key.string.clone(),
                                                value: None,
                                            },
                                            type_,
                                        )
                                    }
                                } else {
                                    self.context.add_error(
                                        key.span.clone(),
                                        format!(
                                            "Enum `{type_:?}` doesn't have a variant `{}`",
                                            key.string,
                                        ),
                                    );
                                    LoweredExpression::Error
                                }
                            }
                        }
                    }
                    LoweredExpression::TypeParameterReference(type_parameter) => {
                        self.context.add_error(
                            key.span.clone(),
                            format!(
                                "Parameter type `{type_parameter:?}` doesn't have a field `{}`",
                                key.string,
                            ),
                        );
                        LoweredExpression::Error
                    }
                    LoweredExpression::EnumVariantReference { .. } => todo!(),
                    LoweredExpression::Error => LoweredExpression::Error,
                }
            }
            AstExpression::Body(AstBody { statements, .. }) => {
                let (id, type_) = self.lower_statements(statements, context_type);
                LoweredExpression::Expression { id, type_ }
            }
            AstExpression::Switch(AstSwitch { value, cases, .. }) => {
                let Some(value) = value.value() else {
                    return LoweredExpression::Error;
                };
                let (value, enum_) = self.lower_expression(value, None);

                let (environment, variants) = match &enum_ {
                    Type::Named(type_) => {
                        let Some(declaration) =
                            &self.context.hir.type_declarations.get(&type_.name)
                        else {
                            // TODO: report actual error location
                            self.context.add_error(
                                Offset(0)..Offset(0),
                                format!("Can't switch over builtin type `{enum_:?}`"),
                            );
                            return LoweredExpression::Error;
                        };
                        match &declaration.kind {
                            TypeDeclarationKind::Struct { .. } => {
                                // TODO: report actual error location
                                self.context.add_error(
                                    Offset(0)..Offset(0),
                                    format!("Can't switch over struct `{enum_:?}`"),
                                );
                                return LoweredExpression::Error;
                            }
                            TypeDeclarationKind::Enum { variants } => (
                                declaration
                                    .type_parameters
                                    .iter()
                                    .map(TypeParameter::type_)
                                    .zip_eq(type_.type_arguments.iter().cloned())
                                    .collect::<FxHashMap<_, _>>(),
                                variants.clone(),
                            ),
                        }
                    }
                    Type::Parameter(type_) => {
                        // TODO: report actual error location
                        self.context.add_error(
                            Offset(0)..Offset(0),
                            format!("Can't switch over type parameter `{}`", type_.name),
                        );
                        return LoweredExpression::Error;
                    }
                    Type::Self_ { .. } => todo!(),
                    Type::Error => return LoweredExpression::Error,
                };

                let mut variant_names = FxHashSet::default();
                let mut return_type = context_type.cloned();
                let cases = cases
                    .iter()
                    .filter_map(|case| try {
                        let variant = case.variant.value()?.clone();
                        let Some((_, value_type)) =
                            variants.iter().find(|(it, _)| it == &variant.string)
                        else {
                            self.context.add_error(
                                variant.span.clone(),
                                format!("Unknown variant in switch: `{}`", *variant),
                            );
                            return None;
                        };

                        if !variant_names.insert(variant.clone()) {
                            self.context.add_error(
                                variant.span.clone(),
                                format!("Duplicate variant in switch: `{}`", *variant),
                            );
                            return None;
                        }

                        let variant_value = match (value_type, &case.value_name) {
                            (None, None) => None,
                            (None, Some(_)) => {
                                self.context.add_error(
                                    variant.span.clone(),
                                    format!(
                                        "Switch case specifies value name for variant `{}` that doesn't have any value",
                                        *variant
                                    ),
                                );
                                return None;
                            },
                            (Some(_), None) => {
                                self.context.add_error(
                                    variant.span.clone(),
                                    format!(
                                        "Switch case is missing a value name for variant `{}`",
                                        *variant
                                    ),
                                );
                                return None;
                            },
                            (Some(value_type), Some((value_name, _))) => {
                                let value_name = value_name.value()?;
                                Some((value_type.substitute(&environment), value_name.string.clone()))
                            },
                        };

                        let mut value_id = None;
                        let body = self.build_inner(|builder| {
                            value_id = variant_value.map(|(type_,name)| {
                                let id = builder.context.id_generator.generate();
                                builder.push_parameter(Parameter { id, name, type_ });
                                id
                            });
                            if let Some(expression) = case.expression.value() {
                                let (_, new_return_type) = builder.lower_expression(expression, return_type.as_ref());
                                if return_type.is_none() {
                                    return_type = Some(new_return_type);
                                }
                            }
                        });
                        SwitchCase {
                            variant: variant.string,
                            value_id,
                            body,
                        }
                    })
                    .collect();

                // TODO: check for missing variants

                self.push_lowered(
                    None,
                    ExpressionKind::Switch {
                        value,
                        enum_,
                        cases,
                    },
                    return_type.unwrap_or_else(|| NamedType::never().into()),
                )
            }
        }
    }
    fn lower_identifier(&mut self, identifier: &AstString) -> LoweredExpression {
        let name = &identifier.string;
        if let Some((id, type_)) = self.lookup_local_identifier(identifier) {
            self.push_lowered(name.clone(), ExpressionKind::Reference(id), type_.clone())
        } else if let Some(named) = self.context.global_identifiers.get(name) {
            match named {
                Named::Assignment(id) => {
                    let id = *id;
                    self.global_assignment_dependencies.insert(id);
                    let type_ = self.context.assignments.get(&id).unwrap().type_.clone();
                    self.push_lowered(name.clone(), ExpressionKind::Reference(id), type_)
                }
                Named::Functions(_) => {
                    todo!("support function references");
                }
            }
        } else if let Some(type_parameter) = self.type_parameters.iter().find(|it| it.name == *name)
        {
            LoweredExpression::TypeParameterReference(type_parameter.type_())
        } else if self.context.hir.type_declarations.get(name).is_some() {
            LoweredExpression::NamedTypeReference(name.clone())
        } else {
            self.context.add_error(
                identifier.span.clone(),
                format!("Unknown reference: {name}"),
            );
            LoweredExpression::Error
        }
    }
    fn lower_call(
        &mut self,
        name: &AstString,
        type_arguments: Option<&AstTypeArguments>,
        arguments: &[(Id, Type)],
    ) -> LoweredExpression {
        let type_arguments = type_arguments.map(|it| {
            (
                it.arguments
                    .iter()
                    .map(|it| {
                        self.context
                            .lower_type(self.type_parameters, self.self_type, &it.type_)
                    })
                    .collect::<Box<_>>(),
                it.span.clone(),
            )
        });

        // TODO(lambdas): resolve local identifiers as well if not calling using instance syntax
        let matches = self
            .context
            .get_all_functions_matching_name(&name.string)
            .into_iter()
            .map(|id| {
                let (function, trait_) = self.context.get_function(id);
                (id, function.clone(), trait_.cloned())
            })
            .collect_vec();
        if matches.is_empty() {
            self.context.add_error(
                name.span.clone(),
                format!("Function `{}` not found", name.string),
            );
            return LoweredExpression::Error;
        }

        // Check type parameter count
        let matches = if let Some((type_arguments, type_arguments_span)) = &type_arguments {
            let (matches, mismatches) = matches.into_iter().partition::<Vec<_>, _>(|(_, it, _)| {
                it.type_parameters.len() == type_arguments.len()
            });
            if matches.is_empty() {
                self.context.add_error(
                    type_arguments_span.clone(),
                    format!(
                        "No overload accepts exactly {} {}:\n{}",
                        arguments.len(),
                        if arguments.len() == 1 {
                            "type argument"
                        } else {
                            "type arguments"
                        },
                        mismatches
                            .iter()
                            .map(|(_, it, _)| it.signature_to_string())
                            .join("\n"),
                    ),
                );
                return LoweredExpression::Error;
            }
            matches
        } else {
            matches
        };

        // TODO: show mismatches from previous steps

        // Check parameter count
        let matches = {
            let (matches, mismatches) = matches
                .into_iter()
                .partition::<Vec<_>, _>(|(_, it, _)| it.parameters.len() == arguments.len());
            if matches.is_empty() {
                self.context.add_error(
                    name.span.clone(),
                    format!(
                        "No overload accepts exactly {} {}:\n{}",
                        arguments.len(),
                        if arguments.len() == 1 {
                            "argument"
                        } else {
                            "arguments"
                        },
                        mismatches
                            .iter()
                            .map(|(_, it, _)| it.signature_to_string())
                            .join("\n"),
                    ),
                );
                return LoweredExpression::Error;
            }
            matches
        };

        // Check argument types
        let argument_types = arguments
            .iter()
            .map(|(_, type_)| type_.clone())
            .collect::<Box<_>>();
        let old_matches = matches;
        let mut matches = vec![];
        let mut mismatches = vec![];
        'outer: for (id, function, solver_rule) in old_matches {
            let mut type_solver = TypeSolver::new(&function.type_parameters);
            // Type arguments
            if let Some((type_arguments, _)) = &type_arguments {
                for (type_argument, type_parameter) in type_arguments
                    .iter()
                    .zip_eq(function.type_parameters.iter())
                {
                    match type_solver.unify(type_argument, &type_parameter.type_().into()) {
                        Ok(true) => {}
                        Ok(false) => unreachable!(),
                        Err(reason) => {
                            mismatches.push((id, function, Some(reason)));
                            continue 'outer;
                        }
                    };
                }
            }

            // Arguments
            for (argument_type, parameter) in
                argument_types.iter().zip_eq(function.parameters.iter())
            {
                match type_solver.unify(&Self::canonicalize_type(argument_type), &parameter.type_) {
                    Ok(true) => {}
                    Ok(false) => {
                        mismatches.push((id, function, None));
                        continue 'outer;
                    }
                    Err(reason) => {
                        mismatches.push((id, function, Some(reason)));
                        continue 'outer;
                    }
                };
            }

            match type_solver.finish() {
                Ok(substitutions) => matches.push((id, function, solver_rule, substitutions)),
                Err(error) => mismatches.push((id, function, Some(error))),
            }
        }

        if matches.is_empty() {
            self.context.add_error(
                name.span.clone(),
                format!(
                    "No matching function found for:\n  {}\n{}:{}",
                    FunctionDeclaration::call_signature_to_string(
                        mismatches.first().unwrap().1.name.as_ref(),
                        argument_types.as_ref()
                    ),
                    if mismatches.len() == 1 {
                        "This is the candidate function"
                    } else {
                        "These are candidate functions"
                    },
                    mismatches
                        .iter()
                        .map(|(_, it, reason)| format!(
                            "\n• {}{}",
                            it.signature_to_string(),
                            reason
                                .as_ref()
                                .map_or_else(String::new, |reason| format!(" ({reason})")),
                        ))
                        .join(""),
                ),
            );
            return LoweredExpression::Error;
        }

        // Solve traits
        let mut matches = {
            let old_matches = matches;
            let mut matches = vec![];
            let mut mismatches = vec![];
            for (id, function, trait_, substitutions) in old_matches {
                let Some(trait_) = trait_ else {
                    matches.push((id, function, None, substitutions));
                    continue;
                };

                let self_goal = substitutions
                    .get(&ParameterType::self_type())
                    .map(|self_type| {
                        trait_.solver_goal.substitute_all(&FxHashMap::from_iter([(
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

                let used_rule = self_goal
                    .iter()
                    .chain(trait_.solver_subgoals.iter())
                    .map(|subgoal| {
                        let solution = self.context.environment.solve(
                            &subgoal.substitute_all(&solver_substitutions),
                            &self
                                .type_parameters
                                .iter()
                                .filter_map(|it| it.clone().try_into().ok())
                                .collect::<Box<[SolverGoal]>>(),
                        );
                        match solution {
                            SolverSolution::Unique(solution) => Some(solution.used_rule),
                            SolverSolution::Ambiguous => {
                                // TODO: Add syntax to disambiguate trait function call on parameter types.
                                self.context.add_error(
                                    name.span.clone(),
                                    format!(
                                        "Function is reachable via different impls:\n{}",
                                        function.signature_to_string(),
                                    ),
                                );
                                None
                            }
                            SolverSolution::Impossible => None,
                        }
                    })
                    .collect::<Option<Vec<_>>>();
                if let Some(mut used_rule) = used_rule {
                    used_rule.truncate(1);
                    matches.push((
                        id,
                        function,
                        Some(used_rule.pop().unwrap().goal),
                        substitutions,
                    ));
                } else {
                    mismatches.push(function);
                }
            }
            if matches.is_empty() {
                // TODO: hide this error when there's an ambiguous solution
                self.context.add_error(
                    name.span.clone(),
                    format!(
                        "No function matches this signature:\n  {}\nThese are candidate functions:{}",
                        FunctionDeclaration::call_signature_to_string(
                            name.string.as_ref(),
                            argument_types.as_ref()
                        ),
                        mismatches
                            .iter()
                            .map(|it| format!("\n• {}", it.signature_to_string()))
                            .join(""),
                    ),
                );
                return LoweredExpression::Error;
            } else if matches.len() > 1 {
                self.context.add_error(
                    name.span.clone(),
                    format!(
                        "Multiple matching function found for:\n  {}\nThese are candidate functions:{}",
                        FunctionDeclaration::call_signature_to_string(
                            name.string.as_ref(),
                            argument_types.as_ref()
                        ),
                        matches
                            .iter()
                            .map(|(_, it, _, _)| format!("\n• {}", it.signature_to_string()))
                            .join(""),
                    ),
                );
                return LoweredExpression::Error;
            }
            matches
        };

        let (function, signature, used_goal, substitutions) = matches.pop().unwrap();
        let return_type = signature.return_type.substitute(&substitutions);
        self.push_lowered(
            None,
            ExpressionKind::Call {
                function,
                used_goal,
                substitutions,
                arguments: arguments.iter().map(|(id, _)| *id).collect(),
            },
            return_type,
        )
    }
    fn canonicalize_type(type_: &Type) -> Type {
        match type_ {
            Type::Named(named_type) => Self::canonicalize_named_type(named_type).into(),
            Type::Parameter(parameter_type) => NamedType {
                name: format!("${parameter_type}").into_boxed_str(),
                type_arguments: Box::default(),
            }
            .into(),
            Type::Self_ { base_type } => Type::Self_ {
                base_type: Self::canonicalize_named_type(base_type),
            },
            Type::Error => Type::Error,
        }
    }
    fn canonicalize_named_type(type_: &NamedType) -> NamedType {
        NamedType {
            name: type_.name.clone(),
            type_arguments: type_
                .type_arguments
                .iter()
                .map(Self::canonicalize_type)
                .collect(),
        }
    }

    fn push_panic(&mut self, message: impl Into<Box<str>>) {
        let message = self.push(
            None,
            ExpressionKind::Text(message.into()),
            NamedType::text(),
        );
        self.push(
            None,
            ExpressionKind::Call {
                function: BuiltinFunction::Panic.id(),
                used_goal: None,
                substitutions: FxHashMap::default(),
                arguments: vec![message].into_boxed_slice(),
            },
            NamedType::never(),
        );
    }

    fn push_lowered(
        &mut self,
        name: impl Into<Option<Box<str>>>,
        kind: ExpressionKind,
        type_: impl Into<Type>,
    ) -> LoweredExpression {
        let type_ = type_.into();
        let id = self.push(name, kind, type_.clone());
        LoweredExpression::Expression { id, type_ }
    }
    fn push_nothing(&mut self) -> Id {
        self.push(
            None,
            ExpressionKind::CreateStruct {
                struct_: Type::nothing(),
                fields: [].into(),
            },
            Type::nothing(),
        )
    }
    fn push_parameter(&mut self, parameter: Parameter) {
        self.local_identifiers
            .push((parameter.name, parameter.id, parameter.type_));
    }
    fn push_error(&mut self) -> Id {
        self.push(None, ExpressionKind::Error, Type::Error)
    }
    fn push(
        &mut self,
        name: impl Into<Option<Box<str>>>,
        kind: ExpressionKind,
        type_: impl Into<Type>,
    ) -> Id {
        let name = name.into();
        let type_ = type_.into();
        let id = self.context.id_generator.generate();
        if let Some(name) = &name {
            self.local_identifiers
                .push(((*name).clone(), id, type_.clone()));
        }
        self.body
            .expressions
            .push((id, name, Expression { kind, type_ }));
        id
    }

    #[must_use]
    fn lookup_local_identifier(&self, name: &str) -> Option<(Id, &Type)> {
        self.local_identifiers
            .iter()
            .rev()
            .find(|(box variable_name, _, _)| variable_name == name)
            .map(|(_, id, type_)| (*id, type_))
    }
}

#[derive(Debug)]
enum LoweredExpression {
    Expression { id: Id, type_: Type },
    NamedTypeReference(Box<str>),
    TypeParameterReference(ParameterType),
    EnumVariantReference { enum_: Type, variant: Box<str> },
    Error,
}
// #[derive(Debug)]
// enum LoweredIdentifier {
//     Expression { id: Id, type_: Type },
//     FunctionReferences(Box<[Id]>),
//     NamedTypeReference(Box<str>),
//     TypeParameterReference { name: Box<str>, id: TypeParameterId },
//     Error,
// }

pub struct TypeSolver<'h> {
    type_parameters: &'h [TypeParameter],
    substitutions: FxHashMap<ParameterType, Type>,
}
impl<'h> TypeSolver<'h> {
    #[must_use]
    pub fn new(type_parameters: &'h [TypeParameter]) -> Self {
        Self {
            type_parameters,
            substitutions: FxHashMap::default(),
        }
    }

    pub fn unify(&mut self, argument: &Type, parameter: &Type) -> Result<bool, Box<str>> {
        match (argument, parameter) {
            (Type::Error, _) | (_, Type::Error) => Ok(true),
            (_, Type::Parameter(parameter)) => {
                if let Some(mapped) = self.substitutions.get(parameter) {
                    if let Type::Parameter { .. } = mapped {
                        panic!("Type parameters can't depend on each other.")
                    }
                    let mapped = mapped.clone();
                    return self.unify(argument, &mapped);
                }

                assert!(
                    parameter.is_self_type()
                        || self
                            .type_parameters
                            .iter()
                            .any(|it| it.name == parameter.name),
                    "Unresolved type parameter: `{}`",
                    parameter.name
                );
                match self.substitutions.entry(parameter.clone()) {
                    Entry::Occupied(entry) => {
                        if !Context::is_assignable_to(entry.get(), argument) {
                            return Err(format!("Type parameter {} gets resolved to different types: `{}` and `{argument}`", parameter.name,entry.get()).into_boxed_str());
                        }
                    }
                    Entry::Vacant(entry) => {
                        entry.insert(argument.clone());
                    }
                }
                Ok(true)
            }
            (Type::Named(argument), Type::Named(parameter)) => {
                if argument.name != parameter.name
                    || argument.type_arguments.len() != parameter.type_arguments.len()
                {
                    return Ok(false);
                }

                for (argument, parameter) in argument
                    .type_arguments
                    .iter()
                    .zip_eq(parameter.type_arguments.iter())
                {
                    let result = self.unify(argument, parameter)?;
                    if !result {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
            (Type::Parameter { .. }, Type::Named { .. }) => Ok(true),
            (Type::Self_ { base_type }, _) => {
                self.unify(&Type::Named(base_type.clone()), parameter)
            }
            (_, Type::Self_ { base_type: _ }) => {
                self.unify(argument, &ParameterType::self_type().into())
            }
        }
    }

    pub fn finish(self) -> Result<FxHashMap<ParameterType, Type>, Box<str>> {
        for type_parameter in self.type_parameters {
            let type_ = type_parameter.type_();
            if !self.substitutions.contains_key(&type_) {
                return Err(format!(
                    "The type parameter `{type_}` can't be resolved to a specific type.",
                )
                .into_boxed_str());
            }
        }
        Ok(self.substitutions)
    }
}
