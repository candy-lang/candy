use crate::hir::{
    Body, BodyOrBuiltin, BuiltinFunction, Expression, ExpressionKind, Hir, Parameter, Type,
    TypeDeclaration,
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
        self.push("typedef int64_t Int;\n\n");
        self.push("typedef char* Text;\n\n");
        self.lower_type_declarations();
        self.push("\n");

        self.push("/// Declarations\n\n");
        self.lower_assignment_declarations();
        self.lower_function_declarations();
        self.push("\n");

        self.push("/// Definitions\n\n");
        self.lower_function_definitions();
        // TODO: init assignments in main in correct order

        let (main_function_id, _, _) = self
            .hir
            .functions
            .iter()
            .find(|(_, box name, _)| name == "main")
            .unwrap();
        self.push(format!("int main() {{ return {main_function_id}(); }}\n",));
    }

    fn lower_type_declarations(&mut self) {
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
                    self.push("} tag;\n};\n");

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
            self.push(format!("/* {name} */ const "));
            self.lower_type(&assignment.type_);
            self.push(format!(" {id};\n"));
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

            // self.push("const ");
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
                        "return {a} + {b};",
                        a = parameters[0].id,
                        b = parameters[1].id,
                    )),
                    BuiltinFunction::Print => {
                        self.push(format!("puts({}); const ", parameters[0].id));
                        self.lower_type(&Type::nothing());
                        self.push(" _1 = ");
                        self.lower_expression(&Expression::nothing());
                        self.push("; return _1;");
                    }
                    BuiltinFunction::TextConcat => self.push(format!(
                        "\
                        const size_t lengthA = strlen({a});\n\
                        const size_t lengthB = strlen({b});\n\
                        char *result = malloc(lengthA + lengthB + 1);\n\
                        memcpy(result, {a}, lengthA);\n\
                        memcpy(result + lengthA, {b}, lengthB + 1);\n\
                        return result;",
                        a = parameters[0].id,
                        b = parameters[1].id,
                    )),
                }
            }
            BodyOrBuiltin::Body(body) => self.lower_body(body),
        }
    }
    fn lower_body(&mut self, body: &Body) {
        for (id, name, expression) in &body.expressions {
            if let Some(name) = name {
                self.push(format!("// {name}\n"));
            }

            self.push("const ");
            self.lower_type(&expression.type_);
            self.push(format!(" {id} = "));
            self.lower_expression(expression);
            self.push(";\n");
        }
        self.push(format!("return {};", body.expressions.last().unwrap().0));
    }
    fn lower_expression(&mut self, expression: &Expression) {
        match &expression.kind {
            ExpressionKind::Int(int) => self.push(format!("{int}")),
            // TODO: escape text
            ExpressionKind::Text(text) => self.push(format!("\"{text}\"")),
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

                self.push("{ ");
                for ((name, _), value) in type_fields.iter().zip_eq(fields.iter()) {
                    self.push(format!(".{name} = {value},"));
                }
                self.push("}");
            }
            ExpressionKind::StructAccess { struct_, field } => {
                self.push(format!("{struct_}.{field}"));
            }
            ExpressionKind::CreateEnum {
                enum_,
                variant,
                value,
            } => {
                let Type::Named(name) = enum_ else {
                    unreachable!();
                };

                self.push("{ ");
                self.push(format!(".symbol = {name}_{variant},"));
                if let Some(value) = value {
                    self.push(format!(".value = {value}"));
                }
                self.push("}");
            }
            // ExpressionKind::Lambda(lambda) => {
            //     self.push("{ .closure = {");
            //     for id in lambda.closure().iter().sorted() {
            //         self.push(format!(".{id} = {id}; "));
            //     }
            //     self.push(format!("}}, .function = {id}_function }};"));
            // }
            ExpressionKind::Reference(id) => {
                self.push(id.to_string());
            }
            ExpressionKind::Call {
                receiver,
                arguments,
            } => {
                self.push(format!("{receiver}("));
                for (index, argument) in arguments.iter().enumerate() {
                    if index > 0 {
                        self.push(", ");
                    }
                    self.push(format!("{argument}"));
                }
                self.push(")");
            }
            ExpressionKind::Error => panic!("Error expression found."),
        }
    }

    fn lower_type(&mut self, type_: &Type) {
        match type_ {
            Type::Named(name) => self.push(name),
            Type::Error => panic!("Error type found."),
        }
    }

    fn push(&mut self, s: impl AsRef<str>) {
        self.c.push_str(s.as_ref());
    }
}
