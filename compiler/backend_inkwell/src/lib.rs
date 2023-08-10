use candy_frontend::mir::{Body, Id, Mir};
use inkwell::{
    builder::Builder,
    context::Context,
    module::{Linkage, Module},
    support::LLVMString,
    types::{BasicType, StructType},
    values::{ArrayValue, BasicValue, BasicValueEnum, FunctionValue, GlobalValue, PointerValue},
    AddressSpace,
};

pub use inkwell;
use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    rc::Rc,
    sync::Arc,
};

#[derive(Clone)]
struct FunctionInfo<'ctx> {
    function_value: Rc<FunctionValue<'ctx>>,
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
    main_return: Option<PointerValue<'ctx>>,
}

impl<'ctx> CodeGen<'ctx> {
    pub fn new(context: &'ctx Context, module_name: &str, mir: Arc<Mir>) -> Self {
        let module = context.create_module(module_name);
        let builder = context.create_builder();
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
            main_return: None,
        }
    }

    pub fn compile(
        &mut self,
        path: &Path,
        print_llvm_ir: bool,
        print_main_output: bool,
    ) -> Result<(), LLVMString> {
        let i32_type = self.context.i32_type();
        let i64_type = self.context.i64_type();
        let i8_type = self.context.i8_type();
        let void_type = self.context.void_type();

        let candy_value = self.context.opaque_struct_type("candy_value");
        let candy_value_ptr = candy_value.ptr_type(AddressSpace::default());

        let make_int_fn_type = candy_value_ptr.fn_type(&[i64_type.into()], false);
        self.module
            .add_function("make_candy_int", make_int_fn_type, Some(Linkage::External));
        let make_tag_fn_type =
            candy_value_ptr.fn_type(&[i8_type.ptr_type(AddressSpace::default()).into()], false);
        let make_candy_tag =
            self.module
                .add_function("make_candy_tag", make_tag_fn_type, Some(Linkage::External));
        self.module
            .add_function("make_candy_text", make_tag_fn_type, Some(Linkage::External));
        let make_list_fn_type = candy_value_ptr.fn_type(&[candy_value_ptr.into()], false);
        self.module.add_function(
            "make_candy_list",
            make_list_fn_type,
            Some(Linkage::External),
        );
        let make_function_fn_type = candy_value_ptr.fn_type(
            &[
                candy_value_ptr.into(),
                candy_value_ptr.into(),
                i64_type.into(),
            ],
            false,
        );
        self.module.add_function(
            "make_candy_function",
            make_function_fn_type,
            Some(Linkage::External),
        );

        let make_struct_fn_type =
            candy_value_ptr.fn_type(&[candy_value_ptr.into(), candy_value_ptr.into()], false);
        self.module.add_function(
            "make_candy_struct",
            make_struct_fn_type,
            Some(Linkage::External),
        );

        let struct_get_fn_type =
            candy_value_ptr.fn_type(&[candy_value_ptr.into(), candy_value_ptr.into()], false);
        let candy_builtin_struct_get =
            self.module
                .add_function("candy_builtin_struct_get", struct_get_fn_type, None);

        let panic_fn_type = void_type.fn_type(
            &[candy_value.ptr_type(AddressSpace::default()).into()],
            false,
        );
        self.module.add_function("candy_panic", panic_fn_type, None);
        let free_fn = self
            .module
            .add_function("free_candy_value", panic_fn_type, None);
        let print_fn = self
            .module
            .add_function("print_candy_value", panic_fn_type, None);

        let candy_fn_type = candy_value_ptr.fn_type(&[], true);
        let get_candy_fn_ptr_type = candy_fn_type
            .ptr_type(AddressSpace::default())
            .fn_type(&[candy_value_ptr.into()], false);
        self.module
            .add_function("get_candy_function_pointer", get_candy_fn_ptr_type, None);
        let get_candy_fn_env_type = candy_value_ptr.fn_type(&[candy_value_ptr.into()], false);
        self.module.add_function(
            "get_candy_function_environment",
            get_candy_fn_env_type,
            None,
        );

        let main_type = i32_type.fn_type(&[], false);
        let main_fn = self.module.add_function("main", main_type, None);
        let block = self.context.append_basic_block(main_fn, "entry");

        let call_candy_function_type =
            candy_value_ptr.fn_type(&[candy_value_ptr.into(), candy_value_ptr.into()], false);
        let call_candy_function_with =
            self.module
                .add_function("call_candy_function_with", call_candy_function_type, None);

        let main_info = FunctionInfo {
            function_value: Rc::new(main_fn),
            captured_ids: vec![],
            env_type: None,
        };

        self.builder.position_at_end(block);
        self.compile_mir(&self.mir.body.clone(), &main_info);
        self.builder.position_at_end(block);

        let environment = self
            .module
            .add_global(candy_value_ptr, None, "candy_environment");

        if let Some(main_return) = self.main_return {
            const MAIN_FN_NAME: &str = "Main";
            let len = i32_type.const_int(MAIN_FN_NAME.len() as u64, false);
            let main_text = self.builder.build_array_alloca(i8_type, len, "");
            let main_text_array = self.make_str_literal(MAIN_FN_NAME);
            self.builder.build_store(main_text, main_text_array);

            let main_tag = self
                .builder
                .build_call(make_candy_tag, &[main_text.into()], "");

            let main_fn = self
                .builder
                .build_call(
                    candy_builtin_struct_get,
                    &[
                        main_return.into(),
                        main_tag.try_as_basic_value().unwrap_left().into(),
                    ],
                    "",
                )
                .try_as_basic_value()
                .unwrap_left();

            let main_res_ptr = self.builder.build_call(
                call_candy_function_with,
                &[main_fn.into(), environment.as_basic_value_enum().into()],
                "",
            );

            if print_main_output {
                self.builder.build_call(
                    print_fn,
                    &[main_res_ptr.try_as_basic_value().unwrap_left().into()],
                    "",
                );
                for value in self.module.get_globals() {
                    let val =
                        self.builder
                            .build_load(candy_value_ptr, value.as_pointer_value(), "");
                    self.builder.build_call(free_fn, &[val.into()], "");
                }
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

    pub fn compile_asm_and_link(
        &self,
        path: &str,
        build_rt: bool,
        debug: bool,
    ) -> Result<(), std::io::Error> {
        let bc_path = PathBuf::from(&format!("{path}.bc"));
        std::process::Command::new("llc")
            .arg(&bc_path)
            .args(["-O3"])
            .spawn()?
            .wait()?;
        if build_rt {
            std::process::Command::new("make")
                .args(["-C", "compiler/backend_inkwell/candy_rt/", "clean"])
                .spawn()?
                .wait()?;

            std::process::Command::new("make")
                .args(["-C", "compiler/backend_inkwell/candy_rt/", "candy_rt.a"])
                .spawn()?
                .wait()?;
        }
        let s_path = PathBuf::from(format!("{path}.s"));
        std::process::Command::new("clang")
            .args([
                s_path.to_str().unwrap(),
                "compiler/backend_inkwell/candy_rt/candy_rt.a",
                if debug { "-g" } else { "" },
                "-O3",
                "-flto",
                "-o",
                &s_path.to_str().unwrap().replace(".candy.s", ""),
            ])
            .spawn()?
            .wait()?;
        Ok(())
    }

    fn compile_mir(&mut self, mir: &Body, function_ctx: &FunctionInfo<'ctx>) {
        let candy_value_ptr = self
            .module
            .get_struct_type("candy_value")
            .unwrap()
            .ptr_type(AddressSpace::default());

        for (idx, (id, expr)) in mir.expressions.iter().enumerate() {
            match expr {
                candy_frontend::mir::Expression::Int(value) => {
                    let i64_type = self.context.i64_type();
                    let v = i64_type.const_int(value.try_into().unwrap(), false);

                    let global =
                        self.module
                            .add_global(candy_value_ptr, None, &format!("num_{value}"));
                    global.set_initializer(&candy_value_ptr.const_null());
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

                    let global = self.module.add_global(candy_value_ptr, None, text);
                    let v = self.make_str_literal(text);
                    let len = i32_type.const_int(text.len() as u64 + 1, false);
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

                    global.set_initializer(&candy_value_ptr.const_null());
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

                    let global = self.module.add_global(candy_value_ptr, None, symbol);
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

                    global.set_initializer(&candy_value_ptr.const_null());
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
                            let fn_type = candy_value_ptr
                                .fn_type(&[candy_value_ptr.into(), candy_value_ptr.into()], false);
                            let function =
                                self.module
                                    .add_function("candy_builtin_equals", fn_type, None);
                            self.functions.insert(
                                *id,
                                FunctionInfo {
                                    function_value: Rc::new(function),
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
                            let fn_type = candy_value_ptr.fn_type(
                                &[
                                    candy_value_ptr.into(),
                                    candy_value_ptr.into(),
                                    candy_value_ptr.into(),
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
                                    captured_ids: vec![],
                                    env_type: None,
                                },
                            );
                        }
                        candy_frontend::builtin_functions::BuiltinFunction::IntAdd => {
                            let fn_type = candy_value_ptr
                                .fn_type(&[candy_value_ptr.into(), candy_value_ptr.into()], false);
                            let function =
                                self.module
                                    .add_function("candy_builtin_int_add", fn_type, None);
                            self.functions.insert(
                                *id,
                                FunctionInfo {
                                    function_value: Rc::new(function),
                                    captured_ids: vec![],
                                    env_type: None,
                                },
                            );
                        }
                        candy_frontend::builtin_functions::BuiltinFunction::IntBitLength => {
                            let fn_type = candy_value_ptr.fn_type(&[candy_value_ptr.into()], false);
                            let function = self.module.add_function(
                                "candy_builtin_int_bit_length",
                                fn_type,
                                None,
                            );
                            self.functions.insert(
                                *id,
                                FunctionInfo {
                                    function_value: Rc::new(function),
                                    captured_ids: vec![],
                                    env_type: None,
                                },
                            );
                        }
                        candy_frontend::builtin_functions::BuiltinFunction::IntBitwiseAnd => {
                            let fn_type = candy_value_ptr
                                .fn_type(&[candy_value_ptr.into(), candy_value_ptr.into()], false);
                            let function = self.module.add_function(
                                "candy_builtin_int_bitwise_and",
                                fn_type,
                                None,
                            );
                            self.functions.insert(
                                *id,
                                FunctionInfo {
                                    function_value: Rc::new(function),
                                    captured_ids: vec![],
                                    env_type: None,
                                },
                            );
                        }
                        candy_frontend::builtin_functions::BuiltinFunction::IntBitwiseOr => {
                            let fn_type = candy_value_ptr
                                .fn_type(&[candy_value_ptr.into(), candy_value_ptr.into()], false);
                            let function = self.module.add_function(
                                "candy_builtin_int_bitwise_or",
                                fn_type,
                                None,
                            );
                            self.functions.insert(
                                *id,
                                FunctionInfo {
                                    function_value: Rc::new(function),
                                    captured_ids: vec![],
                                    env_type: None,
                                },
                            );
                        }
                        candy_frontend::builtin_functions::BuiltinFunction::IntBitwiseXor => {
                            let fn_type = candy_value_ptr
                                .fn_type(&[candy_value_ptr.into(), candy_value_ptr.into()], false);
                            let function = self.module.add_function(
                                "candy_builtin_int_bitwise_xor",
                                fn_type,
                                None,
                            );
                            self.functions.insert(
                                *id,
                                FunctionInfo {
                                    function_value: Rc::new(function),
                                    captured_ids: vec![],
                                    env_type: None,
                                },
                            );
                        }
                        candy_frontend::builtin_functions::BuiltinFunction::IntCompareTo => {
                            let fn_type = candy_value_ptr
                                .fn_type(&[candy_value_ptr.into(), candy_value_ptr.into()], false);
                            let function = self.module.add_function(
                                "candy_builtin_int_compareto",
                                fn_type,
                                None,
                            );
                            self.functions.insert(
                                *id,
                                FunctionInfo {
                                    function_value: Rc::new(function),
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
                            let fn_type = candy_value_ptr
                                .fn_type(&[candy_value_ptr.into(), candy_value_ptr.into()], false);
                            let function = self.module.add_function(
                                "candy_builtin_int_subtract",
                                fn_type,
                                None,
                            );
                            self.functions.insert(
                                *id,
                                FunctionInfo {
                                    function_value: Rc::new(function),
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
                        candy_frontend::builtin_functions::BuiltinFunction::Print => {
                            let fn_type = candy_value_ptr.fn_type(&[candy_value_ptr.into()], false);
                            let function =
                                self.module
                                    .add_function("candy_builtin_print", fn_type, None);
                            self.functions.insert(
                                *id,
                                FunctionInfo {
                                    function_value: Rc::new(function),
                                    captured_ids: vec![],
                                    env_type: None,
                                },
                            );
                        }
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
                            let fn_type = candy_value_ptr.fn_type(&[candy_value_ptr.into()], false);
                            let function =
                                self.module
                                    .add_function("candy_builtin_typeof", fn_type, None);
                            self.functions.insert(
                                *id,
                                FunctionInfo {
                                    function_value: Rc::new(function),
                                    captured_ids: vec![],
                                    env_type: None,
                                },
                            );
                        }
                    }
                    self.unrepresented_ids.insert(*id);
                }
                candy_frontend::mir::Expression::List(list) => {
                    let i32_type = self.context.i32_type();

                    let list_array = self.builder.build_array_alloca(
                        candy_value_ptr,
                        i32_type.const_int(list.len() as u64 + 1, false),
                        "",
                    );
                    let values = list.iter().map(|v| {
                        self.get_value_with_id(function_ctx, v)
                            .unwrap()
                            .into_pointer_value()
                    });
                    for (idx, value) in values.enumerate() {
                        let value_position = unsafe {
                            self.builder.build_gep(
                                candy_value_ptr,
                                list_array,
                                &[i32_type.const_int(idx as u64, false)],
                                "",
                            )
                        };
                        self.builder.build_store(value_position, value);
                    }
                    let end_position = unsafe {
                        self.builder.build_gep(
                            candy_value_ptr,
                            list_array,
                            &[i32_type.const_int(list.len() as u64, false)],
                            "",
                        )
                    };
                    self.builder
                        .build_store(end_position, candy_value_ptr.const_null());

                    let global = self.module.add_global(candy_value_ptr, None, "");
                    global.set_initializer(&candy_value_ptr.const_null());

                    let make_candy_list = self.module.get_function("make_candy_list").unwrap();
                    let candy_list =
                        self.builder
                            .build_call(make_candy_list, &[list_array.into()], "");
                    self.builder.build_store(
                        global.as_pointer_value(),
                        candy_list.try_as_basic_value().unwrap_left(),
                    );
                    self.values.insert(*id, Rc::new(global));
                }
                candy_frontend::mir::Expression::Struct(s) => {
                    let i32_type = self.context.i32_type();
                    let make_candy_struct = self.module.get_function("make_candy_struct").unwrap();

                    let keys_array = self.builder.build_array_alloca(
                        candy_value_ptr,
                        i32_type.const_int(s.len() as u64 + 1, false),
                        "",
                    );
                    let values_array = self.builder.build_array_alloca(
                        candy_value_ptr,
                        i32_type.const_int(s.len() as u64 + 1, false),
                        "",
                    );
                    for (idx, (key, value)) in s.iter().enumerate() {
                        let key = self
                            .get_value_with_id(function_ctx, key)
                            .unwrap()
                            .into_pointer_value();
                        let value = self
                            .get_value_with_id(function_ctx, value)
                            .unwrap()
                            .into_pointer_value();

                        let key_ptr = unsafe {
                            self.builder.build_gep(
                                candy_value_ptr,
                                keys_array,
                                &[i32_type.const_int(idx as u64, false)],
                                "",
                            )
                        };
                        self.builder.build_store(key_ptr, key);
                        let value_ptr = unsafe {
                            self.builder.build_gep(
                                candy_value_ptr,
                                values_array,
                                &[i32_type.const_int(idx as u64, false)],
                                "",
                            )
                        };
                        self.builder.build_store(value_ptr, value);
                    }

                    // Null-terminate key/value arrays
                    let key_ptr = unsafe {
                        self.builder.build_gep(
                            candy_value_ptr,
                            keys_array,
                            &[i32_type.const_int(s.len() as u64, false)],
                            "",
                        )
                    };
                    self.builder
                        .build_store(key_ptr, candy_value_ptr.const_null());
                    let value_ptr = unsafe {
                        self.builder.build_gep(
                            candy_value_ptr,
                            values_array,
                            &[i32_type.const_int(s.len() as u64, false)],
                            "",
                        )
                    };
                    self.builder
                        .build_store(value_ptr, candy_value_ptr.const_null());

                    let struct_value = self
                        .builder
                        .build_call(
                            make_candy_struct,
                            &[keys_array.into(), values_array.into()],
                            "",
                        )
                        .try_as_basic_value()
                        .unwrap_left();

                    self.locals.insert(*id, Rc::new(struct_value));

                    let function_ctx_name =
                        function_ctx.function_value.get_name().to_str().unwrap();
                    if idx == mir.expressions.len() - 1 {
                        if function_ctx_name != "main" {
                            self.builder
                                .build_return(Some(&struct_value.into_pointer_value()));
                        } else {
                            self.main_return.replace(struct_value.into_pointer_value());
                        }
                    }
                }
                candy_frontend::mir::Expression::Reference(ref_id) => {
                    let value = self.get_value_with_id(function_ctx, ref_id).unwrap();

                    self.locals.insert(*id, Rc::new(value));
                    if idx == mir.expressions.len() - 1 {
                        self.builder.build_return(Some(&value));
                    }
                }
                candy_frontend::mir::Expression::HirId(hir_id) => {
                    let i32_type = self.context.i32_type();
                    let i8_type = self.context.i8_type();
                    let candy_value_ptr = self
                        .module
                        .get_struct_type("candy_value")
                        .unwrap()
                        .ptr_type(AddressSpace::default());

                    let text = format!("{hir_id}");

                    let global = self.module.add_global(candy_value_ptr, None, &text);
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

                    global.set_initializer(&candy_value_ptr.const_null());
                    self.values.insert(*id, Rc::new(global));
                }
                candy_frontend::mir::Expression::Function {
                    original_hirs,
                    parameters,
                    body,
                    responsible_parameter,
                } => {
                    let name = format!("{original_hirs:?}")
                        .replace('{', "")
                        .replace('}', "")
                        .replace([':', '.'], "_");

                    let i32_type = self.context.i32_type();
                    let i8_type = self.context.i8_type();
                    let candy_value_ptr = self
                        .module
                        .get_struct_type("candy_value")
                        .unwrap()
                        .ptr_type(AddressSpace::default());

                    let text = format!("{responsible_parameter}");

                    let global = self.module.add_global(candy_value_ptr, None, &text);
                    let v = self.make_str_literal(&text);
                    let len = i32_type.const_int(text.len() as u64 + 1, false);
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

                    global.set_initializer(&candy_value_ptr.const_null());
                    self.values.insert(*responsible_parameter, Rc::new(global));

                    let candy_value_ptr = self
                        .module
                        .get_struct_type("candy_value")
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
                        .map(|_| candy_value_ptr.as_basic_type_enum())
                        .collect();

                    let env_struct_type = self.context.struct_type(&env_types, false);

                    let env_ptr = self.builder.build_malloc(env_struct_type, "").unwrap();

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

                        let member = self
                            .builder
                            .build_struct_gep(env_struct_type, env_ptr, idx as u32, "")
                            .unwrap();
                        self.builder.build_store(member, value.unwrap());
                    }

                    let mut params: Vec<_> =
                        parameters.iter().map(|_| candy_value_ptr.into()).collect();
                    if !captured_ids.is_empty() {
                        params.push(candy_value_ptr.into());
                    }

                    let fn_type = candy_value_ptr.fn_type(&params, false);

                    let function = self.module.add_function(&name, fn_type, None);

                    let fun_info = FunctionInfo {
                        function_value: Rc::new(function),
                        captured_ids: captured_ids.clone(),
                        env_type: if !captured_ids.is_empty() {
                            Some(env_struct_type)
                        } else {
                            None
                        },
                    };
                    self.functions.insert(*id, fun_info.clone());

                    for (id, param) in parameters.iter().zip(function.get_params()) {
                        self.locals.insert(*id, Rc::new(param));
                    }

                    let current_block = self.builder.get_insert_block().unwrap();

                    let env_size = env_struct_type.size_of().unwrap();
                    let function_ptr = function.as_global_value().as_pointer_value();
                    let make_candy_function =
                        self.module.get_function("make_candy_function").unwrap();
                    let call = self.builder.build_call(
                        make_candy_function,
                        &[function_ptr.into(), env_ptr.into(), env_size.into()],
                        "",
                    );

                    let global =
                        self.module
                            .add_global(candy_value_ptr, None, &format!("fun_{name}"));
                    global.set_initializer(&function_ptr.get_type().const_null());

                    self.builder.build_store(
                        global.as_pointer_value(),
                        call.try_as_basic_value().unwrap_left(),
                    );

                    self.values.insert(*id, Rc::new(global));

                    let inner_block = self.context.append_basic_block(function, &name);
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
                    let mut args: Vec<_> = arguments
                        .iter()
                        .map(|arg| self.get_value_with_id(function_ctx, arg).unwrap().into())
                        .collect();
                    if let Some(FunctionInfo {
                        function_value,
                        captured_ids: _,
                        env_type,
                    }) = self.functions.get(function)
                    {
                        if env_type.is_some() {
                            let get_candy_fn_env = self
                                .module
                                .get_function("get_candy_function_environment")
                                .unwrap();

                            let fn_object = self.values.get(function).unwrap_or_else(|| {
                                panic!("Function {function} should have global visibility")
                            });

                            let fn_env_ptr = self.builder.build_call(
                                get_candy_fn_env,
                                &[fn_object.as_pointer_value().into()],
                                "",
                            );

                            args.push(fn_env_ptr.try_as_basic_value().unwrap_left().into());
                        }
                        let call = self.builder.build_call(**function_value, &args, "");
                        let call_value = Rc::new(call.try_as_basic_value().unwrap_left());
                        self.locals.insert(*id, call_value.clone());

                        if idx == mir.expressions.len() - 1 {
                            self.builder
                                .build_return(Some(&call_value.into_pointer_value()));
                        }
                    } else {
                        let function_value = self
                            .get_value_with_id(function_ctx, function)
                            .unwrap_or_else(|| panic!("There is no function with ID {function}"));

                        let get_candy_fn_ptr = self
                            .module
                            .get_function("get_candy_function_pointer")
                            .unwrap();
                        let get_candy_fn_env = self
                            .module
                            .get_function("get_candy_function_environment")
                            .unwrap();

                        let fn_ptr =
                            self.builder
                                .build_call(get_candy_fn_ptr, &[function_value.into()], "");

                        let fn_env_ptr =
                            self.builder
                                .build_call(get_candy_fn_env, &[function_value.into()], "");

                        args.push(fn_env_ptr.try_as_basic_value().unwrap_left().into());

                        let candy_fn_type = candy_value_ptr.fn_type(&[], true);
                        let inner_fn = fn_ptr
                            .try_as_basic_value()
                            .unwrap_left()
                            .into_pointer_value();

                        let call =
                            self.builder
                                .build_indirect_call(candy_fn_type, inner_fn, &args, "");

                        let call_value = Rc::new(call.try_as_basic_value().unwrap_left());
                        self.locals.insert(*id, call_value.clone());

                        if idx == mir.expressions.len() - 1 {
                            self.builder
                                .build_return(Some(&call_value.into_pointer_value()));
                        }
                    }
                }
                candy_frontend::mir::Expression::UseModule { .. } => unreachable!(),
                candy_frontend::mir::Expression::Panic { reason, .. } => {
                    let panic_fn = self.module.get_function("candy_panic").unwrap();

                    let reason = self.get_value_with_id(function_ctx, reason).unwrap();

                    self.builder.build_call(panic_fn, &[reason.into()], "");

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

    fn get_value_with_id(
        &self,
        function_ctx: &FunctionInfo<'ctx>,
        id: &Id,
    ) -> Option<BasicValueEnum<'ctx>> {
        let candy_value_ptr = self
            .module
            .get_struct_type("candy_value")
            .unwrap()
            .ptr_type(AddressSpace::default());
        let mut v = self.values.get(id).map(|a| {
            self.builder
                .build_load(candy_value_ptr, a.as_pointer_value(), "")
        });
        if v.is_none() {
            if let Some(i) = function_ctx.captured_ids.iter().position(|i| i == id) {
                let env_ptr = function_ctx.function_value.get_last_param().unwrap();

                let env_value = self
                    .builder
                    .build_struct_gep(
                        function_ctx.env_type.unwrap(),
                        env_ptr.into_pointer_value(),
                        i as u32,
                        "",
                    )
                    .unwrap();

                v.replace(self.builder.build_load(candy_value_ptr, env_value, ""));
            }
        }
        if v.is_none() {
            if let Some(value) = self.locals.get(id) {
                v.replace(*value.clone());
            }
        }
        v.unwrap_or_else(|| panic!("{id} should be a real ID"))
            .into()
    }
}
