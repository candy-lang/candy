use crate::{
    hir::BuiltinFunction,
    mono::{Body, BodyOrBuiltin, Expression, ExpressionKind, Function, Id, Mono, TypeDeclaration},
};
use itertools::Itertools;

pub fn mono_to_c(mono: &Mono) -> String {
    let mut context = Context::new(mono);
    context.lower_mono();
    context.c
}

#[derive(Debug)]
struct Context<'h> {
    mono: &'h Mono,
    c: String,
}
impl<'h> Context<'h> {
    #[must_use]
    const fn new(mono: &'h Mono) -> Self {
        Self {
            mono,
            c: String::new(),
        }
    }

    fn lower_mono(&mut self) {
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

        self.push("int main() {\n");
        for name in self.mono.assignment_initialization_order.iter() {
            self.push(format!("{name}$init();\n"));
        }
        self.push(format!(
            "return {}()->value;\n}}\n",
            self.mono.main_function,
        ));
    }

    fn lower_type_declarations(&mut self) {
        for (name, declaration) in &self.mono.type_declarations {
            self.push(format!("struct {name} {{\n"));
            match declaration {
                TypeDeclaration::Builtin {
                    name,
                    type_arguments,
                } => {
                    match name.as_ref() {
                        "Array" => {
                            assert_eq!(type_arguments.len(), 1);
                            self.push("uint64_t length;\n");
                            self.push(format!("{}** values;\n", type_arguments[0]));
                        }
                        "Int" => {
                            assert!(type_arguments.is_empty());
                            self.push("uint64_t value;\n");
                        }
                        "Text" => {
                            assert!(type_arguments.is_empty());
                            self.push("char* value;\n");
                        }
                        _ => panic!("Unknown builtin type: {name}"),
                    }
                    self.push("};\n");
                }
                TypeDeclaration::Struct { fields } => {
                    for (name, type_) in fields.iter() {
                        self.push(format!("{type_}* {name}; "));
                    }
                    self.push("};\n");
                }
                TypeDeclaration::Enum { variants } => {
                    if !variants.is_empty() {
                        self.push("enum {");
                        for variant in variants.iter() {
                            self.push(format!("{name}_{},", variant.name));
                        }
                        self.push("} variant;\n");
                    }

                    self.push("union {");
                    for variant in variants.iter() {
                        if let Some(value_type) = &variant.value_type {
                            self.push(format!("{value_type}* {};", variant.name));
                        }
                    }
                    self.push("} value;\n};\n");
                }
            }
            self.push(format!("typedef struct {name} {name};\n"));
        }
    }

    fn lower_assignment_declarations(&mut self) {
        for (name, assignment) in &self.mono.assignments {
            self.push(format!("{}* {name};\n", &assignment.type_));
        }
    }
    fn lower_assignment_definitions(&mut self) {
        for (name, assignment) in &self.mono.assignments {
            self.push(format!("void {name}$init() {{\n"));
            self.lower_body_expressions(&assignment.body);
            self.push(format!(
                "{name} = {};\n}}\n\n",
                assignment.body.return_value_id(),
            ));
        }
    }

    fn lower_function_declarations(&mut self) {
        for (name, function) in &self.mono.functions {
            self.lower_function_signature(name, function);
            self.push(";\n");

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
        for (name, function) in &self.mono.functions {
            self.lower_function_signature(name, function);
            self.push(" {\n");
            self.lower_body_or_builtin(function);
            self.push("}\n\n");
        }
    }
    fn lower_function_signature(&mut self, name: &str, function: &Function) {
        self.push(format!("{}* {name}(", &function.return_type));
        for (index, parameter) in function.parameters.iter().enumerate() {
            if index > 0 {
                self.push(", ");
            }
            self.push(format!("{}* {}", &parameter.type_, parameter.id));
        }
        self.push(")");
    }
    fn lower_body_or_builtin(&mut self, function: &Function) {
        match &function.body {
            BodyOrBuiltin::Builtin(builtin_function) => {
                self.push("// builtin function\n");
                match builtin_function {
                    BuiltinFunction::ArrayFilled => self.push(format!(
                        "\
                        {array_type}* result_pointer = malloc(sizeof({array_type}));
                        result_pointer->length = {length}->value;
                        result_pointer->values = malloc({length}->value * sizeof({array_type}));
                        for (uint64_t i = 0; i < {length}->value; i++) {{
                            result_pointer->values[i] = {item};
                        }}
                        return result_pointer;",
                        array_type = function.return_type,
                        length = function.parameters[0].id,
                        item = function.parameters[1].id,
                    )),
                    BuiltinFunction::ArrayLength => self.push(format!(
                        "\
                        Int* result_pointer = malloc(sizeof(Int));
                        result_pointer->value = {array}->length;
                        return result_pointer;",
                        array = function.parameters[0].id,
                    )),
                    BuiltinFunction::IntAdd => self.push(format!(
                        "\
                        Int* result_pointer = malloc(sizeof(Int));
                        result_pointer->value = {a}->value + {b}->value;
                        return result_pointer;",
                        a = function.parameters[0].id,
                        b = function.parameters[1].id,
                    )),
                    BuiltinFunction::IntCompareTo => self.push(format!(
                        "\
                        Ordering* result_pointer = malloc(sizeof(Ordering));
                        result_pointer->variant = {a}->value < {b}->value    ? Ordering_less
                                                  : {a}->value == {b}->value ? Ordering_equal
                                                                             : Ordering_greater;
                        return result_pointer;",
                        a = function.parameters[0].id,
                        b = function.parameters[1].id,
                    )),
                    BuiltinFunction::IntSubtract => self.push(format!(
                        "\
                        Int* result_pointer = malloc(sizeof(Int));
                        result_pointer->value = {a}->value - {b}->value;
                        return result_pointer;",
                        a = function.parameters[0].id,
                        b = function.parameters[1].id,
                    )),
                    BuiltinFunction::IntToText => self.push(format!(
                        "\
                        int length = snprintf(NULL, 0, \"%ld\", {int}->value);
                        char* result = malloc(length + 1);
                        snprintf(result, length + 1, \"%ld\", {int}->value);
                        
                        Text* result_pointer = malloc(sizeof(Text));
                        result_pointer->value = result;
                        return result_pointer;",
                        int = function.parameters[0].id,
                    )),
                    BuiltinFunction::Panic => {
                        self.push(format!(
                            "\
                            fputs({}->value, stderr);
                            exit(1);",
                            function.parameters[0].id,
                        ));
                    }
                    BuiltinFunction::Print => {
                        self.push(format!(
                            "\
                            puts({}->value);
                            Nothing *_1 = malloc(sizeof(Nothing));
                            return _1;",
                            function.parameters[0].id,
                        ));
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
                        a = function.parameters[0].id,
                        b = function.parameters[1].id,
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
                self.push(format!(
                    "{}* {id} = malloc(sizeof({}));",
                    &expression.type_, &expression.type_,
                ));
                self.push(format!("{id}->value = {int};"));
            }
            ExpressionKind::Text(text) => {
                self.push(format!(
                    "{}* {id} = malloc(sizeof({}));",
                    &expression.type_, &expression.type_,
                ));
                // TODO: escape text
                self.push(format!("{id}->value = \"{text}\";"));
            }
            ExpressionKind::CreateStruct { struct_, fields } => {
                let TypeDeclaration::Struct {
                    fields: type_fields,
                } = &self.mono.type_declarations[struct_]
                else {
                    unreachable!();
                };

                self.push(format!(
                    "{}* {id} = malloc(sizeof({}));",
                    &expression.type_, &expression.type_,
                ));
                for ((name, _), value) in type_fields.iter().zip_eq(fields.iter()) {
                    self.push(format!("\n{id}->{name} = {value};"));
                }
            }
            ExpressionKind::StructAccess { struct_, field } => {
                self.push(format!("{}* {id} = {struct_}->{field};", &expression.type_));
            }
            ExpressionKind::CreateEnum {
                enum_,
                variant,
                value,
            } => {
                self.push(format!(
                    "{}* {id} = malloc(sizeof({}));",
                    &expression.type_, &expression.type_,
                ));
                self.push(format!("{id}->variant = {enum_}_{variant};"));
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
            ExpressionKind::GlobalAssignmentReference(assignment) => {
                self.push(format!("{}* {id} = {assignment};", &expression.type_));
            }
            ExpressionKind::LocalReference(referenced_id) => {
                self.push(format!("{}* {id} = {referenced_id};", &expression.type_));
            }
            ExpressionKind::Call {
                function,
                arguments,
            } => {
                self.push(format!("{}* {id} = {function}(", &expression.type_));
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
                let TypeDeclaration::Enum { variants } = &self.mono.type_declarations[enum_] else {
                    unreachable!();
                };

                self.push(format!("{}* {id};\n", &expression.type_));

                self.push(format!("switch ({value}->variant) {{"));
                for case in cases.iter() {
                    self.push(format!("case {enum_}_{}:\n", case.variant));
                    if let Some(value_id) = case.value_id {
                        let variant_type = variants
                            .iter()
                            .find(|variant| variant.name == case.variant)
                            .unwrap()
                            .value_type
                            .as_ref()
                            .unwrap();
                        self.push(format!("{variant_type}* {value_id} = {value}->value;\n"));
                    }

                    self.lower_body_expressions(&case.body);

                    self.push(format!("{id} = {};\n", case.body.return_value_id()));

                    self.push("break;");
                }
                self.push("}");
            }
        }
    }

    fn push(&mut self, s: impl AsRef<str>) {
        self.c.push_str(s.as_ref());
    }
}
