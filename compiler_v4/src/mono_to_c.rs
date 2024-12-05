use crate::{
    hir::BuiltinFunction,
    mono::{
        Body, BodyOrBuiltin, Expression, ExpressionKind, Function, Id, Lambda, Mono,
        TypeDeclaration,
    },
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

        self.push("/// Type Forward Declarations\n\n");
        self.lower_type_forward_declarations();
        self.push("\n");

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

    fn lower_type_forward_declarations(&mut self) {
        for name in self.mono.type_declarations.keys() {
            self.push(format!("typedef struct {name} {name};\n"));
        }
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
                        "Int" => {
                            assert!(type_arguments.is_empty());
                            self.push("uint64_t value;\n");
                        }
                        "List" => {
                            assert_eq!(type_arguments.len(), 1);
                            self.push("uint64_t length;\n");
                            self.push(format!("{}** values;\n", type_arguments[0]));
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
                TypeDeclaration::Function {
                    parameter_types,
                    return_type,
                } => {
                    self.push("void* closure;\n");
                    self.push(format!("{return_type}* (*function)(void*"));
                    for parameter_type in parameter_types.iter() {
                        self.push(format!(", {parameter_type}*"));
                    }
                    self.push(");\n");
                    self.push("};\n");
                }
            }
        }
    }

    fn lower_assignment_declarations(&mut self) {
        for (name, assignment) in &self.mono.assignments {
            self.lower_lambda_declarations_in(name, &assignment.body);
            self.push(format!("{}* {name};\n", &assignment.type_));
        }
    }
    fn lower_assignment_definitions(&mut self) {
        for (name, assignment) in &self.mono.assignments {
            self.lower_lambda_definitions_in(name, &assignment.body);

            self.push(format!("void {name}$init() {{\n"));
            self.lower_body_expressions(name, &assignment.body);
            self.push(format!(
                "{name} = {};\n}}\n\n",
                assignment.body.return_value_id(),
            ));
        }
    }

    fn lower_function_declarations(&mut self) {
        for (name, function) in &self.mono.functions {
            if let BodyOrBuiltin::Body(body) = &function.body {
                self.lower_lambda_declarations_in(name, body);
            }

            self.lower_function_signature(name, function);
            self.push(";\n");
        }
    }
    fn lower_lambda_declarations_in(&mut self, declaration_name: &str, body: &'h Body) {
        Self::visit_lambdas_inside_body(body, &mut |id, lambda| {
            self.push(format!(
                "typedef struct {declaration_name}$lambda{id}_closure {declaration_name}$lambda{id}_closure;\n",
            ));

            self.lower_lambda_signature(declaration_name, id, lambda);
            self.push(";\n");
        });
    }
    fn lower_function_definitions(&mut self) {
        for (name, function) in &self.mono.functions {
            if let BodyOrBuiltin::Body(body) = &function.body {
                self.lower_lambda_definitions_in(name, body);
            }

            self.lower_function_signature(name, function);
            self.push(" {\n");
            self.lower_body_or_builtin(name, function);
            self.push("}\n\n");
        }
    }
    fn lower_lambda_definitions_in(&mut self, declaration_name: &str, body: &'h Body) {
        Self::visit_lambdas_inside_body(body, &mut |id, lambda| {
            let closure = lambda.closure_with_types(body);

            self.push(format!("struct {declaration_name}$lambda{id}_closure {{"));
            for (id, type_) in &closure {
                self.push(format!("{type_} {id}; "));
            }
            self.push("};\n");

            self.lower_lambda_signature(declaration_name, id, lambda);
            self.push(" {\n");
            self.push(format!(
                "{declaration_name}$lambda{id}_closure* closure = raw_closure;\n"
            ));
            for (id, type_) in &closure {
                self.push(format!("{type_}* {id} = closure->{id};\n"));
            }
            self.push("}\n");
        });
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
    fn lower_lambda_signature(&mut self, declaration_name: &str, id: Id, lambda: &Lambda) {
        self.push(format!(
            "{}* {declaration_name}$lambda{id}_function(void* raw_closure",
            &lambda.body.return_type()
        ));
        for parameter in lambda.parameters.iter() {
            self.push(format!(", {}* {}", &parameter.type_, parameter.id));
        }
        self.push(")");
    }

    fn visit_lambdas_inside_body(body: &'h Body, visitor: &mut impl FnMut(Id, &'h Lambda)) {
        for (id, _, expression) in &body.expressions {
            match &expression.kind {
                ExpressionKind::Int(_)
                | ExpressionKind::Text(_)
                | ExpressionKind::CreateStruct { .. }
                | ExpressionKind::StructAccess { .. }
                | ExpressionKind::CreateEnum { .. }
                | ExpressionKind::GlobalAssignmentReference(_)
                | ExpressionKind::LocalReference(_)
                | ExpressionKind::CallFunction { .. }
                | ExpressionKind::CallLambda { .. }
                | ExpressionKind::Switch { .. } => {}
                ExpressionKind::Lambda(lambda) => {
                    Self::visit_lambdas_inside_body(&lambda.body, visitor);
                    visitor(*id, lambda);
                }
            }
        }
    }

    fn lower_body_or_builtin(&mut self, declaration_name: &str, function: &Function) {
        match &function.body {
            BodyOrBuiltin::Builtin {
                builtin_function,
                substitutions,
            } => {
                self.push("// builtin function\n");
                match builtin_function {
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
                    BuiltinFunction::ListFilled => self.push(format!(
                        "\
                        {list_type}* result_pointer = malloc(sizeof({list_type}));
                        result_pointer->length = {length}->value;
                        result_pointer->values = malloc({length}->value * sizeof({item_type}));
                        for (uint64_t i = 0; i < {length}->value; i++) {{
                            result_pointer->values[i] = {item};
                        }}
                        return result_pointer;",
                        item_type = substitutions["T"],
                        list_type = function.return_type,
                        length = function.parameters[0].id,
                        item = function.parameters[1].id,
                    )),
                    BuiltinFunction::ListGet => self.push(format!(
                        "\
                        {return_type}* result_pointer = malloc(sizeof({return_type}));
                        if (0 <= {index}->value && {index}->value < {list}->length) {{
                            result_pointer->variant = {return_type}_some;
                            result_pointer->value.some = {list}->values[{index}->value];
                        }} else {{
                            result_pointer->variant = {return_type}_none;
                        }}
                        return result_pointer;",
                        return_type = function.return_type,
                        list = function.parameters[0].id,
                        index = function.parameters[1].id,
                    )),
                    BuiltinFunction::ListInsert => self.push(format!(
                        "\
                        if (0 > {index}->value || {index}->value > {list}->length) {{
                            char* message_format = \"Index out of bounds: Tried inserting at index %ld in list of length %ld.\";
                            int length = snprintf(NULL, 0, message_format, {index}->value, {list}->length);
                            char *message = malloc(length + 1);
                            snprintf(message, length + 1, message_format, {index}->value, {list}->length);

                            Text *message_pointer = malloc(sizeof(Text));
                            message_pointer->value = message;
                            builtinPanic$$Text(message_pointer);
                        }}

                        {list_type}* result_pointer = malloc(sizeof({list_type}));
                        result_pointer->length = {list}->length + 1;
                        result_pointer->values = malloc(result_pointer->length * sizeof({item_type}));
                        memcpy(result_pointer->values, {list}->values, {index}->value * sizeof({item_type}));
                        result_pointer->values[{index}->value] = {item};
                        memcpy(result_pointer->values + {index}->value + 1, {list}->values + {index}->value, ({list}->length - {index}->value) * sizeof({item_type}));
                        return result_pointer;",
                        item_type = substitutions["T"],
                        list_type = function.return_type,
                        list = function.parameters[0].id,
                        index = function.parameters[1].id,
                        item = function.parameters[2].id,
                    )),
                    BuiltinFunction::ListLength => self.push(format!(
                        "\
                        Int* result_pointer = malloc(sizeof(Int));
                        result_pointer->value = {list}->length;
                        return result_pointer;",
                        list = function.parameters[0].id,
                    )),
                    BuiltinFunction::ListOf0 => self.push(format!(
                        "\
                        {list_type}* result_pointer = malloc(sizeof({list_type}));
                        result_pointer->length = 0;
                        result_pointer->values = nullptr;
                        return result_pointer;",
                        list_type = function.return_type,
                    )),
                    BuiltinFunction::ListOf1 => self.push(format!(
                        "\
                        {list_type}* result_pointer = malloc(sizeof({list_type}));
                        result_pointer->length = 1;
                        result_pointer->values = malloc(sizeof({item_type}));
                        result_pointer->values[0] = {item0};
                        return result_pointer;",
                        item_type = substitutions["T"],
                        list_type = function.return_type,
                        item0 = function.parameters[0].id,
                    )),
                    BuiltinFunction::ListOf2 => self.push(format!(
                        "\
                        {list_type}* result_pointer = malloc(sizeof({list_type}));
                        result_pointer->length = 2;
                        result_pointer->values = malloc(2 * sizeof({item_type}));
                        result_pointer->values[0] = {item0};
                        result_pointer->values[1] = {item1};
                        return result_pointer;",
                        item_type = substitutions["T"],
                        list_type = function.return_type,
                        item0 = function.parameters[0].id,
                        item1 = function.parameters[1].id,
                    )),
                    BuiltinFunction::ListOf3 => self.push(format!(
                        "\
                        {list_type}* result_pointer = malloc(sizeof({list_type}));
                        result_pointer->length = 3;
                        result_pointer->values = malloc(3 * sizeof({item_type}));
                        result_pointer->values[0] = {item0};
                        result_pointer->values[1] = {item1};
                        result_pointer->values[2] = {item2};
                        return result_pointer;",
                        item_type = substitutions["T"],
                        list_type = function.return_type,
                        item0 = function.parameters[0].id,
                        item1 = function.parameters[1].id,
                        item2 = function.parameters[2].id,
                    )),
                    BuiltinFunction::ListOf4 => self.push(format!(
                        "\
                        {list_type}* result_pointer = malloc(sizeof({list_type}));
                        result_pointer->length = 4;
                        result_pointer->values = malloc(4 * sizeof({item_type}));
                        result_pointer->values[0] = {item0};
                        result_pointer->values[1] = {item1};
                        result_pointer->values[2] = {item2};
                        result_pointer->values[3] = {item3};
                        return result_pointer;",
                        item_type = substitutions["T"],
                        list_type = function.return_type,
                        item0 = function.parameters[0].id,
                        item1 = function.parameters[1].id,
                        item2 = function.parameters[2].id,
                        item3 = function.parameters[3].id,
                    )),
                    BuiltinFunction::ListOf5 => self.push(format!(
                        "\
                        {list_type}* result_pointer = malloc(sizeof({list_type}));
                        result_pointer->length = 5;
                        result_pointer->values = malloc(5 * sizeof({item_type}));
                        result_pointer->values[0] = {item0};
                        result_pointer->values[1] = {item1};
                        result_pointer->values[2] = {item2};
                        result_pointer->values[3] = {item3};
                        result_pointer->values[4] = {item4};
                        return result_pointer;",
                        item_type = substitutions["T"],
                        list_type = function.return_type,
                        item0 = function.parameters[0].id,
                        item1 = function.parameters[1].id,
                        item2 = function.parameters[2].id,
                        item3 = function.parameters[3].id,
                        item4 = function.parameters[4].id,
                    )),
                    BuiltinFunction::ListRemoveAt => self.push(format!(
                        "\
                        if (0 > {index}->value || {index}->value >= {list}->length) {{
                            char* message_format = \"Index out of bounds: Tried removing item at index %ld from list of length %ld.\";
                            int length = snprintf(NULL, 0, message_format, {index}->value, {list}->length);
                            char *message = malloc(length + 1);
                            snprintf(message, length + 1, message_format, {index}->value, {list}->length);

                            Text *message_pointer = malloc(sizeof(Text));
                            message_pointer->value = message;
                            builtinPanic$$Text(message_pointer);
                        }}

                        {list_type}* result_pointer = malloc(sizeof({list_type}));
                        result_pointer->length = {list}->length - 1;
                        result_pointer->values = malloc(result_pointer->length * sizeof({item_type}));
                        memcpy(result_pointer->values, {list}->values, {index}->value * sizeof({item_type}));
                        memcpy(result_pointer->values + {index}->value, {list}->values + {index}->value + 1, ({list}->length - {index}->value - 1) * sizeof({item_type}));
                        return result_pointer;",
                        item_type = substitutions["T"],
                        list_type = function.return_type,
                        list = function.parameters[0].id,
                        index = function.parameters[1].id,
                    )),
                    BuiltinFunction::ListReplace => self.push(format!(
                        "\
                        if (0 > {index}->value || {index}->value >= {list}->length) {{
                            char* message_format = \"Index out of bounds: Tried replacing index %ld in list of length %ld.\";
                            int length = snprintf(NULL, 0, message_format, {index}->value, {list}->length);
                            char *message = malloc(length + 1);
                            snprintf(message, length + 1, message_format, {index}->value, {list}->length);

                            Text *message_pointer = malloc(sizeof(Text));
                            message_pointer->value = message;
                            builtinPanic$$Text(message_pointer);
                        }}

                        {list_type}* result_pointer = malloc(sizeof({list_type}));
                        result_pointer->length = {list}->length;
                        result_pointer->values = malloc(result_pointer->length * sizeof({item_type}));
                        memcpy(result_pointer->values, {list}->values, {list}->length * sizeof({item_type}));
                        result_pointer->values[{index}->value] = {new_item};
                        return result_pointer;",
                        item_type = substitutions["T"],
                        list_type = function.return_type,
                        list = function.parameters[0].id,
                        index = function.parameters[1].id,
                        new_item = function.parameters[2].id,
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
            BodyOrBuiltin::Body(body) => self.lower_body(declaration_name, body),
        }
    }
    fn lower_body(&mut self, declaration_name: &str, body: &Body) {
        self.lower_body_expressions(declaration_name, body);
        self.push(format!("return {};", body.return_value_id()));
    }
    fn lower_body_expressions(&mut self, declaration_name: &str, body: &Body) {
        for (id, name, expression) in &body.expressions {
            if let Some(name) = name {
                self.push(format!("// {name}\n"));
            }

            self.lower_expression(declaration_name, *id, expression);
            self.push("\n");
        }
    }
    fn lower_expression(&mut self, declaration_name: &str, id: Id, expression: &Expression) {
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
                    self.push(format!("\n{id}->value.{variant} = {value};"));
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
            ExpressionKind::CallFunction {
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
            ExpressionKind::CallLambda { lambda, arguments } => {
                self.push(format!(
                    "{}* {id} = {lambda}->function({lambda}->closure",
                    &expression.type_
                ));
                for argument in arguments.iter() {
                    self.push(format!(", {argument}"));
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
                        self.push(format!(
                            "{variant_type}* {value_id} = {value}->value.{};\n",
                            case.variant,
                        ));
                    }

                    self.lower_body_expressions(declaration_name, &case.body);

                    self.push(format!("{id} = {};\n", case.body.return_value_id()));

                    self.push("break;");
                }
                self.push("}");
            }
            ExpressionKind::Lambda(lambda) => {
                self.push(format!("{declaration_name}$lambda{id}_closure* {id}_closure = malloc(sizeof({declaration_name}$lambda{id}_closure));\n",));
                for referenced_id in lambda.closure().iter().sorted() {
                    self.push(format!(
                        "{id}_closure->{referenced_id} = {referenced_id};\n"
                    ));
                }
                self.push(format!(
                    "{type_}* {id} = malloc(sizeof({type_}));",
                    type_ = &expression.type_,
                ));
                self.push(format!("{id}->closure = {id}_closure;"));
                self.push(format!(
                    "{id}->function = {declaration_name}$lambda{id}_function;",
                ));
            }
        }
    }

    fn push(&mut self, s: impl AsRef<str>) {
        self.c.push_str(s.as_ref());
    }
}
