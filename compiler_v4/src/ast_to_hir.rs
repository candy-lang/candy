use crate::{
    ast::{
        Ast, AstArguments, AstAssignment, AstBody, AstCall, AstDeclaration, AstEnum, AstExpression,
        AstFunction, AstImpl, AstParameter, AstResult, AstStatement, AstStruct, AstSwitch,
        AstTextPart, AstTrait, AstType, AstTypeParameter, AstTypeParameters,
    },
    error::CompilerError,
    hir::{
        Assignment, Body, BodyOrBuiltin, BuiltinFunction, Expression, ExpressionKind, Function,
        Hir, Id, NamedType, Parameter, ParameterType, SwitchCase, Type, TypeDeclaration,
        TypeDeclarationKind, TypeParameter, TypeParameterId,
    },
    id::IdGenerator,
    position::Offset,
    type_solver::{
        goals::{Environment, SolverGoal, SolverRule, SolverSolution, SolverSolutionUnique},
        values::{canonical_variable, SolverType, SolverValue, SolverVariable},
    },
    utils::HashMapExtension,
};
use itertools::{Itertools, Position};
use petgraph::{
    algo::toposort,
    graph::{DiGraph, NodeIndex},
};
use rustc_hash::{FxHashMap, FxHashSet};
use std::{collections::hash_map::Entry, iter, ops::Range, path::Path};
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
    type_parameter_id_generator: IdGenerator<TypeParameterId>,
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
#[derive(Debug)]
struct TraitDeclaration<'a> {
    type_parameters: Box<[TypeParameter]>,
    functions: FxHashMap<Id, FunctionDeclaration<'a>>,
}
#[derive(Clone, Debug)]
struct ImplDeclaration<'a> {
    type_parameters: Box<[TypeParameter]>,
    type_: Type,
    trait_: Type,
    functions: FxHashMap<Id, FunctionDeclaration<'a>>,
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
    fn into_function(self) -> Function {
        Function {
            name: self.name,
            type_parameters: self.type_parameters,
            parameters: self.parameters,
            return_type: self.return_type,
            body: self.body.unwrap(),
        }
    }
}

impl<'a> Context<'a> {
    fn new(path: &'a Path, ast: &'a Ast) -> Self {
        Self {
            path,
            ast,
            id_generator: IdGenerator::start_at(BuiltinFunction::VARIANTS.len()),
            type_parameter_id_generator: IdGenerator::default(),
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
                        functions
                            .force_insert(id, self.functions.remove(&id).unwrap().into_function());
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
                    id: self.type_parameter_id_generator.generate(),
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
            match declaration {
                AstDeclaration::Struct(struct_) => self.lower_struct(struct_),
                AstDeclaration::Enum(enum_) => self.lower_enum(enum_),
                AstDeclaration::Trait(trait_) => self.lower_trait(trait_),
                AstDeclaration::Impl(impl_) => self.lower_impl(impl_),
                AstDeclaration::Assignment(assignment) => {
                    if let Some(id) = self.lower_assignment_signature(assignment) {
                        assignments_to_lower.push(id);
                    }
                }
                AstDeclaration::Function(function) => {
                    if let Some((id, function)) = self.lower_function_signature(&[], function) {
                        self.functions.force_insert(id, function);
                        functions_to_lower.push(id);
                    }
                }
            }
        }

        self.environment = Environment {
            rules: self
                .impls
                .clone()
                .iter()
                .flat_map(|it| self.impl_to_solver_rules(it).into_iter())
                .collect(),
        };
        println!("{}", self.environment);

        for trait_ in self.traits.keys().cloned().collect_vec() {
            for (id, mut function) in self.traits[&trait_].functions.clone() {
                self.lower_function(&mut function);
                self.traits
                    .get_mut(&trait_)
                    .unwrap()
                    .functions
                    .insert(id, function)
                    .unwrap();
            }
        }
        for index in 0..self.impls.len() {
            for (id, mut function) in self.impls[index].functions.clone() {
                self.lower_function(&mut function);
                self.impls[index].functions.insert(id, function).unwrap();
            }
        }
        for id in assignments_to_lower {
            self.lower_assignment(id);
        }
        for id in functions_to_lower {
            let mut function = self.functions.get(&id).unwrap().clone();
            self.lower_function(&mut function);
            self.functions.insert(id, function).unwrap();
        }
    }

    fn lower_struct(&mut self, struct_type: &'a AstStruct) {
        let Some(name) = struct_type.name.value() else {
            return;
        };

        let type_parameters = self.lower_type_parameters(&[], struct_type.type_parameters.as_ref());

        let fields = struct_type
            .fields
            .iter()
            .filter_map(|field| {
                let name = field.name.value()?;

                let type_ = self.lower_type(&type_parameters, field.type_.value());
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

        let type_parameters = self.lower_type_parameters(&[], enum_type.type_parameters.as_ref());

        let variants = enum_type
            .variants
            .iter()
            .filter_map(|variant| {
                let name = variant.name.value()?;

                let type_ = variant
                    .type_
                    .as_ref()
                    .map(|it| self.lower_type(&type_parameters, it.value()));
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

    fn lower_trait(&mut self, trait_: &'a AstTrait) {
        let Some(name) = trait_.name.value() else {
            return;
        };

        let type_parameters = self.lower_type_parameters(&[], trait_.type_parameters.as_ref());

        let functions = self.lower_function_signatures(&type_parameters, &trait_.functions);

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
                    functions,
                });
            }
        };
    }
    fn lower_impl(&mut self, impl_: &'a AstImpl) {
        let type_parameters = self.lower_type_parameters(&[], impl_.type_parameters.as_ref());

        let Some(type_) = impl_.type_.value() else {
            return;
        };
        let type_ = self.lower_type(&type_parameters, type_);

        let Some(trait_) = impl_.trait_.value() else {
            return;
        };
        let trait_ = self.lower_type(&type_parameters, trait_);

        let functions = self.lower_function_signatures(&type_parameters, &impl_.functions);

        self.impls.push(ImplDeclaration {
            type_parameters,
            type_,
            trait_,
            functions,
        });
    }

    fn lower_type_parameters(
        &mut self,
        outer_type_parameters: &[TypeParameter],
        type_parameters: Option<&AstTypeParameters>,
    ) -> Box<[TypeParameter]> {
        type_parameters.map_or_else(Box::default, |it| {
            it.parameters
                .iter()
                .filter_map(|it| {
                    let name = it.name.value()?;
                    let id = self.type_parameter_id_generator.generate();
                    let upper_bound = it
                        .upper_bound
                        .as_ref()
                        .and_then(|it| it.value())
                        .map(|it| Box::new(self.lower_type(outer_type_parameters, it)));
                    Some(TypeParameter {
                        id,
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
        type_: impl Into<Option<&AstType>>,
    ) -> Type {
        let type_: Option<&AstType> = type_.into();
        let Some(type_) = type_ else {
            return Type::Error;
        };

        let Some(name) = type_.name.value() else {
            return Type::Error;
        };

        if let Some((name, id)) = Self::resolve_type_parameter(type_parameters, &name.string) {
            if let Some(type_arguments) = &type_.type_arguments {
                self.add_error(
                    type_arguments.span.clone(),
                    "Type parameters can't have type arguments",
                );
            }
            return ParameterType { name, id }.into();
        }

        let type_arguments = type_
            .type_arguments
            .as_ref()
            .map_or_else(Box::default, |it| {
                it.arguments
                    .iter()
                    .map(|it| self.lower_type(type_parameters, &it.type_))
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

        let Some(type_parameters) = self.ast.iter().find_map(|it| match it {
            AstDeclaration::Struct(AstStruct {
                name: it_name,
                type_parameters,
                ..
            })
            | AstDeclaration::Enum(AstEnum {
                name: it_name,
                type_parameters,
                ..
            })
            | AstDeclaration::Trait(AstTrait {
                name: it_name,
                type_parameters,
                ..
            }) if it_name.value().map(|it| &it.string) == Some(&name.string) => Some(
                type_parameters
                    .as_ref()
                    .map_or::<&[AstTypeParameter], _>(&[], |it| &it.parameters),
            ),
            _ => None,
        }) else {
            self.add_error(name.span.clone(), format!("Unknown type: `{}`", **name));
            return Type::Error;
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
    fn resolve_type_parameter(
        type_parameters: &[TypeParameter],
        name: &str,
    ) -> Option<(Box<str>, TypeParameterId)> {
        type_parameters
            .iter()
            .find(|it| &*it.name == name)
            .map(|it| (it.name.clone(), it.id))
    }

    fn lower_assignment_signature(&mut self, assignment: &'a AstAssignment) -> Option<Id> {
        let name = assignment.name.value()?;

        let id = self.id_generator.generate();
        // TODO: infer type
        let type_ = assignment
            .type_
            .as_ref()
            .map_or(Type::Error, |it| self.lower_type(&[], it.value()));

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

        let (hir_body, global_assignment_dependencies) = BodyBuilder::build(self, &[], |builder| {
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
        functions: &'a [AstFunction],
    ) -> FxHashMap<Id, FunctionDeclaration<'a>> {
        functions
            .iter()
            .filter_map(|function| self.lower_function_signature(outer_type_parameters, function))
            .collect()
    }
    fn lower_function_signature(
        &mut self,
        outer_type_parameters: &[TypeParameter],
        function: &'a AstFunction,
    ) -> Option<(Id, FunctionDeclaration<'a>)> {
        let name = function.name.value()?;

        let id = self.id_generator.generate();

        let type_parameters =
            self.lower_type_parameters(outer_type_parameters, function.type_parameters.as_ref());
        let all_type_parameters = outer_type_parameters
            .iter()
            .chain(type_parameters.iter())
            .cloned()
            .collect::<Box<_>>();

        let parameters = self.lower_parameters(&all_type_parameters, &function.parameters);
        let return_type = function.return_type.as_ref().map_or_else(
            || NamedType::nothing().into(),
            |it| self.lower_type(&all_type_parameters, it),
        );
        match self.global_identifiers.entry(name.string.clone()) {
            Entry::Occupied(mut entry) => match entry.get_mut() {
                Named::Functions(functions) => {
                    // TODO: check for invalid overloads
                    functions.push(id);
                }
                Named::Assignment(_) => {
                    self.add_error(
                        name.span.clone(),
                        "A top-level function can't have the same name as a top-level assignment.",
                    );
                    return None;
                }
            },
            Entry::Vacant(entry) => {
                entry.insert(Named::Functions(vec![id]));
            }
        }
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

                let type_ = self.lower_type(type_parameters, parameter.type_.value());

                let id = self.id_generator.generate();
                Parameter {
                    id,
                    name: name.string,
                    type_,
                }
            })
            .collect()
    }
    fn lower_function(&mut self, function: &mut FunctionDeclaration<'a>) {
        let (hir_body, _) = BodyBuilder::build(self, &function.type_parameters, |builder| {
            for parameter in function.parameters.iter() {
                builder.push_parameter(parameter.clone());
            }

            builder.lower_statements(&function.ast.unwrap().body, Some(&function.return_type));
        });

        function.body = Some(BodyOrBuiltin::Body(hir_body));
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
            (Type::Parameter(from), Type::Parameter(to)) => from.id == to.id,
            // TODO: Self type
            _ => false,
        }
    }

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

    fn impl_to_solver_rules(&mut self, impl_: &ImplDeclaration<'a>) -> FxHashSet<SolverRule> {
        // Lower the constraints. For example, in the impl `impl[T: Equals] Foo[T]: Equals`, the
        // `solver_constraints` are a list containing `Equals(?T)`.
        let mut rules = FxHashSet::default();
        let mut solver_constraints = vec![];
        for type_parameter in impl_.type_parameters.iter() {
            if let Some(upper_bound) = &type_parameter.upper_bound {
                let (solver_type, goals, new_rules) =
                    Self::trait_to_solver_type_and_goals_and_rules(upper_bound);
                rules.extend(new_rules.into_iter());

                let SolverType::Variable(solver_variable) = solver_type else {
                    panic!();
                };
                let substitution = FxHashMap::from_iter([(
                    solver_variable,
                    SolverVariable::new(type_parameter.type_()).into(),
                )]);
                solver_constraints
                    .extend(goals.into_iter().map(|it| it.substitute_all(&substitution)));
            };
        }

        // Lower the base type. For example, in the impl `impl Iterable[Int]: Foo`, the base type
        // `Iterable[Int]` gets lowered to `?0` with the goal `Iterable(Int, ?0)`.
        let solver_base = Self::type_to_solver_type(&impl_.type_);

        // Lower the trait. For example, in the impl `impl Foo: Iterable[Int]`, the implemented trait
        // `Iterable[Int]` gets lowered to `?0` with the goals `Iterable(Int, ?0)`. Impls that don't
        // implement a trait – like `impl Foo { ... }` – don't correspond to a `SolverRule` and cause
        // this function to return `None`.
        // let trait_ = match &impl_.trait_ {
        //     Type::Named(named_type) => named_type,
        //     Type::Parameter(_) | Type::Self_ { .. } | Type::Error => return FxHashSet::default(),
        // };
        let (solver_trait, mut trait_goals, new_rules) =
            Self::trait_to_solver_type_and_goals_and_rules(&impl_.trait_);
        let SolverType::Variable(solver_trait) = solver_trait else {
            panic!("Traits didn't get lowered to `SolverVariable`s.");
        };

        rules.extend(new_rules);
        if trait_goals.len() > 1 {
            // TODO(never, marcelgarus): We can't implement a trait that contains another trait yet, like
            // `Iterable<Equals>`. This does work in Rust (with boxing and explicit dynamism), so we'll
            // probably have to look at how to put general-purpose logic implications in our solver (like
            // `A B -> C D` instead of only having one implicative result like `A B -> C` in the solver).
            // Or we need to somehow reduce this to multiple simple rules.
            // For now, we're probably fine with only implementing "simple" traits like `Iterable<Int>`.
            // (As "primitive" types like `List` etc. are also traits, resolving this todo is somewhat of
            // a priority.)
            self.add_error(
                Offset(0)..Offset(0),
                format!("Can't implement trait of trait: {impl_:?}."),
            );
            return FxHashSet::default();
        }
        assert_eq!(trait_goals.len(), 1);
        let trait_goal = trait_goals.pop().unwrap();

        // Let's play this through for the impl `impl[T: Equals] Iterable[T]: Equals`.
        //
        // These would be the values calculated above:
        //
        // * `constraints`: list with goal `Equals(?T)`
        // * `solverBase`: type `?0` and goal `Iterable(?T, ?0)`
        // * `solverTrait`: type `?1` and goal `Equals(?1)`
        //
        // The goal that this impl wants to achieve can be calculated by taking the `solverTrait`, which
        // is guaranteed to have only one goal `Equals(?1)`, and replacing the `solverTrait`'s type `?1`
        // with the `solverBase` type `?0` – which results in `Equals(?0)`.
        // To achieve that goal, we have to satisfy the goals of the `solverBase` and the `constraints`.
        // So, our total `SolverRule` would look like this: `Equals(?0) <- Iterable(?T, ?0), Equals(?T)`
        let substitution = FxHashMap::from_iter([(solver_trait, solver_base)]);
        rules.insert(SolverRule {
            goal: trait_goal.substitute_all(&substitution),
            subgoals: solver_constraints.into_boxed_slice(),
        });
        rules
    }

    fn find_unique_solver_solution_for(
        &mut self,
        base: &Type,
        trait_: &Type,
    ) -> Option<SolverSolutionUnique> {
        assert!(
            trait_ != &Type::Error,
            "Can't reveal the impl for the error type",
        );

        let base_type = Self::type_to_solver_type(base);

        let (trait_type, mut trait_goals, _) =
            Self::trait_to_solver_type_and_goals_and_rules(trait_);
        let SolverType::Variable(trait_type) = trait_type else {
            panic!("This shouldn't happen. Trait should be lowered to a SolverVariable.");
        };
        if trait_goals.len() != 1 {
            self.add_error(
                Offset(0)..Offset(0),
                "Trying to find impl for trait with trait as parameter",
            );
            return None;
        }
        let trait_goal = trait_goals.pop().unwrap();

        let solution = self.environment.solve(
            trait_goal.substitute_all(&FxHashMap::from_iter([(trait_type, base_type)])),
            &[],
        );
        match solution {
            SolverSolution::Unique(solution) => Some(solution),
            SolverSolution::Ambiguous | SolverSolution::Impossible => None,
        }
    }

    /// Turns a trait type into a [`SolverType`] and a list of [`SolverGoal`]s,
    /// as well as a set of `SolverRule`s.
    fn trait_to_solver_type_and_goals_and_rules(
        type_: &Type,
    ) -> (SolverType, Vec<SolverGoal>, FxHashSet<SolverRule>) {
        let mut lowering_context = TypeLoweringContext::default();
        let (solver_type, goals) = lowering_context.trait_to_solver_type_and_goals(type_);
        (solver_type.into(), goals, lowering_context.rules)
    }
    /// Turns a concrete [Type] (i.e., no trait) into a [`SolverType`].
    fn type_to_solver_type(type_: &Type) -> SolverType {
        match type_ {
            Type::Named(type_) => SolverValue {
                type_: type_.name.clone(),
                parameters: type_
                    .type_arguments
                    .iter()
                    .map(Self::type_to_solver_type)
                    .collect(),
            }
            .into(),
            Type::Self_ { .. } => todo!(),
            Type::Parameter(type_) => SolverVariable::new(type_.clone()).into(),
            Type::Error => SolverVariable::error().into(),
        }
    }

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
    local_identifiers: Vec<(Box<str>, Id, Type)>,
    body: Body,
}
impl<'c, 'a> BodyBuilder<'c, 'a> {
    #[must_use]
    fn build(
        context: &'c mut Context<'a>,
        type_parameters: &'c [TypeParameter],
        fun: impl FnOnce(&mut BodyBuilder),
    ) -> (Body, FxHashSet<Id>) {
        let mut builder = Self {
            context,
            global_assignment_dependencies: FxHashSet::default(),
            type_parameters,
            local_identifiers: vec![],
            body: Body::default(),
        };
        fun(&mut builder);
        (builder.body, builder.global_assignment_dependencies)
    }
    #[must_use]
    fn build_inner(&mut self, fun: impl FnOnce(&mut BodyBuilder)) -> Body {
        BodyBuilder::build(self.context, self.type_parameters, |builder| {
            builder.local_identifiers = self.local_identifiers.clone();
            fun(builder);
            self.global_assignment_dependencies
                .extend(&builder.global_assignment_dependencies);
        })
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

                    let type_ = assignment
                        .type_
                        .as_ref()
                        .map(|it| self.context.lower_type(self.type_parameters, it.value()));

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
            LoweredExpression::FunctionReferences { .. } => {
                // TODO: report actual error location
                self.context
                    .add_error(Offset(0)..Offset(0), "Function must be called.");
                (self.push_error(), Type::Error)
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

                let name = &identifier.string;
                if let Some((id, type_)) = self.lookup_local_identifier(identifier) {
                    self.push_lowered(None, ExpressionKind::Reference(id), type_.clone())
                } else if let Some(named) = self.context.global_identifiers.get(name) {
                    match named {
                        Named::Assignment(id) => {
                            let id = *id;
                            self.global_assignment_dependencies.insert(id);
                            let type_ = self.context.assignments.get(&id).unwrap().type_.clone();
                            self.push_lowered(None, ExpressionKind::Reference(id), type_)
                        }
                        Named::Functions(function_ids) => {
                            assert!(!function_ids.is_empty());
                            LoweredExpression::FunctionReferences {
                                receiver: None,
                                function_ids: function_ids.iter().copied().collect(),
                            }
                        }
                    }
                } else if let Some((name, id)) =
                    Context::resolve_type_parameter(self.type_parameters, name)
                {
                    LoweredExpression::TypeParameterReference { name, id }
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
                                type_arguments: Box::default(),
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
                    parameter_types: &[Type],
                ) -> Option<Box<[Id]>> {
                    let arguments = arguments
                        .arguments_or_default()
                        .iter()
                        .enumerate()
                        .map(|(index, argument)| {
                            builder
                                .lower_expression(&argument.value, parameter_types.get(index))
                                .0
                        })
                        .collect::<Box<_>>();
                    if arguments.len() == parameter_types.len() {
                        Some(arguments)
                    } else {
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
                        None
                    }
                }

                let receiver = self.lower_expression_raw(&call.receiver, None);

                match receiver {
                    LoweredExpression::Expression { .. } => {
                        // TODO: report actual error location
                        self.context
                            .add_error(Offset(0)..Offset(0), "Cannot call this type");
                        LoweredExpression::Error
                    }
                    LoweredExpression::FunctionReferences {
                        receiver,
                        function_ids,
                    } => {
                        assert!(!function_ids.is_empty());

                        let type_arguments = call.type_arguments.as_ref().map(|it| {
                            it.arguments
                                .iter()
                                .map(|it| self.context.lower_type(self.type_parameters, &it.type_))
                                .collect::<Box<_>>()
                        });

                        let arguments = receiver
                            .into_iter()
                            .chain(
                                call.arguments
                                    .arguments_or_default()
                                    .iter()
                                    .map(|argument| self.lower_expression(&argument.value, None)),
                            )
                            .collect::<Box<_>>();

                        let matches = function_ids
                            .iter()
                            .map(|id| (*id, &self.context.functions[id]))
                            .collect_vec();

                        // Check type parameter count
                        let matches = if let Some(type_arguments) = &type_arguments {
                            let (matches, mismatches) =
                                matches.iter().partition::<Vec<_>, _>(|(_, it)| {
                                    it.type_parameters.len() == type_arguments.len()
                                });
                            if matches.is_empty() {
                                self.context.add_error(
                                    call.type_arguments.as_ref().unwrap().span.clone(),
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
                                            .map(|(_, it)| it.signature_to_string())
                                            .join("\n"),
                                    ),
                                );
                                return LoweredExpression::Error;
                            }
                            matches
                        } else {
                            matches
                        };

                        // TODO: report actual error location
                        let arguments_start_span =
                            call.arguments.value().map_or(Offset(0)..Offset(0), |it| {
                                it.opening_parenthesis_span.clone()
                            });

                        // Check parameter count
                        let matches = {
                            let (matches, mismatches) =
                                matches.iter().partition::<Vec<_>, _>(|(_, it)| {
                                    it.parameters.len() == arguments.len()
                                });
                            if matches.is_empty() {
                                self.context.add_error(
                                    arguments_start_span,
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
                                            .map(|(_, it)| it.signature_to_string())
                                            .join("\n"),
                                    ),
                                );
                                return LoweredExpression::Error;
                            }
                            matches
                        };

                        // Check argument types
                        // FIXME: Unify types
                        let argument_types = arguments
                            .iter()
                            .map(|(_, type_)| type_.clone())
                            .collect::<Box<_>>();
                        let old_matches = matches;
                        let mut matches = vec![];
                        let mut mismatches = vec![];
                        'outer: for (id, function) in old_matches {
                            let mut type_solver = TypeSolver::new(&function.type_parameters);
                            // Type arguments
                            if let Some(type_arguments) = &type_arguments {
                                for (type_argument, type_parameter) in type_arguments
                                    .iter()
                                    .zip_eq(function.type_parameters.iter())
                                {
                                    match type_solver.unify(
                                        type_argument,
                                        &ParameterType {
                                            name: type_parameter.name.clone(),
                                            id: type_parameter.id,
                                        }
                                        .into(),
                                    ) {
                                        Ok(true) => {}
                                        Ok(false) => unreachable!(),
                                        Err(reason) => {
                                            mismatches.push((id, function, Some(reason)));
                                            break 'outer;
                                        }
                                    };
                                }
                            }

                            // Arguments
                            for (argument_type, parameter) in
                                argument_types.iter().zip_eq(function.parameters.iter())
                            {
                                match type_solver.unify(argument_type, &parameter.type_) {
                                    Ok(true) => {}
                                    Ok(false) => {
                                        mismatches.push((id, function, None));
                                        break 'outer;
                                    }
                                    Err(reason) => {
                                        mismatches.push((id, function, Some(reason)));
                                        break 'outer;
                                    }
                                };
                            }

                            match type_solver.finish() {
                                Ok(environment) => matches.push((id, function, environment)),
                                Err(error) => mismatches.push((id, function, Some(error))),
                            }
                        }

                        if matches.is_empty() {
                            self.context.add_error(
                                arguments_start_span,
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
                                                .map_or_else(String::new, |reason| format!(
                                                    " ({reason})"
                                                )),
                                        ))
                                        .join(""),
                                ),
                            );
                            return LoweredExpression::Error;
                        } else if matches.len() > 1 {
                            self.context.add_error(
                                arguments_start_span,
                                format!(
                                    "Multiple matching function found for:\n  {}\nThese are candidate functions:{}",
                                    FunctionDeclaration::call_signature_to_string(matches.first().unwrap().1.name.as_ref(), argument_types.as_ref()),
                                    matches
                                        .iter()
                                        .map(|(_,it,_)| format!("\n• {}", it.signature_to_string()))
                                        .join(""),
                                ),
                            );
                            return LoweredExpression::Error;
                        }

                        let (function, signature, environment) = matches.pop().unwrap();
                        self.push_lowered(
                            None,
                            ExpressionKind::Call {
                                function,
                                type_arguments: signature
                                    .type_parameters
                                    .iter()
                                    .map(|it| environment.get(&it.id).unwrap().clone())
                                    .collect(),
                                arguments: arguments.iter().map(|(id, _)| *id).collect(),
                            },
                            signature.return_type.substitute(&environment),
                        )
                        // let parameter_types = function
                        //     .parameters
                        //     .iter()
                        //     .map(|it| it.type_.clone())
                        //     .collect_vec();
                        // let return_type = function.return_type.clone();

                        //   if full_matches.is_empty() then return error[LookupFunSolution, Str]({
                        //     var out = string_builder().&
                        //     out.
                        //       "This call doesn't work:{newline}
                        //       ' > {call_signature(name, type_args, arg_types)}{newline}{newline}"
                        //     if name_matches.is_empty()
                        //     then out.'"There are no defintions named "{{name}}"."'
                        //     else {
                        //       out."These definitions have the same name, but arguments don't match:"
                        //       for match in name_matches do
                        //         out."{newline} - {AstDef.fun_(match).signature()}"
                        //     }
                        //     out.to_str()
                        //   })
                        //   if full_matches.len.is_greater_than(1) then return error[LookupFunSolution, Str]({
                        //     var out = string_builder().&
                        //     out.
                        //       "This call doesn't work:{newline}
                        //       ' > {call_signature(name, type_args, arg_types)}{newline}{newline}
                        //       'Multiple definitions match:"
                        //     for match in full_matches do {
                        //       var padded_signature = "{AstDef.fun_(match.fun_).signature()}"
                        //         .pad_right(30, # )
                        //       out."{newline} - {padded_signature}"
                        //       if match.type_env.is_not_empty() then {
                        //         out." with "
                        //         var first = true
                        //         for entry in match.type_env do {
                        //           if first then first = false else out.", "
                        //           out."{entry.key} = {entry.value}"
                        //         }
                        //       }
                        //     }
                        //     out.to_str()
                        //   })
                        //   ok[LookupFunSolution, Str](full_matches.get(0))
                    }
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
                                    &fields.iter().map(|(_, type_)| type_.clone()).collect_vec(),
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
                                            fields,
                                        },
                                        type_,
                                    )
                                })
                            }
                            Some(TypeDeclaration {
                                type_parameters,
                                kind: TypeDeclarationKind::Enum { .. },
                            }) => {
                                // TODO: report actual error location
                                self.context
                                    .add_error(Offset(0)..Offset(0), "Enum variant is missing.");
                                LoweredExpression::Error
                            }
                            Some(TypeDeclaration {
                                type_parameters,
                                kind: TypeDeclarationKind::Trait { .. },
                            }) => unreachable!(),
                            None => {
                                // TODO: report actual error location
                                self.context.add_error(
                                    Offset(0)..Offset(0),
                                    format!("Can't instantiate builtin type {type_} directly."),
                                );
                                LoweredExpression::Error
                            }
                        }
                    }
                    LoweredExpression::TypeParameterReference { name, .. } => {
                        // TODO: report actual error location
                        self.context.add_error(
                            Offset(0)..Offset(0),
                            format!("Can't instantiate type parameter {name} directly."),
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
                        let variant_type = variant_type.substitute(&Type::build_environment(
                            &enum_.type_parameters,
                            &enum_named_type.type_arguments,
                        ));
                        let parameter_types = [variant_type];
                        let arguments = lower_arguments(
                            self,
                            call,
                            &call.arguments,
                            parameter_types.as_slice(),
                        );
                        arguments.map_or(LoweredExpression::Error, |arguments| {
                            self.push_lowered(
                                None,
                                ExpressionKind::CreateEnum {
                                    enum_: enum_type.clone(),
                                    variant,
                                    value: arguments.first().copied(),
                                },
                                enum_type,
                            )
                        })
                    }
                    LoweredExpression::Error => LoweredExpression::Error,
                }
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

                            // TODO: merge with global function resolution
                            if let Some(Named::Functions(function_ids)) =
                                self.context.global_identifiers.get(&key.string)
                            {
                                let function_ids = function_ids
                                    .iter()
                                    .map(|id| (*id, &self.context.functions[id]))
                                    .filter(|(_, it)| {
                                        !it.parameters.is_empty()
                                            && it.parameters[0].type_ == receiver_type
                                    })
                                    .map(|(id, _)| id)
                                    .collect::<Box<_>>();
                                if !function_ids.is_empty() {
                                    return LoweredExpression::FunctionReferences {
                                        receiver: Some((receiver_id, receiver_type.clone())),
                                        function_ids,
                                    };
                                }
                            }

                            self.context.add_error(
                                key.span.clone(),
                                format!(
                                    "Value of type `{receiver_type:?}` doesn't have a function or field `{}`",
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
                    LoweredExpression::FunctionReferences { .. } => {
                        self.context.add_error(
                            key.span.clone(),
                            format!("Function doesn't have a field `{}`", key.string),
                        );
                        LoweredExpression::Error
                    }
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
                            TypeDeclarationKind::Trait { .. } => unreachable!(),
                        }
                    }
                    LoweredExpression::TypeParameterReference { name, .. } => {
                        self.context.add_error(
                            key.span.clone(),
                            format!(
                                "Parameter type `{name:?}` doesn't have a field `{}`",
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
                                    .map(|it| it.id)
                                    .zip_eq(type_.type_arguments.iter().cloned())
                                    .collect::<FxHashMap<_, _>>(),
                                variants.clone(),
                            ),
                            TypeDeclarationKind::Trait { .. } => unreachable!(),
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
    Expression {
        id: Id,
        type_: Type,
    },
    FunctionReferences {
        receiver: Option<(Id, Type)>,
        function_ids: Box<[Id]>,
    },
    NamedTypeReference(Box<str>),
    TypeParameterReference {
        name: Box<str>,
        id: TypeParameterId,
    },
    EnumVariantReference {
        enum_: Type,
        variant: Box<str>,
    },
    Error,
}

struct TypeSolver<'h> {
    type_parameters: &'h [TypeParameter],
    environment: FxHashMap<TypeParameterId, Type>,
}
impl<'h> TypeSolver<'h> {
    #[must_use]
    fn new(type_parameters: &'h [TypeParameter]) -> Self {
        Self {
            type_parameters,
            environment: FxHashMap::default(),
        }
    }

    fn unify(&mut self, argument: &Type, parameter: &Type) -> Result<bool, Box<str>> {
        match (argument, parameter) {
            (Type::Error, _) | (_, Type::Error) => Ok(true),
            (_, Type::Parameter(parameter)) => {
                if let Some(mapped) = self.environment.get(&parameter.id) {
                    if let Type::Parameter { .. } = mapped {
                        panic!("Type parameters can't depend on each other.")
                    }
                    let mapped = mapped.clone();
                    return self.unify(argument, &mapped);
                }

                assert!(
                    self.type_parameters.iter().any(|it| it.id == parameter.id),
                    "Unresolved type parameter: `{}`",
                    parameter.name
                );
                match self.environment.entry(parameter.id) {
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
            (_, _) => todo!(), // TODO: Self type
        }
    }

    fn finish(self) -> Result<FxHashMap<TypeParameterId, Type>, Box<str>> {
        for type_parameter in self.type_parameters {
            if !self.environment.contains_key(&type_parameter.id) {
                return Err(format!(
                    "The type parameter `{}` can't be resolved to a specific type.",
                    &type_parameter.name
                )
                .into_boxed_str());
            }
        }
        Ok(self.environment)
    }
}

#[derive(Default)]
struct TypeLoweringContext {
    next_canonical_index: usize,
    rules: FxHashSet<SolverRule>,
}
impl TypeLoweringContext {
    fn get_next_canonical_variable(&mut self) -> ParameterType {
        let result = canonical_variable(self.next_canonical_index);
        self.next_canonical_index += 1;
        result
    }

    fn trait_to_solver_type_and_goals(
        &mut self,
        type_: &Type,
    ) -> (SolverVariable, Vec<SolverGoal>) {
        match type_ {
            Type::Named(type_) => {
                // Example:
                //
                // trait_to_solver_type_and_goals(Foo[String, Int]) == (
                //   ?0,
                //   Foo(?0, ?1, ?2), String(?1), Int(?2),
                //   [
                //     Foo($Foo[?1, ?2], ?1, ?2) <- <empty>,
                //     Foo($Never, ?1, ?2) <- <empty>,
                //     Foo(?0, ?1, ?2) <- Error(?0),
                //     ## Generated transitively:
                //     String($String) <- <empty>,
                //     String($Never) <- <empty>,
                //     String(?0) <- Error(?0),
                //     Int($Int) <- <empty>,
                //     Int($Never) <- <empty>,
                //     Int(?0) <- Error(?0),
                //     Error(?0) <- <empty>,
                //     Any(?0) <- <empty>,
                //   ],
                // )

                let mut solver_parameters = vec![];
                let mut goals = vec![];

                for type_argument in type_.type_arguments.iter() {
                    let (type_argument, mut new_goals) =
                        self.trait_to_solver_type_and_goals(type_argument);
                    solver_parameters.push(type_argument.into());
                    goals.append(&mut new_goals);
                }

                // TODO: move this above the generics
                // We'll return a substitution that will have to satisfy the trait. For example, given the
                // trait `Equals`, we'll return `?0` with the goal `Equals(?0)`. The `?0` is this
                // substitution.
                let substitution = SolverVariable::new(self.get_next_canonical_variable());
                solver_parameters.push(substitution.clone().into());
                goals.push(SolverGoal {
                    trait_: type_.name.clone(),
                    parameters: solver_parameters.into_boxed_slice(),
                });

                (substitution, goals)
            }
            Type::Self_ { base_type } => {
                // TODO
                // if (declaration is HirTrait) {
                //     // We don't need to lower `Self` types in traits to solver types because we only use impls
                //     // for logical type solving.
                //     throw "Don't call hirTypeToSolverType for a Self type in a trait"
                // }

                // If a `Self` type is used inside an impl, it just assumes the solver type of the base
                // type.
                //
                // Whether that solver type is a `SolverVariable` or `SolverValue` depends on whether the
                // impl is for a trait or type:
                //
                // * `impl Int: InfixAmpersand[Self, Int]` has the lowered base type `Int`, so the `Self`
                //   gets replaced with `Int`, resulting in `InfixAmpersand(Int, Bool)`.
                // * `impl And: InfixAmpersand[Self, Bool]` has the lowered base type `?0` with the
                //   additional goal `And(?0)`, so the `Self` gets replaced with `?0`, resulting in
                //   `InfixAmpersand(?0, Bool) <- And(?0)`.
                let (base_type, _) = self.trait_to_solver_type_and_goals(&base_type.clone().into());
                (base_type, vec![])
            }
            Type::Parameter(type_) => (SolverVariable::new(type_.clone()), vec![]),
            Type::Error => (SolverVariable::error(), vec![]),
        }
    }
}
