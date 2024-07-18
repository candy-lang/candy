use crate::{
    ast::{
        AstAssignment, AstAssignmentFunction, AstAssignmentKind, AstAssignmentValue, AstExpression,
        AstStatement, AstTextPart,
    },
    error::CompilerError,
    hir::{
        Body, BuiltinFunction, Definition, Expression, Hir, Id, OrType, Parameter, TagType, Type,
    },
    id::IdGenerator,
    position::Offset,
    utils::HashMapExtension,
};
use itertools::{Itertools, Position};
use rustc_hash::{FxHashMap, FxHashSet};
use std::{
    borrow::Cow,
    collections::{hash_map::Entry, BTreeSet},
    mem,
    ops::Range,
    path::Path,
};
use strum::VariantArray;

pub fn ast_to_hir(path: &Path, ast: &[AstAssignment]) -> (Hir, Vec<CompilerError>) {
    let mut context = Context::new(path);
    context.add_builtin_value("type", Expression::Type(Type::Type), Type::Type);
    context.add_builtin_value("int", Expression::Type(Type::Int), Type::Type);
    context.add_builtin_value("text", Expression::Type(Type::Text), Type::Type);
    context.add_builtin_functions();

    context.catalog_assignments(ast);
    context.lower_assignments();

    let mut identifiers: FxHashMap<_, _> = context
        .global_identifiers
        .iter()
        .flat_map(|(name, named)| match named {
            Named::Value(id) => vec![(*id, name.clone())],
            Named::Functions(functions) => {
                functions.iter().map(|id| (*id, name.clone())).collect_vec()
            }
        })
        .collect();

    let mut hir = Hir::default();
    for (id, definition) in mem::take(&mut context.definitions) {
        let identifier = identifiers.remove(&id).unwrap();
        match definition {
            TempDefinition::Value(ValueDefinition { definition, .. }) => {
                let (type_, value) = definition.expect("Missing definition");
                hir.assignments.push((
                    id,
                    identifier,
                    Definition::Value {
                        type_,
                        value: value.unwrap(),
                    },
                ));
            }
            TempDefinition::Function(FunctionDefinition {
                signature_and_body, ..
            }) => {
                let (parameters, return_type, body) =
                    signature_and_body.expect("Missing signature and body");
                hir.assignments.push((
                    id,
                    identifier.clone(),
                    Definition::Function {
                        parameters,
                        return_type,
                        body: body.expect("Missing body"),
                    },
                ));
            }
        }
    }

    let main_function = hir.assignments.iter().find(|(_, box n, _)| n == "main");
    if let Some((_, _, assignment)) = main_function {
        match assignment.clone() {
            Definition::Function {
                parameters,
                return_type,
                ..
            } => {
                if !parameters.is_empty() {
                    // TODO: report actual error location
                    context.add_error(
                        Offset(0)..Offset(0),
                        "Main function must not have parameters",
                    );
                }
                if !matches!(return_type, Type::Int | Type::Error) {
                    // TODO: report actual error location
                    context.add_error(Offset(0)..Offset(0), "Main function must return an int");
                }
            }
            Definition::Value { value, type_ } => {
                // TODO: report actual error location
                context.add_error(Offset(0)..Offset(0), "`main` function must be a function");
            }
        }
    } else {
        context.add_error(Offset(0)..Offset(0), "Program is missing a main function");
    }

    (hir, context.errors)
}

#[derive(Debug)]
struct Context<'c> {
    path: &'c Path,
    id_generator: IdGenerator<Id>,
    definitions: FxHashMap<Id, TempDefinition<'c>>,
    definitions_to_lower: BTreeSet<Id>,
    global_identifiers: FxHashMap<Box<str>, Named>,
    local_identifiers: Vec<(Box<str>, Id, Option<Definition>, Type)>,
    current_lowering_chain: Vec<Id>,
    errors: Vec<CompilerError>,
}
#[derive(Debug)]
enum Named {
    Value(Id),
    Functions(Vec<Id>),
}
#[derive(Debug)]
enum TempDefinition<'a> {
    Value(ValueDefinition<'a>),
    Function(FunctionDefinition<'a>),
}
#[derive(Debug)]
struct ValueDefinition<'a> {
    ast: Option<&'a AstAssignmentValue>,
    definition: Option<(Type, Option<Expression>)>,
}
#[derive(Debug)]
struct FunctionDefinition<'a> {
    ast: Option<&'a AstAssignmentFunction>,
    signature_and_body: Option<(Box<[Parameter]>, Type, Option<Body>)>,
}
impl<'c> Context<'c> {
    fn new(path: &'c Path) -> Self {
        Self {
            path,
            id_generator: IdGenerator::start_at(BuiltinFunction::VARIANTS.len()),
            definitions: FxHashMap::default(),
            definitions_to_lower: BTreeSet::default(),
            global_identifiers: FxHashMap::default(),
            local_identifiers: vec![],
            current_lowering_chain: vec![],
            errors: vec![],
        }
    }

    fn add_builtin_value(
        &mut self,
        name: impl Into<Box<str>>,
        expression: Expression,
        type_: Type,
    ) {
        let id = self.id_generator.generate();
        self.definitions.force_insert(
            id,
            TempDefinition::Value(ValueDefinition {
                ast: None,
                definition: Some((type_, Some(expression))),
            }),
        );
        self.global_identifiers
            .force_insert(name.into(), Named::Value(id));
    }
    fn add_builtin_functions(&mut self) {
        {
            let a_id = self.id_generator.generate();
            let b_id = self.id_generator.generate();
            self.add_builtin_function(
                BuiltinFunction::IntAdd,
                [
                    Parameter {
                        id: a_id,
                        name: "a".into(),
                        type_: Type::Int,
                    },
                    Parameter {
                        id: b_id,
                        name: "b".into(),
                        type_: Type::Int,
                    },
                ],
                Type::Int,
            );
        }
        {
            // TODO: Return `Nothing`
            let message_id = self.id_generator.generate();
            self.add_builtin_function(
                BuiltinFunction::Print,
                [Parameter {
                    id: message_id,
                    name: "message".into(),
                    type_: Type::Text,
                }],
                Type::Int,
            );
        }
        {
            let a_id = self.id_generator.generate();
            let b_id = self.id_generator.generate();
            self.add_builtin_function(
                BuiltinFunction::TextConcat,
                [
                    Parameter {
                        id: a_id,
                        name: "a".into(),
                        type_: Type::Text,
                    },
                    Parameter {
                        id: b_id,
                        name: "b".into(),
                        type_: Type::Text,
                    },
                ],
                Type::Text,
            );
        }
    }
    fn add_builtin_function(
        &mut self,
        builtin_function: BuiltinFunction,
        parameters: impl Into<Box<[Parameter]>>,
        return_type: Type,
    ) {
        let name = builtin_function.as_ref();
        let parameters = parameters.into();
        let id = builtin_function.id();
        self.definitions.force_insert(
            id,
            TempDefinition::Function(FunctionDefinition {
                ast: None,
                signature_and_body: Some((
                    parameters,
                    return_type,
                    Some(Body::Builtin(builtin_function)),
                )),
            }),
        );
        self.global_identifiers
            .force_insert(name.into(), Named::Functions(vec![id]));
    }

    fn catalog_assignments(&mut self, assignments: &'c [AstAssignment]) {
        for assignment in assignments {
            self.catalog_assignment(assignment);
        }
    }
    fn catalog_assignment(&mut self, assignment: &'c AstAssignment) {
        let Some(name) = assignment.name.value().and_then(|it| it.identifier.value()) else {
            return;
        };

        let id = self.id_generator.generate();
        let definition = match &assignment.kind {
            AstAssignmentKind::Value(ast) => {
                match self.global_identifiers.entry(name.string.clone()) {
                    Entry::Occupied(entry) => match entry.get() {
                        Named::Functions(_) => {
                            self.add_error(name.span.clone(), "A top-level value can't have the same name as a top-level function.");
                            return;
                        }
                        Named::Value { .. } => {
                            self.add_error(
                                name.span.clone(),
                                "Two top-level values can't have the same name.",
                            );
                            return;
                        }
                    },
                    Entry::Vacant(entry) => {
                        entry.insert(Named::Value(id));
                    }
                }
                TempDefinition::Value(ValueDefinition {
                    ast: Some(ast),
                    definition: None,
                })
            }
            AstAssignmentKind::Function(ast) => {
                let function = FunctionDefinition {
                    ast: Some(ast),
                    signature_and_body: None,
                };
                match self.global_identifiers.entry(name.string.clone()) {
                    Entry::Occupied(mut entry) => match entry.get_mut() {
                        Named::Functions(functions) => {
                            functions.push(id);
                        }
                        Named::Value { .. } => {
                            self.add_error(name.span.clone(), "A top-level function can't have the same name as a top-level value.");
                            return;
                        }
                    },
                    Entry::Vacant(entry) => {
                        entry.insert(Named::Functions(vec![id]));
                    }
                }
                TempDefinition::Function(function)
            }
        };
        self.definitions.force_insert(id, definition);
        self.definitions_to_lower.insert(id);
    }
    fn lower_assignments(&mut self) {
        while let Some(id) = self.definitions_to_lower.first() {
            self.current_lowering_chain.clear();

            self.lower_assignment(*id);

            assert!(self.current_lowering_chain.is_empty());
        }
    }
    fn lower_assignments_with_name(&mut self, name: &str) {
        let Some(named) = self.global_identifiers.get(name) else {
            return;
        };

        match named {
            Named::Value(id) => self.lower_assignment(*id),
            Named::Functions(ids) => {
                for id in ids.clone() {
                    self.lower_assignment(id);
                }
            }
        }
    }
    fn lower_assignment(&mut self, id: Id) {
        if self.current_lowering_chain.contains(&id) {
            // TODO: Is this even necessary with `definitions_to_lower`?
            return;
        }
        if !self.definitions_to_lower.remove(&id) {
            return;
        }

        let old_local_assignments = mem::take(&mut self.local_identifiers);
        self.current_lowering_chain.push(id);

        let definition = self.definitions.get(&id).unwrap();
        match definition {
            TempDefinition::Value(ValueDefinition { ast, .. }) => {
                let ast = ast.unwrap();

                let explicit_type =
                    ast.type_
                        .as_ref()
                        .and_then(|type_| type_.value())
                        .map(|type_| {
                            let type_value = self
                                .lower_expression(type_, Some(&Type::Type))
                                .map_or(Expression::Type(Type::Error), |(value, _)| value);
                            self.evaluate_expression_to_type(&type_value)
                        });
                if let Some(explicit_type) = explicit_type.as_ref() {
                    match self.definitions.get_mut(&id).unwrap() {
                        TempDefinition::Value(ValueDefinition { definition, .. }) => {
                            *definition = Some((explicit_type.clone(), None));
                        }
                        TempDefinition::Function(_) => unreachable!(),
                    }
                }

                let (value, value_type) = ast
                    .value
                    .value()
                    .and_then(|it| self.lower_expression(it, explicit_type.as_ref()))
                    .unwrap_or((Expression::Error, Type::Error));
                // TODO: check `value_type` is assignable to `explicit_type`
                match self.definitions.get_mut(&id).unwrap() {
                    TempDefinition::Value(ValueDefinition { definition, .. }) => {
                        *definition = Some((explicit_type.unwrap_or(value_type), Some(value)));
                    }
                    TempDefinition::Function(_) => unreachable!(),
                }
            }
            TempDefinition::Function(FunctionDefinition { ast, .. }) => {
                let ast = ast.unwrap();
                self.with_scope(|context| {
                    // TODO: lower parameter types
                    if !ast.parameters.is_empty() {
                        todo!("Function definition with parameters");
                    }
                    // let mut parameter_names = FxHashSet::default();
                    // let parameters = parameters
                    //     .iter()
                    //     .map(|parameter| try {
                    //         let name = parameter.name.identifier.value()?.clone();
                    //         if !parameter_names.insert(name.clone()) {
                    //             context.add_error(
                    //                 name.span.clone(),
                    //                 format!("Duplicate parameter name: {}", *name),
                    //             );
                    //             return None;
                    //         }

                    //         todo!();

                    //         // let type_ = context.lower_expression(parameter.type_.as_ref()?.value()?)?;
                    //         // let id = context.id_generator.generate();
                    //         // context.define_variable(name.string.clone(), id);
                    //         // Parameter {
                    //         //     name: name.string,
                    //         //     type_,
                    //         // }
                    //     })
                    //     .collect::<Option<Box<[_]>>>()?;

                    let return_type = ast
                        .return_type
                        .value()
                        .and_then(|it| {
                            context
                                .lower_expression(it, Some(&Type::Type))
                                .map(|(value, _)| context.evaluate_expression_to_type(&value))
                        })
                        .unwrap_or(Type::Error);
                    match context.definitions.get_mut(&id).unwrap() {
                        TempDefinition::Value(_) => unreachable!(),
                        TempDefinition::Function(FunctionDefinition {
                            signature_and_body, ..
                        }) => {
                            *signature_and_body = Some(([].into(), return_type.clone(), None));
                        }
                    }

                    let body = context.lower_body(&ast.body, &return_type);
                    // TODO: check body's return type is assignable to `return_type`
                    match context.definitions.get_mut(&id).unwrap() {
                        TempDefinition::Value(_) => unreachable!(),
                        TempDefinition::Function(FunctionDefinition {
                            signature_and_body, ..
                        }) => {
                            *signature_and_body = Some(([].into(), return_type, Some(body)));
                        }
                    };
                });
            }
        }

        assert_eq!(self.current_lowering_chain.pop().unwrap(), id);
        self.local_identifiers = old_local_assignments;
    }

    fn evaluate_expression_to_type(&mut self, expression: &Expression) -> Type {
        match expression {
            Expression::Int(_) => {
                // TODO: report actual error location
                self.add_error(Offset(0)..Offset(0), "Expected a type, not an int");
                Type::Error
            }
            Expression::Text(_) => {
                // TODO: report actual error location
                self.add_error(Offset(0)..Offset(0), "Expected a type, not a text");
                Type::Error
            }
            Expression::Tag { symbol, value } => Type::Tag(TagType {
                symbol: symbol.clone(),
                value_type: value
                    .as_ref()
                    .map(|it| Box::new(self.evaluate_expression_to_type(it))),
            }),
            Expression::Struct(_) => todo!(),
            Expression::StructAccess { struct_, field } => todo!(),
            Expression::ValueWithTypeAnnotation { value, type_ } => todo!(),
            Expression::Lambda { parameters, body } => todo!(),
            Expression::Reference(id) => {
                if let Some(assignment) = self
                    .local_identifiers
                    .iter()
                    .find(|(_, i, _, _)| i == id)
                    .map(|(_, _, assignment, _)| {
                        assignment.clone().expect("TODO: ID belongs to a parameter")
                    })
                {
                    match assignment {
                        Definition::Value { value, .. } => self.evaluate_expression_to_type(&value),
                        Definition::Function {
                            parameters,
                            return_type,
                            body,
                        } => todo!(),
                    }
                } else if let Some(definition) = self.definitions.get(id) {
                    match definition {
                        TempDefinition::Value(ValueDefinition { definition, .. }) => {
                            let Some(value) =
                                definition.as_ref().and_then(|(_, value)| value.as_ref())
                            else {
                                // TODO: report actual error location
                                self.add_error(
                                    Offset(0)..Offset(0),
                                    "Recursion while resolving type",
                                );
                                return Type::Error;
                            };
                            self.evaluate_expression_to_type(&value.clone())
                        }
                        TempDefinition::Function(_) => {
                            // TODO: report actual error location
                            self.add_error(
                                Offset(0)..Offset(0),
                                "Function reference is not a valid type",
                            );
                            return Type::Error;
                        }
                    }
                } else {
                    unreachable!("ID not found");
                }
            }
            Expression::Call {
                receiver,
                arguments,
            } => todo!(),
            Expression::Or { left, right } => {
                fn add_tag_type(
                    context: &mut Context,
                    tags: &mut Vec<TagType>,
                    has_error: &mut bool,
                    tag: TagType,
                ) {
                    if tags.iter().any(|it| it.symbol == tag.symbol) {
                        // TODO: report actual error location
                        context.add_error(
                            Offset(0)..Offset(0),
                            format!("Or type contains tag `{}` multiple times.", tag.symbol),
                        );
                        *has_error = true;
                    } else {
                        tags.push(tag);
                    }
                }
                fn add(
                    context: &mut Context,
                    tags: &mut Vec<TagType>,
                    has_error: &mut bool,
                    type_: Type,
                ) {
                    match type_ {
                        Type::Type => todo!(),
                        Type::Tag(tag) => add_tag_type(context, tags, has_error, tag),
                        Type::Or(OrType(or_tags)) => {
                            for tag in or_tags.iter() {
                                add_tag_type(context, tags, has_error, tag.clone());
                            }
                        }
                        Type::Int | Type::Text | Type::Struct(_) | Type::Function { .. } => {
                            // TODO: report actual error location
                            context.add_error(
                                Offset(0)..Offset(0),
                                format!("Or type can only contain tags types, found `{type_:?}`."),
                            );
                            *has_error = true;
                        }
                        Type::Error => *has_error = true,
                    };
                }

                let mut tags = vec![];
                let mut has_error = false;

                let left = self.evaluate_expression_to_type(left);
                let right = self.evaluate_expression_to_type(right);
                add(self, &mut tags, &mut has_error, left);
                add(self, &mut tags, &mut has_error, right);

                if has_error {
                    Type::Error
                } else {
                    Type::Or(OrType(tags.into()))
                }
            }
            Expression::CreateOrVariant { or_type, .. } => Type::Or(or_type.clone()),
            Expression::Type(type_) => type_.clone(),
            Expression::Error => Type::Error,
        }
    }

    fn lower_body(&mut self, body: &[AstStatement], context_type: &Type) -> Body {
        let mut expressions = vec![];
        for (position, statement) in body.iter().with_position() {
            let statement_context_type = if matches!(position, Position::Last | Position::Only) {
                Some(context_type)
            } else {
                None
            };

            let (name, expression, type_) = match statement {
                AstStatement::Assignment(assignment) => {
                    let Some(name) = assignment
                        .name
                        .value()
                        .and_then(|it| it.identifier.value())
                        .cloned()
                    else {
                        continue;
                    };

                    let (expression, type_) = match &assignment.kind {
                        AstAssignmentKind::Value(AstAssignmentValue { value, type_: _ }) => {
                            // TODO: lower written type
                            if let Some((value, type_)) =
                                value.value().and_then(|it| self.lower_expression(it, None))
                            {
                                (
                                    Expression::ValueWithTypeAnnotation {
                                        value: Box::new(value),
                                        type_: type_.clone(),
                                    },
                                    type_,
                                )
                            } else {
                                (Expression::Error, Type::Error)
                            }
                        }
                        AstAssignmentKind::Function { .. } => todo!(),
                    };
                    // TODO: return `Nothing` instead
                    (Some(name.string), expression, type_)
                }
                AstStatement::Expression(expression) => {
                    let Some((expression, type_)) =
                        self.lower_expression(expression, statement_context_type)
                    else {
                        continue;
                    };

                    (None, expression, type_)
                }
            };

            let id = self.id_generator.generate();
            if let Some(name) = &name {
                self.local_identifiers.push((
                    name.clone(),
                    id,
                    Some(Definition::Value {
                        value: expression.clone(),
                        type_: type_.clone(),
                    }),
                    type_.clone(),
                ));
            }
            expressions.push((id, name, expression, type_));
        }

        if expressions.is_empty() {
            // TODO: report actual error location
            self.add_error(Offset(0)..Offset(0), "Body must not be empty");
        }
        Body::Written { expressions }
    }

    fn lower_expression(
        &mut self,
        expression: &AstExpression,
        context_type: Option<&Type>,
    ) -> Option<(Expression, Type)> {
        let (expression, type_) = match expression {
            AstExpression::Identifier(identifier) => {
                let identifier = identifier.identifier.value()?;
                let name = &identifier.string;
                if let Some((id, type_)) = self.lookup_local_identifier(identifier) {
                    (Expression::Reference(id), type_.clone())
                } else if let Some(named) = self.global_identifiers.get(name) {
                    match named {
                        Named::Value(id) => {
                            let id = *id;
                            self.lower_assignments_with_name(name);
                            let definition = match self.definitions.get(&id).unwrap() {
                                TempDefinition::Value(ValueDefinition { definition, .. }) => {
                                    definition
                                }
                                TempDefinition::Function(_) => unreachable!(),
                            };

                            let type_ = if let Some((type_, _)) = definition {
                                type_.clone()
                            } else {
                                self.add_error(
                                    identifier.span.clone(),
                                    "Missing type in recursion",
                                );
                                Type::Error
                            };
                            (Expression::Reference(id), type_)
                        }
                        Named::Functions(functions) => {
                            assert!(!functions.is_empty());
                            if functions.len() > 1 {
                                self.add_error(
                                    identifier.span.clone(),
                                    "Function overloads are not yet supported",
                                );
                                (Expression::Error, Type::Error)
                            } else {
                                let id = functions[0];
                                self.lower_assignments_with_name(name);
                                let signature_and_body = match self.definitions.get(&id).unwrap() {
                                    TempDefinition::Value(_) => unreachable!(),
                                    TempDefinition::Function(FunctionDefinition {
                                        signature_and_body,
                                        ..
                                    }) => signature_and_body,
                                };

                                if let Some((parameter_types, return_type, _)) = signature_and_body
                                {
                                    (
                                        Expression::Reference(id),
                                        Type::Function {
                                            parameter_types: parameter_types
                                                .iter()
                                                .map(|it| it.type_.clone())
                                                .collect(),
                                            return_type: Box::new(return_type.clone()),
                                        },
                                    )
                                } else {
                                    self.add_error(
                                        identifier.span.clone(),
                                        "Missing function signature in recursion",
                                    );
                                    (Expression::Error, Type::Error)
                                }
                            }
                        }
                    }
                } else {
                    self.add_error(identifier.span.clone(), format!("Unknown variable: {name}"));
                    return None;
                }
            }
            AstExpression::Symbol(symbol) => {
                let symbol_string = &symbol.symbol.value()?.string;
                if context_type == Some(&Type::Type) {
                    (
                        Expression::Type(Type::Tag(TagType {
                            symbol: symbol_string.clone(),
                            value_type: None,
                        })),
                        Type::Type,
                    )
                } else {
                    (
                        Expression::Tag {
                            symbol: symbol_string.clone(),
                            value: None,
                        },
                        Type::Tag(TagType {
                            symbol: symbol_string.clone(),
                            value_type: None,
                        }),
                    )
                }
            }
            AstExpression::Int(int) => (Expression::Int(*int.value.value()?), Type::Int),
            AstExpression::Text(text) => {
                let text = text
                    .parts
                    .iter()
                    .map::<Option<Expression>, _>(|it| try {
                        match it {
                            AstTextPart::Text(text) => Expression::Text(text.clone()),
                            AstTextPart::Interpolation { expression, .. } => {
                                let (value, type_) =
                                    self.lower_expression(expression.value()?, Some(&Type::Text))?;
                                if type_ != Type::Text {
                                    // TODO: report actual error location
                                    self.add_error(
                                        Offset(0)..Offset(0),
                                        "Interpolated expression must be text",
                                    );
                                    return None;
                                }
                                value
                            }
                        }
                    })
                    .reduce(|lhs, rhs| match (lhs, rhs) {
                        (Some(lhs), Some(rhs)) => Some(Expression::Call {
                            receiver: Box::new(Expression::Reference(
                                BuiltinFunction::TextConcat.id(),
                            )),
                            arguments: [lhs, rhs].into(),
                        }),
                        _ => None,
                    })??;
                (text, Type::Text)
            }
            AstExpression::Parenthesized(parenthesized) => {
                return self.lower_expression(parenthesized.inner.value()?, context_type);
            }
            AstExpression::Call(call) => {
                let (receiver, receiver_type) = self.lower_expression(&call.receiver, None)?;
                let parameter_context_types: Option<Cow<Box<[Type]>>> = match &receiver_type {
                    Type::Tag(TagType {
                        value_type: Some(box type_),
                        ..
                    }) => Some(Cow::Owned([type_.clone()].into())),
                    Type::Function {
                        parameter_types, ..
                    } => Some(Cow::Borrowed(parameter_types)),
                    _ => None,
                };

                let arguments = call
                    .arguments
                    .iter()
                    .enumerate()
                    .map(|(index, argument)| {
                        self.lower_expression(
                            &argument.value,
                            parameter_context_types
                                .as_deref()
                                .and_then(|it| it.get(index)),
                        )
                    })
                    .collect::<Option<Box<[_]>>>()?;
                match receiver_type {
                    Type::Type | Type::Int | Type::Text | Type::Struct(_) | Type::Or { .. } => {
                        // TODO: report actual error location
                        self.add_error(Offset(0)..Offset(0), "Cannot call this type");
                        return None;
                    }
                    Type::Tag(TagType { symbol, value_type }) => {
                        if value_type.is_some() {
                            // TODO: report actual error location
                            self.add_error(
                                Offset(0)..Offset(0),
                                "You called a tag that already has a value.",
                            );
                            return None;
                        } else if arguments.len() > 1 {
                            // TODO: report actual error location
                            self.add_error(
                                Offset(0)..Offset(0),
                                "Tags can only be created with one value.",
                            );
                            return None;
                        }

                        assert!(!arguments.is_empty());
                        let (value, value_type) = arguments[0].clone();
                        (
                            Expression::Tag {
                                symbol: symbol.clone(),
                                value: Some(Box::new(value)),
                            },
                            Type::Tag(TagType {
                                symbol,
                                value_type: Some(Box::new(value_type)),
                            }),
                        )
                    }
                    Type::Function {
                        parameter_types,
                        box return_type,
                        ..
                    } => {
                        if parameter_types.len() == arguments.len() {
                            (
                                Expression::Call {
                                    receiver: Box::new(receiver),
                                    arguments: arguments
                                        .iter()
                                        .map(|(value, _)| value.clone())
                                        .collect(),
                                },
                                return_type,
                            )
                        } else {
                            // TODO: report actual error location
                            self.add_error(
                                Offset(0)..Offset(0),
                                format!(
                                    "Expected {} {}, got {}.",
                                    parameter_types.len(),
                                    if parameter_types.len() == 1 {
                                        "argument"
                                    } else {
                                        "arguments"
                                    },
                                    arguments.len(),
                                ),
                            );
                            (Expression::Error, Type::Error)
                        }
                    }
                    Type::Error => (Expression::Error, Type::Error),
                }
            }
            AstExpression::Struct(struct_) => {
                let struct_context_type =
                    context_type.and_then(|context_type| match context_type {
                        Type::Struct(context_type) => Some(context_type),
                        _ => None,
                    });

                let mut keys = FxHashSet::default();
                let fields = struct_
                    .fields
                    .iter()
                    .map(|field| try {
                        let name = field.key.identifier.value()?;
                        if !keys.insert(name.clone()) {
                            self.add_error(
                                name.span.clone(),
                                format!("Duplicate struct field: {}", **name),
                            );
                            return None;
                        }

                        let (value, type_) = self.lower_expression(
                            field.value.value()?.as_ref(),
                            struct_context_type.and_then(|it| {
                                it.iter()
                                    .find(|(type_name, _)| type_name == &**name)
                                    .map(|(_, type_)| type_)
                            }),
                        )?;
                        (name.string.clone(), value, type_)
                    })
                    .collect::<Option<Vec<(Box<str>, Expression, Type)>>>()?;
                let type_ = if fields.iter().any(|(_, _, type_)| type_ == &Type::Type) {
                    Type::Type
                } else {
                    Type::Struct(
                        fields
                            .iter()
                            .map(|(name, _, type_)| (name.clone(), type_.clone()))
                            .collect(),
                    )
                };
                let fields = fields
                    .into_iter()
                    .map(|(name, value, _)| (name, value))
                    .collect();
                (Expression::Struct(fields), type_)
            }
            AstExpression::StructAccess(struct_access) => {
                // TODO: If we support implicit struct type conversion, pass a context type here
                let (struct_, struct_type) =
                    self.lower_expression(struct_access.struct_.as_ref(), None)?;
                let field = struct_access
                    .key
                    .value()?
                    .identifier
                    .value()?
                    .string
                    .clone();
                let type_ = match struct_type {
                    Type::Type
                    | Type::Tag { .. }
                    | Type::Int
                    | Type::Text
                    | Type::Function { .. }
                    | Type::Or { .. } => {
                        // TODO: report actual error location
                        self.add_error(Offset(0)..Offset(0), "Receiver is not a struct");
                        Type::Error
                    }
                    Type::Struct(struct_type) => {
                        struct_type
                            .iter()
                            .find_map(|(name, type_)| {
                                if name == &field {
                                    Some(type_.clone())
                                } else {
                                    None
                                }
                            })
                            .unwrap_or_else(|| {
                                // TODO: report actual error location
                                self.add_error(
                                    Offset(0)..Offset(0),
                                    format!("Struct does not have field `{field}`"),
                                );
                                Type::Error
                            })
                    }
                    Type::Error => todo!(),
                };
                (
                    Expression::StructAccess {
                        struct_: Box::new(struct_),
                        field,
                    },
                    type_,
                )
            }
            AstExpression::Lambda(_) => todo!(),
            AstExpression::Or(or) => {
                let (left, _) = self.lower_expression(&or.left, Some(&Type::Type))?;
                let right = or
                    .right
                    .value()
                    .and_then(|it| {
                        self.lower_expression(it, Some(&Type::Type))
                            .map(|(value, _)| value)
                    })
                    .unwrap_or(Expression::Error);
                (
                    Expression::Or {
                        left: Box::new(left),
                        right: Box::new(right),
                    },
                    Type::Type,
                )
            }
        };

        if let Some(context_type) = context_type {
            if let (Type::Tag(from), Type::Or(OrType(to))) = (&type_, context_type)
                && to
                    .iter()
                    .any(|to| Self::tag_type_is_assignable_to(from, to))
            {
                Some((
                    Expression::CreateOrVariant {
                        or_type: OrType(to.clone()),
                        symbol: from.symbol.clone(),
                        value: Box::new(expression),
                    },
                    context_type.clone(),
                ))
            } else if Self::is_assignable_to(&type_, context_type) {
                Some((expression, context_type.clone()))
            } else {
                // TODO: report actual error location
                self.add_error(
                    Offset(0)..Offset(0),
                    format!("Expected type `{context_type:?}`, got `{type_:?}`."),
                );
                Some((Expression::Error, Type::Error))
            }
        } else {
            Some((expression, type_))
        }
    }

    fn is_assignable_to(from: &Type, to: &Type) -> bool {
        match (from, to) {
            (Type::Error, _)
            | (_, Type::Error)
            | (Type::Type, Type::Type)
            | (Type::Int, Type::Int)
            | (Type::Text, Type::Text) => true,
            (Type::Tag(from), Type::Tag(to)) => Self::tag_type_is_assignable_to(from, to),
            (Type::Or(OrType(from)), Type::Or(OrType(to))) => {
                // TODO: support subsets
                if from.len() == to.len() {
                    return false;
                }
                from.iter().all(|from| {
                    to.iter()
                        .any(|to| Self::tag_type_is_assignable_to(from, to))
                })
            }
            (Type::Struct(from), Type::Struct(to)) => {
                if from.len() != to.len() {
                    return false;
                }
                from.iter().all(|(name, field_from)| {
                    let Some((_, field_to)) = to.iter().find(|(to_name, _)| name == to_name) else {
                        return false;
                    };
                    Self::is_assignable_to(field_from, field_to)
                })
            }
            (
                Type::Function {
                    parameter_types,
                    return_type,
                },
                Type::Function {
                    parameter_types: to_parameter_types,
                    return_type: to_return_type,
                },
            ) => {
                parameter_types.len() == to_parameter_types.len()
                    && parameter_types
                        .iter()
                        .zip(to_parameter_types.iter())
                        .all(|(from, to)| Self::is_assignable_to(from, to))
                    && Self::is_assignable_to(return_type, to_return_type)
            }
            _ => false,
        }
    }

    fn tag_type_is_assignable_to(from: &TagType, to: &TagType) -> bool {
        from.symbol == to.symbol
            && match (from.value_type.as_ref(), to.value_type.as_ref()) {
                (Some(from), Some(to)) => Self::is_assignable_to(from, to),
                (None, None) => true,
                _ => false,
            }
    }

    // Utils
    fn with_scope<T>(&mut self, fun: impl FnOnce(&mut Self) -> T) -> T {
        let scope = self.local_identifiers.len();
        let result = fun(self);
        assert!(self.local_identifiers.len() >= scope);
        self.local_identifiers.truncate(scope);
        result
    }
    #[must_use]
    fn lookup_local_identifier(&self, name: &str) -> Option<(Id, &Type)> {
        self.local_identifiers
            .iter()
            .rev()
            .find(|(box variable_name, _, _, _)| variable_name == name)
            .map(|(_, id, _, type_)| (*id, type_))
    }

    fn add_error(&mut self, span: Range<Offset>, message: impl Into<String>) {
        self.errors.push(CompilerError {
            path: self.path.to_path_buf(),
            span,
            message: message.into(),
        });
    }
}
