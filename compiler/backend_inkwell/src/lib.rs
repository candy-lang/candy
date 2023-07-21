use candy_frontend::mir::{Body, Id, Mir};
use inkwell::{
    builder::Builder,
    context::Context,
    module::{Linkage, Module},
    support::LLVMString,
    types::{BasicType, StructType},
    values::{ArrayValue, BasicValue, BasicValueEnum, FunctionValue, GlobalValue},
    AddressSpace,
};

pub use inkwell;
use std::{
    collections::{HashMap, HashSet},
    path::Path,
    rc::Rc,
    sync::Arc,
};

#[derive(Clone)]
struct FunctionInfo<'ctx> {
    function_value: Rc<FunctionValue<'ctx>>,
    parameters: Vec<Id>,
    captured_ids: Vec<Id>,
    env_type: Option<StructType<'ctx>>,
}

pub struct CodeGen<'ctx> {
    context: &'ctx Context,
    module: Module<'ctx>,
    builder: Builder<'ctx>,
    mir: Arc<Mir>,
    tags: HashMap<String, Option<Id>>,
    values: HashMap<Id, Rc<GlobalValue<'ctx>>>,
    locals: HashMap<Id, Rc<BasicValueEnum<'ctx>>>,
    functions: HashMap<Id, FunctionInfo<'ctx>>,
    unrepresented_ids: HashSet<Id>,
}

impl<'ctx> CodeGen<'ctx> {
    pub fn new(
        context: &'ctx Context,
        module: Module<'ctx>,
        builder: Builder<'ctx>,
        mir: Arc<Mir>,
    ) -> Self {
        Self {
            context,
            module,
            builder,
            mir,
            tags: HashMap::new(),
            values: HashMap::new(),
            locals: HashMap::new(),
            functions: HashMap::new(),
            unrepresented_ids: HashSet::new(),
        }
    }

    pub fn compile(
        mut self,
        path: &Path,
        print_llvm_ir: bool,
        print_main_output: bool,
    ) -> Result<(), LLVMString> {
        let i32_type = self.context.i32_type();
        let i128_type = self.context.i64_type();
        let i8_type = self.context.i8_type();
        let void_type = self.context.void_type();

        let candy_type = self.context.opaque_struct_type("candy_type");
        let candy_type_ptr = candy_type.ptr_type(AddressSpace::default());

        let make_int_fn_type = candy_type_ptr.fn_type(&[i128_type.into()], false);
        self.module
            .add_function("make_candy_int", make_int_fn_type, Some(Linkage::External));
        let make_tag_fn_type =
            candy_type_ptr.fn_type(&[i8_type.ptr_type(AddressSpace::default()).into()], false);
        self.module
            .add_function("make_candy_tag", make_tag_fn_type, Some(Linkage::External));
        self.module
            .add_function("make_candy_text", make_tag_fn_type, Some(Linkage::External));
        let make_function_fn_type =
            candy_type_ptr.fn_type(&[candy_type_ptr.into(), candy_type_ptr.into()], false);
        self.module.add_function(
            "make_candy_function",
            make_function_fn_type,
            Some(Linkage::External),
        );
        let panic_fn_type = void_type.fn_type(
            &[candy_type.ptr_type(AddressSpace::default()).into()],
            false,
        );
        self.module.add_function("candy_panic", panic_fn_type, None);
        let free_fn = self
            .module
            .add_function("free_candy_value", panic_fn_type, None);
        let print_fn = self
            .module
            .add_function("print_candy_value", panic_fn_type, None);

        let main_type = i32_type.fn_type(&[], false);
        let main_fn = self.module.add_function("main", main_type, None);
        let block = self.context.append_basic_block(main_fn, "entry");

        let main_info = FunctionInfo {
            function_value: Rc::new(main_fn),
            parameters: vec![],
            captured_ids: vec![],
            env_type: None,
        };

        self.builder.position_at_end(block);
        self.compile_mir(&self.mir.body.clone(), &main_info);
        self.builder.position_at_end(block);
        let environment = self
            .module
            .add_global(candy_type_ptr, None, "candy_environment");
        let main_res_ptr = self.builder.build_call(
            self.module.get_function("candy_main").unwrap(),
            &[environment.as_basic_value_enum().into()],
            "",
        );
        if print_main_output {
            self.builder.build_call(
                print_fn,
                &[main_res_ptr.try_as_basic_value().unwrap_left().into()],
                "",
            );
            for value in self.module.get_globals() {
                let val = self
                    .builder
                    .build_load(candy_type_ptr, value.as_pointer_value(), "");
                self.builder.build_call(free_fn, &[val.into()], "");
            }
        }
        let ret_value = i32_type.const_int(0, false);
        self.builder.build_return(Some(&ret_value));
        if print_llvm_ir {
            self.module.print_to_stderr();
        }
        self.module.verify()?;
        self.module.write_bitcode_to_path(path);
        Ok(())
    }

    fn compile_mir(&mut self, mir: &Body, function_ctx: &FunctionInfo<'ctx>) {
        for (idx, (id, expr)) in mir.expressions.iter().enumerate() {
            match expr {
                candy_frontend::mir::Expression::Int(value) => {
                    let i128_type = self.context.i64_type();
                    let v = i128_type.const_int(value.try_into().unwrap(), false);
                    let candy_type = self.module.get_struct_type("candy_type").unwrap();
                    let candy_type_ptr = candy_type.ptr_type(AddressSpace::default());
                    let global =
                        self.module
                            .add_global(candy_type_ptr, None, &format!("num_{value}"));
                    global.set_initializer(&candy_type_ptr.const_null());
                    let make_candy_int = self.module.get_function("make_candy_int").unwrap();
                    let call = self.builder.build_call(make_candy_int, &[v.into()], "");

                    self.builder.build_store(
                        global.as_pointer_value(),
                        call.try_as_basic_value().unwrap_left(),
                    );

                    self.values.insert(*id, Rc::new(global));

                    if idx == mir.expressions.len() - 1 {
                        self.builder
                            .build_return(Some(&global.as_basic_value_enum()));
                    }
                }
                candy_frontend::mir::Expression::Text(text) => {
                    let i32_type = self.context.i32_type();
                    let i8_type = self.context.i8_type();
                    let candy_type_ptr = self
                        .module
                        .get_struct_type("candy_type")
                        .unwrap()
                        .ptr_type(AddressSpace::default());

                    let global = self.module.add_global(candy_type_ptr, None, text);
                    let v = self.make_str_literal(text);
                    let len = i32_type.const_int(text.len() as u64, false);
                    let arr_alloc = self.builder.build_array_alloca(i8_type, len, "");
                    self.builder.build_store(arr_alloc, v);
                    let cast = self.builder.build_bitcast(
                        arr_alloc,
                        i8_type.ptr_type(AddressSpace::default()),
                        "",
                    );
                    let make_candy_text = self.module.get_function("make_candy_text").unwrap();
                    let call = self.builder.build_call(make_candy_text, &[cast.into()], "");

                    self.builder.build_store(
                        global.as_pointer_value(),
                        call.try_as_basic_value().unwrap_left(),
                    );

                    global.set_initializer(&candy_type_ptr.const_null());
                    self.values.insert(*id, Rc::new(global));

                    if idx == mir.expressions.len() - 1 {
                        self.builder
                            .build_return(Some(&global.as_basic_value_enum()));
                    }
                }
                candy_frontend::mir::Expression::Tag { symbol, value } => {
                    self.tags.insert(symbol.clone(), *value);
                    let i32_type = self.context.i32_type();
                    let i8_type = self.context.i8_type();
                    let candy_type_ptr = self
                        .module
                        .get_struct_type("candy_type")
                        .unwrap()
                        .ptr_type(AddressSpace::default());

                    let global = self.module.add_global(candy_type_ptr, None, symbol);
                    let v = self.make_str_literal(symbol);
                    let len = i32_type.const_int(symbol.len() as u64, false);
                    let arr_alloc = self.builder.build_array_alloca(i8_type, len, "");
                    self.builder.build_store(arr_alloc, v);
                    let cast = self.builder.build_bitcast(
                        arr_alloc,
                        i8_type.ptr_type(AddressSpace::default()),
                        "",
                    );
                    let make_candy_tag = self.module.get_function("make_candy_tag").unwrap();
                    let call = self.builder.build_call(make_candy_tag, &[cast.into()], "");

                    self.builder.build_store(
                        global.as_pointer_value(),
                        call.try_as_basic_value().unwrap_left(),
                    );

                    global.set_initializer(&candy_type_ptr.const_null());
                    self.values.insert(*id, Rc::new(global));

                    if idx == mir.expressions.len() - 1 {
                        self.builder
                            .build_return(Some(&global.as_basic_value_enum()));
                    }
                }
                candy_frontend::mir::Expression::Builtin(builtin) => {
                    match builtin {
                        candy_frontend::builtin_functions::BuiltinFunction::ChannelCreate => {
                            todo!()
                        }
                        candy_frontend::builtin_functions::BuiltinFunction::ChannelSend => todo!(),
                        candy_frontend::builtin_functions::BuiltinFunction::ChannelReceive => {
                            todo!()
                        }
                        candy_frontend::builtin_functions::BuiltinFunction::Equals => {
                            let candy_type_ptr = self
                                .module
                                .get_struct_type("candy_type")
                                .unwrap()
                                .ptr_type(AddressSpace::default());

                            let fn_type = candy_type_ptr
                                .fn_type(&[candy_type_ptr.into(), candy_type_ptr.into()], false);
                            let function =
                                self.module
                                    .add_function("candy_builtin_equals", fn_type, None);
                            self.functions.insert(
                                *id,
                                FunctionInfo {
                                    function_value: Rc::new(function),
                                    parameters: vec![],
                                    captured_ids: vec![],
                                    env_type: None,
                                },
                            );
                        }
                        candy_frontend::builtin_functions::BuiltinFunction::FunctionRun => todo!(),
                        candy_frontend::builtin_functions::BuiltinFunction::GetArgumentCount => {
                            todo!()
                        }
                        candy_frontend::builtin_functions::BuiltinFunction::IfElse => {
                            let candy_type_ptr = self
                                .module
                                .get_struct_type("candy_type")
                                .unwrap()
                                .ptr_type(AddressSpace::default());

                            let fn_type = candy_type_ptr.fn_type(
                                &[
                                    candy_type_ptr.into(),
                                    candy_type_ptr.into(),
                                    candy_type_ptr.into(),
                                ],
                                false,
                            );
                            let function =
                                self.module
                                    .add_function("candy_builtin_ifelse", fn_type, None);
                            self.functions.insert(
                                *id,
                                FunctionInfo {
                                    function_value: Rc::new(function),
                                    parameters: vec![],
                                    captured_ids: vec![],
                                    env_type: None,
                                },
                            );
                        }
                        candy_frontend::builtin_functions::BuiltinFunction::IntAdd => {
                            let candy_type_ptr = self
                                .module
                                .get_struct_type("candy_type")
                                .unwrap()
                                .ptr_type(AddressSpace::default());

                            let fn_type = candy_type_ptr
                                .fn_type(&[candy_type_ptr.into(), candy_type_ptr.into()], false);
                            let function =
                                self.module
                                    .add_function("candy_builtin_int_add", fn_type, None);
                            self.functions.insert(
                                *id,
                                FunctionInfo {
                                    function_value: Rc::new(function),
                                    parameters: vec![],
                                    captured_ids: vec![],
                                    env_type: None,
                                },
                            );
                        }
                        candy_frontend::builtin_functions::BuiltinFunction::IntBitLength => {
                            let candy_type_ptr = self
                                .module
                                .get_struct_type("candy_type")
                                .unwrap()
                                .ptr_type(AddressSpace::default());

                            let fn_type = candy_type_ptr.fn_type(&[candy_type_ptr.into()], false);
                            let function = self.module.add_function(
                                "candy_builtin_int_bit_length",
                                fn_type,
                                None,
                            );
                            self.functions.insert(
                                *id,
                                FunctionInfo {
                                    function_value: Rc::new(function),
                                    parameters: vec![],
                                    captured_ids: vec![],
                                    env_type: None,
                                },
                            );
                        }
                        candy_frontend::builtin_functions::BuiltinFunction::IntBitwiseAnd => {
                            let candy_type_ptr = self
                                .module
                                .get_struct_type("candy_type")
                                .unwrap()
                                .ptr_type(AddressSpace::default());

                            let fn_type = candy_type_ptr
                                .fn_type(&[candy_type_ptr.into(), candy_type_ptr.into()], false);
                            let function = self.module.add_function(
                                "candy_builtin_int_bitwise_and",
                                fn_type,
                                None,
                            );
                            self.functions.insert(
                                *id,
                                FunctionInfo {
                                    function_value: Rc::new(function),
                                    parameters: vec![],
                                    captured_ids: vec![],
                                    env_type: None,
                                },
                            );
                        }
                        candy_frontend::builtin_functions::BuiltinFunction::IntBitwiseOr => {
                            let candy_type_ptr = self
                                .module
                                .get_struct_type("candy_type")
                                .unwrap()
                                .ptr_type(AddressSpace::default());

                            let fn_type = candy_type_ptr
                                .fn_type(&[candy_type_ptr.into(), candy_type_ptr.into()], false);
                            let function = self.module.add_function(
                                "candy_builtin_int_bitwise_or",
                                fn_type,
                                None,
                            );
                            self.functions.insert(
                                *id,
                                FunctionInfo {
                                    function_value: Rc::new(function),
                                    parameters: vec![],
                                    captured_ids: vec![],
                                    env_type: None,
                                },
                            );
                        }
                        candy_frontend::builtin_functions::BuiltinFunction::IntBitwiseXor => {
                            let candy_type_ptr = self
                                .module
                                .get_struct_type("candy_type")
                                .unwrap()
                                .ptr_type(AddressSpace::default());

                            let fn_type = candy_type_ptr
                                .fn_type(&[candy_type_ptr.into(), candy_type_ptr.into()], false);
                            let function = self.module.add_function(
                                "candy_builtin_int_bitwise_xor",
                                fn_type,
                                None,
                            );
                            self.functions.insert(
                                *id,
                                FunctionInfo {
                                    function_value: Rc::new(function),
                                    parameters: vec![],
                                    captured_ids: vec![],
                                    env_type: None,
                                },
                            );
                        }
                        candy_frontend::builtin_functions::BuiltinFunction::IntCompareTo => {
                            let candy_type_ptr = self
                                .module
                                .get_struct_type("candy_type")
                                .unwrap()
                                .ptr_type(AddressSpace::default());

                            let fn_type = candy_type_ptr
                                .fn_type(&[candy_type_ptr.into(), candy_type_ptr.into()], false);
                            let function = self.module.add_function(
                                "candy_builtin_int_compareto",
                                fn_type,
                                None,
                            );
                            self.functions.insert(
                                *id,
                                FunctionInfo {
                                    function_value: Rc::new(function),
                                    parameters: vec![],
                                    captured_ids: vec![],
                                    env_type: None,
                                },
                            );
                        }
                        candy_frontend::builtin_functions::BuiltinFunction::IntDivideTruncating => {
                            todo!()
                        }
                        candy_frontend::builtin_functions::BuiltinFunction::IntModulo => todo!(),
                        candy_frontend::builtin_functions::BuiltinFunction::IntMultiply => todo!(),
                        candy_frontend::builtin_functions::BuiltinFunction::IntParse => todo!(),
                        candy_frontend::builtin_functions::BuiltinFunction::IntRemainder => todo!(),
                        candy_frontend::builtin_functions::BuiltinFunction::IntShiftLeft => todo!(),
                        candy_frontend::builtin_functions::BuiltinFunction::IntShiftRight => {
                            todo!()
                        }
                        candy_frontend::builtin_functions::BuiltinFunction::IntSubtract => {
                            let candy_type_ptr = self
                                .module
                                .get_struct_type("candy_type")
                                .unwrap()
                                .ptr_type(AddressSpace::default());

                            let fn_type = candy_type_ptr
                                .fn_type(&[candy_type_ptr.into(), candy_type_ptr.into()], false);
                            let function = self.module.add_function(
                                "candy_builtin_int_subtract",
                                fn_type,
                                None,
                            );
                            self.functions.insert(
                                *id,
                                FunctionInfo {
                                    function_value: Rc::new(function),
                                    parameters: vec![],
                                    captured_ids: vec![],
                                    env_type: None,
                                },
                            );
                        }
                        candy_frontend::builtin_functions::BuiltinFunction::ListFilled => todo!(),
                        candy_frontend::builtin_functions::BuiltinFunction::ListGet => todo!(),
                        candy_frontend::builtin_functions::BuiltinFunction::ListInsert => todo!(),
                        candy_frontend::builtin_functions::BuiltinFunction::ListLength => todo!(),
                        candy_frontend::builtin_functions::BuiltinFunction::ListRemoveAt => todo!(),
                        candy_frontend::builtin_functions::BuiltinFunction::ListReplace => todo!(),
                        candy_frontend::builtin_functions::BuiltinFunction::Parallel => todo!(),
                        candy_frontend::builtin_functions::BuiltinFunction::Print => todo!(),
                        candy_frontend::builtin_functions::BuiltinFunction::StructGet => todo!(),
                        candy_frontend::builtin_functions::BuiltinFunction::StructGetKeys => {
                            todo!()
                        }
                        candy_frontend::builtin_functions::BuiltinFunction::StructHasKey => todo!(),
                        candy_frontend::builtin_functions::BuiltinFunction::TagGetValue => todo!(),
                        candy_frontend::builtin_functions::BuiltinFunction::TagHasValue => todo!(),
                        candy_frontend::builtin_functions::BuiltinFunction::TagWithoutValue => {
                            todo!()
                        }
                        candy_frontend::builtin_functions::BuiltinFunction::TextCharacters => {
                            todo!()
                        }
                        candy_frontend::builtin_functions::BuiltinFunction::TextConcatenate => {
                            todo!()
                        }
                        candy_frontend::builtin_functions::BuiltinFunction::TextContains => todo!(),
                        candy_frontend::builtin_functions::BuiltinFunction::TextEndsWith => todo!(),
                        candy_frontend::builtin_functions::BuiltinFunction::TextFromUtf8 => todo!(),
                        candy_frontend::builtin_functions::BuiltinFunction::TextGetRange => todo!(),
                        candy_frontend::builtin_functions::BuiltinFunction::TextIsEmpty => todo!(),
                        candy_frontend::builtin_functions::BuiltinFunction::TextLength => todo!(),
                        candy_frontend::builtin_functions::BuiltinFunction::TextStartsWith => {
                            todo!()
                        }
                        candy_frontend::builtin_functions::BuiltinFunction::TextTrimEnd => todo!(),
                        candy_frontend::builtin_functions::BuiltinFunction::TextTrimStart => {
                            todo!()
                        }
                        candy_frontend::builtin_functions::BuiltinFunction::ToDebugText => todo!(),
                        candy_frontend::builtin_functions::BuiltinFunction::Try => todo!(),
                        candy_frontend::builtin_functions::BuiltinFunction::TypeOf => {
                            let candy_type_ptr = self
                                .module
                                .get_struct_type("candy_type")
                                .unwrap()
                                .ptr_type(AddressSpace::default());

                            let fn_type = candy_type_ptr.fn_type(&[candy_type_ptr.into()], false);
                            let function =
                                self.module
                                    .add_function("candy_builtin_typeof", fn_type, None);
                            self.functions.insert(
                                *id,
                                FunctionInfo {
                                    function_value: Rc::new(function),
                                    parameters: vec![],
                                    captured_ids: vec![],
                                    env_type: None,
                                },
                            );
                        }
                    }
                    self.unrepresented_ids.insert(*id);
                }
                candy_frontend::mir::Expression::List(_) => todo!(),
                candy_frontend::mir::Expression::Struct(_s) => {
                    // Not yet implemented, but not allowed to panic
                }
                candy_frontend::mir::Expression::Reference(id) => {
                    if let Some(v) = self.values.get(id) {
                        let candy_type_ptr = self
                            .module
                            .get_struct_type("candy_type")
                            .unwrap()
                            .ptr_type(AddressSpace::default());
                        let value =
                            self.builder
                                .build_load(candy_type_ptr, v.as_pointer_value(), "");
                        self.builder.build_return(Some(&value));
                    }
                }
                candy_frontend::mir::Expression::HirId(hir_id) => {
                    let i32_type = self.context.i32_type();
                    let i8_type = self.context.i8_type();
                    let candy_type_ptr = self
                        .module
                        .get_struct_type("candy_type")
                        .unwrap()
                        .ptr_type(AddressSpace::default());

                    let text = format!("{hir_id}");

                    let global = self.module.add_global(candy_type_ptr, None, &text);
                    let v = self.make_str_literal(&text);
                    let len = i32_type.const_int(text.len() as u64, false);
                    let arr_alloc = self.builder.build_array_alloca(i8_type, len, "");
                    self.builder.build_store(arr_alloc, v);
                    let cast = self.builder.build_bitcast(
                        arr_alloc,
                        i8_type.ptr_type(AddressSpace::default()),
                        "",
                    );
                    let make_candy_text = self.module.get_function("make_candy_text").unwrap();
                    let call = self.builder.build_call(make_candy_text, &[cast.into()], "");

                    self.builder.build_store(
                        global.as_pointer_value(),
                        call.try_as_basic_value().unwrap_left(),
                    );

                    global.set_initializer(&candy_type_ptr.const_null());
                    self.values.insert(*id, Rc::new(global));
                }
                candy_frontend::mir::Expression::Function {
                    original_hirs,
                    parameters,
                    body,
                    responsible_parameter,
                } => {
                    let original_name = format!("{original_hirs:?}")
                        .replace('{', "")
                        .replace('}', "");
                    let name = if original_name.ends_with("main") {
                        "candy_main"
                    } else {
                        &original_name
                    };

                    let i32_type = self.context.i32_type();
                    let i8_type = self.context.i8_type();
                    let candy_type_ptr = self
                        .module
                        .get_struct_type("candy_type")
                        .unwrap()
                        .ptr_type(AddressSpace::default());

                    let text = format!("{responsible_parameter}");

                    let global = self.module.add_global(candy_type_ptr, None, &text);
                    let v = self.make_str_literal(&text);
                    let len = i32_type.const_int(text.len() as u64, false);
                    let arr_alloc = self.builder.build_array_alloca(i8_type, len, "");
                    self.builder.build_store(arr_alloc, v);
                    let cast = self.builder.build_bitcast(
                        arr_alloc,
                        i8_type.ptr_type(AddressSpace::default()),
                        "",
                    );
                    let make_candy_text = self.module.get_function("make_candy_text").unwrap();
                    let call = self.builder.build_call(make_candy_text, &[cast.into()], "");

                    self.builder.build_store(
                        global.as_pointer_value(),
                        call.try_as_basic_value().unwrap_left(),
                    );

                    global.set_initializer(&candy_type_ptr.const_null());
                    self.values.insert(*responsible_parameter, Rc::new(global));

                    let candy_type_ptr = self
                        .module
                        .get_struct_type("candy_type")
                        .unwrap()
                        .ptr_type(AddressSpace::default());

                    let captured_ids: Vec<_> = expr
                        .captured_ids()
                        .into_iter()
                        .filter(|cap_id| {
                            !(self.values.contains_key(cap_id)
                                || self.unrepresented_ids.contains(cap_id))
                        })
                        .collect();

                    let env_types: Vec<_> = captured_ids
                        .iter()
                        .map(|_| candy_type_ptr.as_basic_type_enum())
                        .collect();

                    let env_struct_type = self.context.struct_type(&env_types, false);

                    let env_ptr = self.builder.build_alloca(env_struct_type, "");

                    let env_struct = self.builder.build_load(env_struct_type, env_ptr, "");
                    for (idx, cap_id) in captured_ids.iter().enumerate() {
                        let mut value = self.locals.get(cap_id).map(|v| **v);

                        if value.is_none() {
                            value.replace(
                                (self
                                    .values
                                    .get(cap_id)
                                    .unwrap_or_else(|| panic!("{cap_id} is not a global")))
                                .as_basic_value_enum(),
                            );
                        }

                        self.builder.build_insert_value(
                            env_struct.into_struct_value(),
                            value.unwrap(),
                            idx as u32,
                            "",
                        );
                    }

                    let mut params: Vec<_> =
                        parameters.iter().map(|_| candy_type_ptr.into()).collect();
                    if !captured_ids.is_empty() {
                        params.push(candy_type_ptr.into());
                    }

                    let fn_type = candy_type_ptr.fn_type(&params, false);

                    let function = self.module.add_function(name, fn_type, None);

                    let fun_info = FunctionInfo {
                        function_value: Rc::new(function),
                        parameters: parameters.clone(),
                        captured_ids: captured_ids.clone(),

                        env_type: Some(env_struct_type),
                    };
                    self.functions.insert(*id, fun_info.clone());

                    for (id, param) in parameters.iter().zip(function.get_params()) {
                        self.locals.insert(*id, Rc::new(param));
                    }

                    let current_block = self.builder.get_insert_block().unwrap();

                    let function_ptr = function.as_global_value().as_pointer_value();
                    let make_candy_function =
                        self.module.get_function("make_candy_function").unwrap();
                    let call = self.builder.build_call(
                        make_candy_function,
                        &[function_ptr.into(), env_ptr.into()],
                        "",
                    );

                    let global = self.module.add_global(candy_type_ptr, None, "");
                    global.set_initializer(&function_ptr.get_type().const_null());

                    self.builder.build_store(
                        global.as_pointer_value(),
                        call.try_as_basic_value().unwrap_left(),
                    );

                    self.values.insert(*id, Rc::new(global));

                    let inner_block = self.context.append_basic_block(function, name);
                    self.builder.position_at_end(inner_block);

                    self.compile_mir(body, &fun_info);
                    self.builder.position_at_end(current_block);
                }
                candy_frontend::mir::Expression::Parameter => unreachable!(),
                candy_frontend::mir::Expression::Call {
                    function,
                    arguments,
                    ..
                } => {
                    let FunctionInfo {
                        function_value,
                        parameters,
                        captured_ids: _,
                        env_type: _,
                    } = self
                        .functions
                        .get(function)
                        .unwrap_or_else(|| panic!("Cannot find function with ID {function}"));

                    let candy_type_ptr = self
                        .module
                        .get_struct_type("candy_type")
                        .unwrap()
                        .ptr_type(AddressSpace::default());

                    let args: Vec<_> = arguments
                        .iter()
                        .map(|arg| {
                            let mut v = self.values.get(arg).map(|a| {
                                self.builder
                                    .build_load(candy_type_ptr, a.as_pointer_value(), "")
                            });
                            if v.is_none() {
                                if let Some(i) = parameters.iter().position(|i| i == arg) {
                                    v.replace(function_value.get_nth_param(i as u32).unwrap());
                                }
                            }
                            if v.is_none() {
                                if let Some(i) =
                                    function_ctx.captured_ids.iter().position(|i| i == arg)
                                {
                                    let env_ptr =
                                        function_ctx.function_value.get_last_param().unwrap();

                                    let env_value = self.builder.build_struct_gep(
                                        function_ctx.env_type.unwrap(),
                                        env_ptr.into_pointer_value(),
                                        i as u32,
                                        "",
                                    );

                                    if let Ok(env_value) = env_value {
                                        v.replace(env_value.as_basic_value_enum());
                                    }
                                }
                            }
                            if v.is_none() {
                                if let Some(value) = self.locals.get(arg) {
                                    v.replace(*value.clone());
                                }
                            }
                            v.unwrap_or_else(|| panic!("{arg} should be a real ID"))
                                .into()
                        })
                        .collect();
                    let call = self.builder.build_call(**function_value, &args, "");
                    let call_value = Rc::new(call.try_as_basic_value().unwrap_left());
                    self.locals.insert(*id, call_value.clone());

                    if idx == mir.expressions.len() - 1 {
                        self.builder
                            .build_return(Some(&call_value.into_pointer_value()));
                    }
                }
                candy_frontend::mir::Expression::UseModule { .. } => unreachable!(),
                candy_frontend::mir::Expression::Panic { reason, .. } => {
                    let panic_fn = self.module.get_function("candy_panic").unwrap();

                    let candy_type_ptr = self
                        .module
                        .get_struct_type("candy_type")
                        .unwrap()
                        .ptr_type(AddressSpace::default());

                    if let Some(reason) = self.values.get(reason) {
                        let reason =
                            self.builder
                                .build_load(candy_type_ptr, reason.as_pointer_value(), "");
                        self.builder.build_call(panic_fn, &[reason.into()], "");
                    } else {
                        self.builder.build_call(
                            panic_fn,
                            &[candy_type_ptr.const_null().into()],
                            "",
                        );
                    }

                    self.builder.build_unreachable();
                }
                candy_frontend::mir::Expression::TraceCallStarts { .. } => unimplemented!(),
                candy_frontend::mir::Expression::TraceCallEnds { .. } => unimplemented!(),
                candy_frontend::mir::Expression::TraceExpressionEvaluated { .. } => {
                    unimplemented!()
                }
                candy_frontend::mir::Expression::TraceFoundFuzzableFunction { .. } => {
                    unimplemented!()
                }
            }
        }
    }

    fn make_str_literal(&self, s: &str) -> ArrayValue<'_> {
        let i8_type = self.context.i8_type();
        let content: Vec<_> = s
            .chars()
            .chain(std::iter::once('\0'))
            .map(|c| i8_type.const_int(c as u64, false))
            .collect();
        i8_type.const_array(&content)
    }
}
