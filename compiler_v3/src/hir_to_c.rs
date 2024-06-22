use crate::hir::{Assignment, Body, BuiltinFunction, Expression, Hir, Parameter, Type};
use core::panic;

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
        self.push("#include <stdio.h>\n\n");

        for (id, name, assignment) in &self.hir.assignments {
            self.push(format!("// {name}\n"));
            match assignment {
                Assignment::Value { type_, value } => 'case: {
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
                Assignment::Function {
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
            Expression::Symbol(_) => todo!(),
            Expression::Int(int) => self.push(format!("{int}")),
            // TODO: escape text
            Expression::Text(text) => self.push(format!("\"{text}\"")),
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

            Expression::Type(_) => panic!("Should have been resolved to a value."),
            Expression::Error => panic!("Error expression found."),
        }
    }

    // fn lower_expression_to_type_definition(&mut self, body: &Body) {
    //     self.lower_expression_to_type_helper(body.return_value_id())
    // }
    fn lower_type(&mut self, type_: &Type) {
        self.push("const ");
        match type_ {
            Type::Type => todo!(),
            Type::Symbol => todo!(),
            Type::Int => self.push("int64_t"),
            Type::Text => self.push("char*"),
            Type::Struct(struct_) => {
                self.push("struct { ");
                for (name, type_) in struct_.iter() {
                    self.lower_type(type_);
                    self.push(format!(" {name}; "));
                }
                self.push("}");
            }
            Type::Function {
                parameter_types,
                return_type,
            } => {
                self.lower_type(return_type);
                self.push(" (*)(");
                for (i, parameter_type) in parameter_types.iter().enumerate() {
                    if i != 0 {
                        self.push(", ");
                    }
                    self.lower_type(parameter_type);
                }
                self.push(")");
            }
            Type::Error => todo!(),
            // Expression::Symbol(symbol) => self.push(format!("type_symbol_{symbol}")),
            // // self.push(format!("type_int_{int}")),
            // Expression::Int(_) | Expression::IntType => self.push("int64_t"),
            // // let text_type = self
            // //     .text_types
            // //     .iter()
            // //     .position(|t| t == text)
            // //     .unwrap_or_else(|| {
            // //         self.text_types.push(text);
            // //         self.text_types.len() - 1
            // //     });
            // // self.push(format!("text_type_{text_type}"));
            // Expression::Text(_) | Expression::TextType => self.push("char*"),
            // Expression::Struct(struct_) => {
            //     self.push("struct { ");
            //     for (name, id) in struct_.iter() {
            //         self.lower_expression_to_type_helper(*id);
            //         self.push(format!(" {name}; "));
            //     }
            //     self.push("}");
            // }
            // Expression::StructAccess { .. }
            // | Expression::ValueWithTypeAnnotation { .. }
            // | Expression::Reference(_)
            // | Expression::Call(_, _)
            // | Expression::BuiltinEquals
            // | Expression::BuiltinPrint => {
            //     panic!("Should have been resolved to a type.")
            // }
            // Expression::Error => panic!("Error expression found."),
        }
    }

    fn push(&mut self, s: impl AsRef<str>) {
        self.c.push_str(s.as_ref());
    }
}
