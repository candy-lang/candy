use crate::{
    hir::{
        Body, BodyOrBuiltin, BuiltinFunction, Expression, ExpressionKind, Hir, Id, Parameter, Type,
        TypeDeclaration,
    },
    id::CountableId,
};
use core::panic;
use itertools::Itertools;

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

        self.push("/// Type Declarations\n\n");
        self.lower_type_declarations();
        self.push("\n");

        self.push("/// Assignment Declarations\n\n");
        self.lower_assignment_declarations();
        self.push("\n");

        self.push("/// Function Declarations\n\n");
        self.lower_function_declarations();
        self.push("\n");

        self.push("/// Assignment Definitions\n\n");
        self.lower_assignment_definitions();
        self.push("\n");

        self.push("/// Function Definitions\n\n");
        self.lower_function_definitions();
        // TODO: init assignments in main in correct order

        self.push("int main() {\n");
        for id in &self.hir.assignment_initialization_order {
            self.push(format!("init{id}();\n"));
        }
        self.push(format!(
            "return {}()->value;\n}}\n",
            self.hir.main_function_id,
        ));
    }

    fn lower_type_declarations(&mut self) {
        self.push(
            "\
            struct Int {
                uint64_t value;
            };
            typedef struct Int Int;",
        );
        self.push(
            "\
            struct Text {
                char* value;
            };
            typedef struct Text Text;",
        );

        for (name, declaration) in &self.hir.type_declarations {
            match declaration {
                TypeDeclaration::Struct { fields } => {
                    self.push("struct ");
                    self.push(name);
                    self.push("{");
                    for (name, type_) in fields.iter() {
                        self.lower_type(type_);
                        self.push(format!(" {name}; "));
                    }
                    self.push("};\n");

                    self.push("typedef struct ");
                    self.push(name);
                    self.push(" ");
                    self.push(name);
                    self.push(";\n");
                }
                TypeDeclaration::Enum { variants } => {
                    self.push(format!("struct {name} {{\n"));

                    if !variants.is_empty() {
                        self.push("enum {");
                        for (variant, _) in variants.iter() {
                            self.push(format!("{name}_{variant},"));
                        }
                        self.push("} variant;\n");
                    }

                    self.push("union {");
                    for (variant, value_type) in variants.iter() {
                        if let Some(value_type) = value_type {
                            self.lower_type(value_type);
                            self.push(" ");
                            self.push(variant);
                            self.push(";");
                        }
                    }
                    self.push("} value;\n};\n");

                    self.push("typedef struct ");
                    self.push(name);
                    self.push(" ");
                    self.push(name);
                    self.push(";\n");
                }
            }
        }
    }

    fn lower_assignment_declarations(&mut self) {
        for (id, name, assignment) in self.hir.assignments.iter() {
            self.push(format!("/* {name} */ "));
            self.lower_type(&assignment.type_);
            self.push(format!(" {id};\n"));
        }
    }
    fn lower_assignment_definitions(&mut self) {
        for (id, name, assignment) in self.hir.assignments.iter() {
            self.push(format!("// {name}\n"));

            self.push(format!("void init{id}() {{\n"));
            self.lower_body_expressions(&assignment.body);
            self.push(format!(
                "{id} = {};\n}}\n\n",
                assignment.body.return_value_id(),
            ));
        }
    }

    fn lower_function_declarations(&mut self) {
        for (id, name, function) in self.hir.functions.iter() {
            self.push(format!("/* {name} */ "));
            self.lower_type(&function.return_type);
            self.push(format!(" {id}("));
            for (index, parameter) in function.parameters.iter().enumerate() {
                if index > 0 {
                    self.push(", ");
                }
                self.lower_type(&parameter.type_);
                self.push(format!(" {}", parameter.id));
            }
            self.push(");\n");

            // self.lower_type(&Type::Function {
            //     parameter_types: function
            //         .parameters
            //         .iter()
            //         .map(|it| it.type_.clone())
            //         .collect(),
            //     return_type: Box::new(function.return_type.clone()),
            // });
            // self.push(format!(
            //     " {id} = {{ .closure = NULL, .function = {id}_function }};",
            // ));
            // self.push("\n");
        }
    }
    fn lower_function_definitions(&mut self) {
        for (id, name, function) in self.hir.functions.iter() {
            self.push(format!("// {name}\n"));

            self.lower_type(&function.return_type);
            self.push(format!(" {id}("));
            for (index, parameter) in function.parameters.iter().enumerate() {
                if index > 0 {
                    self.push(", ");
                }
                self.lower_type(&parameter.type_);
                self.push(format!(" {}", parameter.id));
            }
            self.push(") {\n");
            self.lower_body_or_builtin(&function.parameters, &function.body);
            self.push("}\n\n");
        }
    }
    fn lower_body_or_builtin(&mut self, parameters: &[Parameter], body: &BodyOrBuiltin) {
        match body {
            BodyOrBuiltin::Builtin(builtin_function) => {
                self.push("// builtin function\n");
                match builtin_function {
                    BuiltinFunction::IntAdd => self.push(format!(
                        "\
                        Int* result_pointer = malloc(sizeof(Int));
                        result_pointer->value = {a}->value + {b}->value;
                        return result_pointer;",
                        a = parameters[0].id,
                        b = parameters[1].id,
                    )),
                    BuiltinFunction::IntCompareTo => self.push(format!(
                        "\
                        Ordering* result_pointer = malloc(sizeof(Ordering));
                        result_pointer->variant = {a}->value < {b}->value    ? Ordering_less
                                                  : {a}->value == {b}->value ? Ordering_equal
                                                                             : Ordering_greater;
                        return result_pointer;",
                        a = parameters[0].id,
                        b = parameters[1].id,
                    )),
                    BuiltinFunction::IntSubtract => self.push(format!(
                        "\
                        Int* result_pointer = malloc(sizeof(Int));
                        result_pointer->value = {a}->value - {b}->value;
                        return result_pointer;",
                        a = parameters[0].id,
                        b = parameters[1].id,
                    )),
                    BuiltinFunction::IntToText => self.push(format!(
                        "\
                        int length = snprintf(NULL, 0, \"%ld\", {int}->value);
                        char* result = malloc(length + 1);
                        snprintf(result, length + 1, \"%ld\", {int}->value);
                        
                        Text* result_pointer = malloc(sizeof(Text));
                        result_pointer->value = result;
                        return result_pointer;",
                        int = parameters[0].id,
                    )),
                    BuiltinFunction::Panic => {
                        self.push(format!(
                            "\
                            fputs({}->value);
                            exit(1);",
                            parameters[0].id,
                        ));
                    }
                    BuiltinFunction::Print => {
                        self.push(format!("puts({}->value);\n", parameters[0].id));
                        let nothing_id = Id::from_usize(1);
                        self.lower_expression(nothing_id, &Expression::nothing());
                        self.push(format!("return {nothing_id};"));
                    }
                    BuiltinFunction::TextConcat => self.push(format!(
                        "\
                        size_t lengthA = strlen({a}->value);\n\
                        size_t lengthB = strlen({b}->value);\n\
                        char* result = malloc(lengthA + lengthB + 1);\n\
                        memcpy(result, {a}->value, lengthA);\n\
                        memcpy(result + lengthA, {b}->value, lengthB + 1);\n\
                        Text* result_pointer = malloc(sizeof(Text));
                        result_pointer->value = result;
                        return result_pointer;",
                        a = parameters[0].id,
                        b = parameters[1].id,
                    )),
                }
            }
            BodyOrBuiltin::Body(body) => self.lower_body(body),
        }
    }
    fn lower_body(&mut self, body: &Body) {
        self.lower_body_expressions(body);
        self.push(format!("return {};", body.return_value_id()));
    }
    fn lower_body_expressions(&mut self, body: &Body) {
        for (id, name, expression) in &body.expressions {
            if let Some(name) = name {
                self.push(format!("// {name}\n"));
            }

            self.lower_expression(*id, expression);
            self.push("\n");
        }
    }
    fn lower_expression(&mut self, id: Id, expression: &Expression) {
        match &expression.kind {
            ExpressionKind::Int(int) => {
                self.lower_type(&expression.type_);
                self.push(format!(" {id} = malloc(sizeof("));
                self.lower_type_without_pointer(&expression.type_);
                self.push("));");

                self.push(format!("{id}->value = {int};"));
            }
            ExpressionKind::Text(text) => {
                self.lower_type(&expression.type_);
                self.push(format!(" {id} = malloc(sizeof("));
                self.lower_type_without_pointer(&expression.type_);
                self.push("));");

                // TODO: escape text
                self.push(format!("{id}->value = \"{text}\";"));
            }
            ExpressionKind::CreateStruct { struct_, fields } => {
                let Type::Named(name) = struct_ else {
                    unreachable!();
                };
                let TypeDeclaration::Struct {
                    fields: type_fields,
                } = &self.hir.type_declarations[name]
                else {
                    unreachable!();
                };

                self.lower_type(&expression.type_);
                self.push(format!(" {id} = malloc(sizeof("));
                self.lower_type_without_pointer(&expression.type_);
                self.push("));");

                for ((name, _), value) in type_fields.iter().zip_eq(fields.iter()) {
                    self.push(format!("\n{id}->{name} = {value};"));
                }
            }
            ExpressionKind::StructAccess { struct_, field } => {
                self.lower_type(&expression.type_);
                self.push(format!(" {id} = {struct_}->{field};"));
            }
            ExpressionKind::CreateEnum {
                enum_,
                variant,
                value,
            } => {
                let Type::Named(name) = enum_ else {
                    unreachable!();
                };

                self.lower_type(&expression.type_);
                self.push(format!(" {id} = malloc(sizeof("));
                self.lower_type_without_pointer(&expression.type_);
                self.push("));\n");

                self.push(format!("{id}->variant = {name}_{variant};"));
                if let Some(value) = value {
                    self.push(format!("\n{id}->value = {value}"));
                }
            }
            // ExpressionKind::Lambda(lambda) => {
            //     self.push("{ .closure = {");
            //     for id in lambda.closure().iter().sorted() {
            //         self.push(format!(".{id} = {id}; "));
            //     }
            //     self.push(format!("}}, .function = {id}_function }};"));
            // }
            ExpressionKind::Reference(referenced_id) => {
                self.lower_type(&expression.type_);
                self.push(format!(" {id} = {referenced_id};"));
            }
            ExpressionKind::Call {
                function,
                arguments,
            } => {
                self.lower_type(&expression.type_);
                self.push(format!(" {id} = {function}("));
                for (index, argument) in arguments.iter().enumerate() {
                    if index > 0 {
                        self.push(", ");
                    }
                    self.push(format!("{argument}"));
                }
                self.push(");");
            }
            ExpressionKind::Switch {
                value,
                enum_,
                cases,
            } => {
                let Type::Named(name) = enum_ else {
                    unreachable!();
                };
                let TypeDeclaration::Enum { variants } = &self.hir.type_declarations[name] else {
                    unreachable!();
                };

                self.lower_type(&expression.type_);
                self.push(format!(" {id};\n"));

                self.push(format!("switch ({value}->variant) {{"));
                for case in cases.iter() {
                    self.push(format!("case {name}_{}:\n", case.variant));
                    if let Some(value_id) = case.value_id {
                        let variant_type = variants
                            .iter()
                            .find(|(var, _)| var == &case.variant)
                            .unwrap()
                            .1
                            .as_ref()
                            .unwrap();
                        self.lower_type(variant_type);
                        self.push(format!(" {value_id} = {value}->value;\n"));
                    }

                    self.lower_body_expressions(&case.body);

                    self.push(format!("{id} = {};\n", case.body.return_value_id()));

                    self.push("break;");
                }
                self.push("}");
            }
            ExpressionKind::Error => panic!("Error expression found."),
        }
    }

    fn lower_type(&mut self, type_: &Type) {
        self.lower_type_without_pointer(type_);
        self.push("*");
    }
    fn lower_type_without_pointer(&mut self, type_: &Type) {
        match type_ {
            Type::Named(name) => self.push(name),
            Type::Error => panic!("Error type found."),
        }
    }

    fn push(&mut self, s: impl AsRef<str>) {
        self.c.push_str(s.as_ref());
    }
}
