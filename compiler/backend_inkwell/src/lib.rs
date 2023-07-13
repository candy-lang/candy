use candy_frontend::mir::{Body, Id, Mir};
use inkwell::{
    builder::Builder,
    context::Context,
    module::{Linkage, Module},
    support::LLVMString,
    values::{
        ArrayValue, BasicMetadataValueEnum, BasicValue, BasicValueEnum, FunctionValue, GlobalValue,
    },
    AddressSpace,
};

pub use inkwell;
use std::{collections::HashMap, path::Path, rc::Rc, sync::Arc};

pub struct CodeGen<'ctx> {
    context: &'ctx Context,
    module: Module<'ctx>,
    builder: Builder<'ctx>,
    mir: Arc<Mir>,
    tags: HashMap<String, Option<Id>>,
    values: HashMap<Id, Rc<GlobalValue<'ctx>>>,
    locals: HashMap<Id, Rc<BasicValueEnum<'ctx>>>,
    functions: HashMap<Id, (Rc<FunctionValue<'ctx>>, Vec<Id>)>,
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
        }
    }

    pub fn compile(mut self, path: &Path, print_llvm_ir: bool) -> Result<(), LLVMString> {
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
        //let candy_fn_type = candy_type_ptr.fn_type(&[], false);
        let make_function_fn_type = candy_type_ptr.fn_type(&[candy_type_ptr.into()], false);
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

        let main_type = i32_type.fn_type(&[], false);
        let main_fn = self.module.add_function("main", main_type, None);
        let block = self.context.append_basic_block(main_fn, "entry");
        self.builder.position_at_end(block);
        self.compile_mir(&self.mir.body.clone());
        self.builder.position_at_end(block);
        let environment = self
            .module
            .add_global(candy_type_ptr, None, "candy_environment");
        self.builder.build_call(
            self.module.get_function("candy_main").unwrap(),
            &[environment.as_basic_value_enum().into()],
            "",
        );
        let ret_value = i32_type.const_int(0, false);
        self.builder.build_return(Some(&ret_value));
        if print_llvm_ir {
            self.module.print_to_stderr();
        }
        self.module.verify()?;
        self.module.write_bitcode_to_path(path);
        Ok(())
    }

    pub fn compile_mir(&mut self, mir: &Body) {
        for (idx, (id, expr)) in mir.expressions.iter().enumerate() {
            //dbg!(expr);
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

                    //let tag = i32_type.const_int(self.tags.len().try_into().unwrap(), false);
                    global.set_initializer(&candy_type_ptr.const_null());
                    self.values.insert(*id, Rc::new(global));
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

                    //let tag = i32_type.const_int(self.tags.len().try_into().unwrap(), false);
                    global.set_initializer(&candy_type_ptr.const_null());
                    self.values.insert(*id, Rc::new(global));
                }
                candy_frontend::mir::Expression::Builtin(builtin) => match builtin {
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
                        self.functions.insert(*id, (Rc::new(function), vec![]));
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
                        self.functions.insert(*id, (Rc::new(function), vec![]));
                    }
                    candy_frontend::builtin_functions::BuiltinFunction::IntAdd => todo!(),
                    candy_frontend::builtin_functions::BuiltinFunction::IntBitLength => todo!(),
                    candy_frontend::builtin_functions::BuiltinFunction::IntBitwiseAnd => {
                        todo!()
                    }
                    candy_frontend::builtin_functions::BuiltinFunction::IntBitwiseOr => todo!(),
                    candy_frontend::builtin_functions::BuiltinFunction::IntBitwiseXor => {
                        todo!()
                    }
                    candy_frontend::builtin_functions::BuiltinFunction::IntCompareTo => todo!(),
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
                    candy_frontend::builtin_functions::BuiltinFunction::IntSubtract => todo!(),
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
                        self.functions.insert(*id, (Rc::new(function), vec![]));
                    }
                },
                candy_frontend::mir::Expression::List(_) => todo!(),
                candy_frontend::mir::Expression::Struct(_s) => {
                    // Not yet implemented, but not allowed to panic
                    self.context.struct_type(&[], false);
                }
                candy_frontend::mir::Expression::Reference(id) => {
                    if let Some(v) = self.values.get(id) {
                        self.builder.build_return(Some(&v.as_pointer_value()));
                    }
                }
                candy_frontend::mir::Expression::HirId(_id) => {
                    // Intentionally ignored
                }
                candy_frontend::mir::Expression::Function {
                    original_hirs,
                    parameters,
                    body,
                    ..
                } => {
                    let original_name = &original_hirs.iter().next().unwrap().keys[0].to_string();
                    let name = match original_name.as_str() {
                        "main" => "candy_main",
                        other => other,
                    };

                    let candy_type_ptr = self
                        .module
                        .get_struct_type("candy_type")
                        .unwrap()
                        .ptr_type(AddressSpace::default());

                    let params: Vec<_> = parameters.iter().map(|_| candy_type_ptr.into()).collect();

                    let fn_type = candy_type_ptr.fn_type(&params, false);

                    let function = self.module.add_function(name, fn_type, None);
                    self.functions
                        .insert(*id, (Rc::new(function), parameters.clone()));

                    let function_ptr = function.as_global_value().as_pointer_value();
                    let make_candy_function =
                        self.module.get_function("make_candy_function").unwrap();
                    let call =
                        self.builder
                            .build_call(make_candy_function, &[function_ptr.into()], "");

                    let global = self.module.add_global(candy_type_ptr, None, "");
                    global.set_initializer(&function_ptr.get_type().const_null());
                    self.builder.build_store(
                        global.as_pointer_value(),
                        call.try_as_basic_value().unwrap_left(),
                    );

                    self.values.insert(*id, Rc::new(global));

                    let inner_block = self.context.append_basic_block(function, name);
                    self.builder.position_at_end(inner_block);
                    self.compile_mir(body);
                }
                candy_frontend::mir::Expression::Parameter => todo!(),
                candy_frontend::mir::Expression::Call {
                    function,
                    arguments,
                    ..
                } => {
                    let (fun, _params) = self.functions.get(function).unwrap();

                    let candy_type_ptr = self
                        .module
                        .get_struct_type("candy_type")
                        .unwrap()
                        .ptr_type(AddressSpace::default());

                    let args: Vec<_> = arguments
                        .iter()
                        .map(|arg| {
                            let v = match self.values.get(arg) {
                                Some(value) => Some(value.as_pointer_value()),
                                None => match self.locals.get(arg) {
                                    Some(value) => Some(value.into_pointer_value()),
                                    None => match self
                                        .functions
                                        .values()
                                        .find(|(_, args)| args.contains(arg))
                                    {
                                        Some((fun, args)) => {
                                            let idx = args.iter().position(|i| i == arg).unwrap();
                                            Some(
                                                fun.get_nth_param(idx as u32)
                                                    .unwrap()
                                                    .into_pointer_value(),
                                            )
                                        }
                                        None => Some(candy_type_ptr.const_null()),
                                    },
                                },
                            };
                            self.builder
                                .build_load(candy_type_ptr, v.unwrap(), "")
                                .into()
                            //v.unwrap_or_else(|| panic!("{arg} should be a real ID"))
                        })
                        .collect();
                    let call = self.builder.build_call(**fun, &args, "");
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
                    if let Some(reason) = self.values.get(reason) {
                        self.builder
                            .build_call(panic_fn, &[reason.as_pointer_value().into()], "");
                    } else {
                        let candy_type_ptr = self
                            .module
                            .get_struct_type("candy_type")
                            .unwrap()
                            .ptr_type(AddressSpace::default());

                        self.builder.build_call(
                            panic_fn,
                            &[candy_type_ptr.const_null().into()],
                            "",
                        );
                    }
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

    fn _get_value_by_id(&self, id: &Id) -> Option<BasicMetadataValueEnum> {
        match self.values.get(id) {
            Some(value) => Some(value.as_pointer_value().into()),
            None => match self.locals.get(id) {
                Some(value) => Some(value.into_pointer_value().into()),
                None => match self.functions.values().find(|(_, args)| args.contains(id)) {
                    Some((fun, args)) => {
                        let idx = args.iter().position(|i| i == id).unwrap();
                        Some(fun.get_nth_param(idx as u32).unwrap().into())
                    }
                    None => None,
                },
            },
        }
    }
}
