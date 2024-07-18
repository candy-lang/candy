use crate::{
    hir::{Body, BuiltinFunction, Definition, Expression, Hir, OrType, Parameter, TagType, Type},
    utils::HashSetExtension,
};
use core::panic;
use itertools::Itertools;
use rustc_hash::FxHashSet;
use std::borrow::Cow;

pub fn hir_to_c(hir: &Hir) -> String {
    let mut context = Context::new(hir);
    context.lower_hir();
    context.c
}

#[derive(Debug)]
struct Context<'h> {
    hir: &'h Hir,
    c: String,
}
impl<'h> Context<'h> {
    #[must_use]
    const fn new(hir: &'h Hir) -> Self {
        Self {
            hir,
            c: String::new(),
        }
    }

    fn lower_hir(&mut self) {
        self.push("#include <stdint.h>\n");
        self.push("#include <stdio.h>\n");
        self.push("#include <stdlib.h>\n");
        self.push("#include <string.h>\n\n");

        self.push("/// Types Definitions\n\n");
        self.lower_type_definitions();
        self.push("\n");

        self.push("/// Declarations\n\n");
        for (id, name, assignment) in &self.hir.assignments {
            self.push(format!("/* {name} */ "));
            match assignment {
                Definition::Value { type_, .. } => 'case: {
                    if type_ == &Type::Type {
                        self.push("// Is a type.");
                        break 'case;
                    }

                    self.push("const ");
                    self.lower_type(type_);
                    self.push(format!(" {id};"));
                }
                Definition::Function {
                    box parameters,
                    return_type,
                    ..
                } => {
                    self.lower_type(return_type);
                    self.push(format!(" {id}("));
                    for (i, parameter) in parameters.iter().enumerate() {
                        if i != 0 {
                            self.push(", ");
                        }
                        self.lower_type(&parameter.type_);
                        self.push(format!(" {}", parameter.id));
                    }
                    self.push(");");
                }
            }
            self.push("\n");
        }
        self.push("\n");

        self.push("/// Definitions\n\n");
        for (id, name, assignment) in &self.hir.assignments {
            self.push(format!("// {name}\n"));
            match assignment {
                Definition::Value { type_, value } => 'case: {
                    if type_ == &Type::Type {
                        self.push("// Is a type.");
                        break 'case;
                    }

                    self.push("const ");
                    self.lower_type(type_);
                    self.push(format!(" {id} = "));
                    self.lower_expression(value);
                    self.push(";");
                }
                Definition::Function {
                    box parameters,
                    return_type,
                    body,
                } => {
                    self.lower_type(return_type);
                    self.push(format!(" {id}("));
                    for (i, parameter) in parameters.iter().enumerate() {
                        if i != 0 {
                            self.push(", ");
                        }
                        self.lower_type(&parameter.type_);
                        self.push(format!(" {}", parameter.id));
                    }
                    self.push(") {\n");
                    self.lower_body(parameters, body);
                    self.push("}");
                }
            }
            self.push("\n\n");
        }

        let (main_function_id, _, _) = self
            .hir
            .assignments
            .iter()
            .find(|(_, box name, _)| name == "main")
            .unwrap();
        self.push(format!("int main() {{ return {main_function_id}(); }}\n"));
    }
    fn lower_body(&mut self, parameters: &[Parameter], body: &Body) {
        match body {
            Body::Builtin(builtin_function) => {
                self.push("// builtin function\n");
                self.push(match builtin_function {
                    BuiltinFunction::IntAdd => format!(
                        "return {a} + {b};",
                        a = parameters[0].id,
                        b = parameters[1].id,
                    ),
                    BuiltinFunction::Print => format!("puts({}); return 0;", parameters[0].id),
                    BuiltinFunction::TextConcat => format!(
                        "\
                        const size_t lengthA = strlen({a});\n\
                        const size_t lengthB = strlen({b});\n\
                        char *result = malloc(lengthA + lengthB + 1);\n\
                        memcpy(result, {a}, lengthA);\n\
                        memcpy(result + lengthA, {b}, lengthB + 1);\n\
                        return result;",
                        a = parameters[0].id,
                        b = parameters[1].id,
                    ),
                });
            }
            Body::Written { expressions } => {
                for (id, name, expression, type_) in expressions {
                    if let Some(name) = name {
                        self.push(format!("// {name}\n"));
                    }

                    self.lower_type(type_);
                    self.push(format!(" {id} = "));
                    self.lower_expression(expression);
                    self.push(";\n");
                }
                self.push(format!("return {};", expressions.last().unwrap().0));
            }
        }
    }
    fn lower_expression(&mut self, expression: &Expression) {
        match expression {
            Expression::Int(int) => self.push(format!("{int}")),
            // TODO: escape text
            Expression::Text(text) => self.push(format!("\"{text}\"")),
            Expression::Tag { symbol: _, value } => {
                self.push("{ ");
                if let Some(value) = value {
                    self.push(".value = ");
                    self.lower_expression(value);
                }
                self.push("}");
            }
            Expression::Struct(fields) => {
                self.push("{ ");
                for (name, value) in fields.iter() {
                    self.push(format!(".{name} = "));
                    self.lower_expression(value);
                    self.push(", ");
                }
                self.push("}");
            }
            Expression::StructAccess { struct_, field } => {
                self.lower_expression(struct_);
                self.push(format!(".{field}"));
            }
            Expression::ValueWithTypeAnnotation { value, type_ } => {
                self.lower_expression(value);
            }
            Expression::Lambda { .. } => todo!(),
            Expression::Reference(id) => self.push(id.to_string()),
            Expression::Call {
                receiver,
                arguments,
            } => {
                self.lower_expression(receiver);
                self.push("(");
                for (i, argument) in arguments.iter().enumerate() {
                    if i != 0 {
                        self.push(", ");
                    }
                    self.lower_expression(argument);
                }
                self.push(")");
            }
            Expression::Or { .. } => panic!("Or expression found."),
            Expression::CreateOrVariant {
                or_type,
                symbol,
                value,
            } => {
                self.push("{ ");
                self.push(".symbol = ");
                self.push(symbol);
                self.push(", .tag = ");
                self.lower_expression(value);
                self.push("}");
            }
            Expression::Type(_) => panic!("Should have been resolved to a value."),
            Expression::Error => panic!("Error expression found."),
        }
    }

    fn lower_type_definitions(&mut self) {
        let mut definitions = FxHashSet::default();
        for (_, _, assignment) in &self.hir.assignments {
            match assignment {
                Definition::Value { type_, value } => {
                    self.lower_type_definition(&mut definitions, type_);
                    self.lower_type_definitions_in_expression(&mut definitions, value);
                }
                Definition::Function {
                    box parameters,
                    return_type,
                    body,
                } => {
                    for parameter in parameters {
                        self.lower_type_definition(&mut definitions, &parameter.type_);
                    }
                    self.lower_type_definition(&mut definitions, return_type);
                    self.lower_type_definitions_in_body(&mut definitions, body);
                }
            }
        }
    }
    fn lower_type_definitions_in_body(
        &mut self,
        definitions: &mut FxHashSet<Type>,
        body: &'h Body,
    ) {
        match body {
            Body::Builtin(_) => {}
            Body::Written { expressions } => {
                for (_, _, expression, type_) in expressions {
                    self.lower_type_definition(definitions, type_);
                    self.lower_type_definitions_in_expression(definitions, expression);
                }
            }
        }
    }
    fn lower_type_definitions_in_expression(
        &mut self,
        definitions: &mut FxHashSet<Type>,
        expression: &'h Expression,
    ) {
        match expression {
            Expression::Int(_)
            | Expression::Text(_)
            | Expression::Tag { .. }
            | Expression::Struct(_)
            | Expression::StructAccess { .. }
            | Expression::ValueWithTypeAnnotation { .. } => {}
            Expression::Lambda { parameters, body } => {
                for parameter in parameters.iter() {
                    self.lower_type_definition(definitions, &parameter.type_);
                }
                self.lower_type_definitions_in_body(definitions, body);
            }
            Expression::Reference(_)
            | Expression::Call { .. }
            | Expression::Or { .. }
            | Expression::CreateOrVariant { .. } => {}
            Expression::Type(type_) => self.lower_type_definition(definitions, type_),
            Expression::Error => {}
        }
    }
    fn lower_type_definition(&mut self, definitions: &mut FxHashSet<Type>, type_: &Type) {
        if matches!(type_, &Type::Type | &Type::Error) || definitions.contains(type_) {
            return;
        }
        definitions.force_insert(type_.clone());

        match type_ {
            Type::Type => panic!("Type type found."),
            Type::Tag(TagType {
                symbol: _,
                value_type,
            }) => {
                if let Some(box value_type) = value_type {
                    self.lower_type_definition(definitions, value_type);
                }

                self.push("struct ");
                self.lower_type(type_);
                self.push("{");
                if let Some(value_type) = value_type {
                    self.lower_type(value_type);
                    self.push(" value; ");
                }
                self.push("};\n");

                self.push("typedef struct ");
                self.lower_type(type_);
                self.push(" ");
                self.lower_type(type_);
                self.push(";\n");
            }
            Type::Or(OrType(tags)) => {
                for tag in tags.iter() {
                    self.lower_type_definition(definitions, &Type::Tag(tag.clone()));
                }

                let tags = tags
                    .iter()
                    .sorted_by_key(|tag| &tag.symbol)
                    .collect::<Vec<_>>();

                self.push("struct ");
                self.lower_type(type_);
                self.push("{\nenum {");
                for tag in &tags {
                    self.push(&tag.symbol);
                    self.push(", ");
                }
                self.push("} symbol;\nunion {");
                for tag in tags {
                    self.lower_type(&Type::Tag(tag.clone()));
                    self.push(" ");
                    self.push(&tag.symbol);
                    self.push(";");
                }
                self.push("} tag;\n};\n");

                self.push("typedef struct ");
                self.lower_type(type_);
                self.push(" ");
                self.lower_type(type_);
                self.push(";\n");
            }
            Type::Int => {
                self.push("typedef int64_t ");
                self.lower_type(type_);
                self.push(";\n");
            }
            Type::Text => {
                self.push("typedef char* ");
                self.lower_type(type_);
                self.push(";\n");
            }
            Type::Struct(struct_) => {
                for (_, type_) in struct_.iter() {
                    self.lower_type_definition(definitions, type_);
                }

                self.push("struct ");
                self.lower_type(type_);
                self.push("{");
                for (name, type_) in struct_.iter() {
                    self.lower_type(type_);
                    self.push(format!(" {name}; "));
                }
                self.push("};\n");

                self.push("typedef struct ");
                self.lower_type(type_);
                self.push(" ");
                self.lower_type(type_);
                self.push(";\n");
            }
            Type::Function {
                parameter_types,
                box return_type,
            } => {
                for type_ in parameter_types.iter() {
                    self.lower_type_definition(definitions, type_);
                }
                self.lower_type_definition(definitions, return_type);

                self.push("typedef ");
                self.lower_type(return_type);
                self.push(" (*)(");
                for (i, parameter_type) in parameter_types.iter().enumerate() {
                    if i != 0 {
                        self.push(", ");
                    }
                    self.lower_type(parameter_type);
                }
                self.push(") ");
                self.lower_type(type_);
                self.push(";\n");
            }
            Type::Error => panic!("Error type found."),
        }
    }

    fn lower_type(&mut self, type_: &Type) {
        match type_ {
            Type::Type => panic!("Type type found."),
            Type::Tag(tag_type) => self.lower_tag_type(tag_type),
            Type::Or(OrType(tags)) => {
                for (index, tag) in tags.iter().enumerate() {
                    if index != 0 {
                        self.push("_or_");
                    }
                    self.lower_tag_type(tag);
                }
            }
            Type::Int => self.push("candyInt"),
            Type::Text => self.push("candyText"),
            Type::Struct(struct_) => {
                self.push("structOf_");
                for (index, (name, type_)) in
                    struct_.iter().sorted_by_key(|(name, _)| name).enumerate()
                {
                    if index != 0 {
                        self.push("_and_");
                    }
                    self.push(format!("{name}_"));
                    self.lower_type(type_);
                }
                self.push("_end");
            }
            Type::Function {
                parameter_types,
                return_type,
            } => {
                self.push("function_");
                if !parameter_types.is_empty() {
                    self.push("taking");
                    for (index, parameter_type) in parameter_types.iter().enumerate() {
                        if index != 0 {
                            self.push("_and_");
                        }
                        self.lower_type(parameter_type);
                    }
                }
                self.push("_returning_");
                self.lower_type(return_type);
                self.push("_end");
            }
            Type::Error => panic!("Error type found."),
        }
    }
    fn lower_tag_type(&mut self, tag_type: &TagType) {
        self.push("tag_");
        self.push(&tag_type.symbol);
        if let Some(value_type) = tag_type.value_type.as_ref() {
            self.push("_of_");
            self.lower_type(value_type);
        }
    }

    fn push(&mut self, s: impl AsRef<str>) {
        self.c.push_str(s.as_ref());
    }
}
