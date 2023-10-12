#![feature(let_chains)]
#![warn(unused_crate_dependencies)]

use candy_frontend::{
    builtin_functions::BuiltinFunction,
    hir_to_mir::ExecutionTarget,
    mir::{Body, Expression, Id, Mir},
    mir_optimize::OptimizeMir,
    rich_ir::{RichIr, ToRichIr},
    string_to_rcst::ModuleError,
    utils::HashMapExtension,
    TracingConfig,
};
pub use inkwell;
use inkwell::{
    builder::Builder,
    context::Context,
    module::Module,
    support::LLVMString,
    types::{
        BasicMetadataTypeEnum, BasicType, FunctionType, IntType, PointerType, StructType, VoidType,
    },
    values::{BasicValue, BasicValueEnum, FunctionValue, GlobalValue},
    AddressSpace,
};
use itertools::Itertools;
// We depend on this package (used by inkwell) to specify a version and configure features.
use llvm_sys as _;
use rustc_hash::FxHashMap;
use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    sync::Arc,
};

#[salsa::query_group(LlvmIrStorage)]
pub trait LlvmIrDb: OptimizeMir {
    #[salsa::transparent]
    fn llvm_ir(&self, target: ExecutionTarget) -> Result<RichIr, ModuleError>;
}

#[allow(clippy::needless_pass_by_value)]
fn llvm_ir(db: &dyn LlvmIrDb, target: ExecutionTarget) -> Result<RichIr, ModuleError> {
    let (mir, _, _) = db.optimized_mir(target, TracingConfig::off())?;

    let context = Context::create();
    let mut codegen = CodeGen::new(&context, "module", mir);
    let llvm_ir = codegen.compile("", false, true).unwrap();

    Ok(llvm_ir.to_str().unwrap().to_rich_ir(true))
}

#[derive(Clone)]
struct FunctionInfo<'ctx> {
    function_value: FunctionValue<'ctx>,
    captured_ids: Vec<Id>,
    env_type: Option<StructType<'ctx>>,
}

pub struct CodeGen<'ctx> {
    context: &'ctx Context,
    module: Module<'ctx>,
    builder: Builder<'ctx>,
    mir: Arc<Mir>,
    candy_value_pointer_type: PointerType<'ctx>,
    builtins: FxHashMap<BuiltinFunction, FunctionValue<'ctx>>,
    globals: HashMap<Id, GlobalValue<'ctx>>,
    locals: HashMap<Id, BasicValueEnum<'ctx>>,
    functions: HashMap<Id, FunctionInfo<'ctx>>,
    unrepresented_ids: HashSet<Id>,
}

impl<'ctx> CodeGen<'ctx> {
    pub fn new(context: &'ctx Context, module_name: &str, mir: Arc<Mir>) -> Self {
        let module = context.create_module(module_name);
        let builder = context.create_builder();

        let candy_value_type = context.opaque_struct_type("candy_value");
        let candy_value_pointer_type = candy_value_type.ptr_type(AddressSpace::default());

        Self {
            context,
            module,
            builder,
            mir,
            candy_value_pointer_type,
            builtins: FxHashMap::default(),
            globals: HashMap::new(),
            locals: HashMap::new(),
            functions: HashMap::new(),
            unrepresented_ids: HashSet::new(),
        }
    }

    pub fn compile(
        &mut self,
        path: &str,
        print_llvm_ir: bool,
        print_main_output: bool,
    ) -> Result<LLVMString, LLVMString> {
        let void_type = self.context.void_type();
        let i8_type = self.context.i8_type();
        let i32_type = self.context.i32_type();
        let i64_type = self.context.i64_type();

        self.add_function(
            "make_candy_int",
            &[i64_type.into()],
            self.candy_value_pointer_type,
        );
        self.add_function(
            "make_candy_tag",
            &[
                i8_type.ptr_type(AddressSpace::default()).into(),
                self.candy_value_pointer_type.into(),
            ],
            self.candy_value_pointer_type,
        );
        self.add_function(
            "make_candy_text",
            &[i8_type.ptr_type(AddressSpace::default()).into()],
            self.candy_value_pointer_type,
        );
        self.add_function(
            "make_candy_list",
            &[self.candy_value_pointer_type.into()],
            self.candy_value_pointer_type,
        );
        self.add_function(
            "make_candy_function",
            &[
                self.candy_value_pointer_type.into(),
                self.candy_value_pointer_type.into(),
                i64_type.into(),
            ],
            self.candy_value_pointer_type,
        );
        self.add_function(
            "make_candy_struct",
            &[
                self.candy_value_pointer_type.into(),
                self.candy_value_pointer_type.into(),
            ],
            self.candy_value_pointer_type,
        );

        self.add_function(
            "candy_panic",
            &[self.candy_value_pointer_type.into()],
            void_type,
        );
        let free_fn = self.add_function(
            "free_candy_value",
            &[self.candy_value_pointer_type.into()],
            void_type,
        );
        let print_fn = self.add_function(
            "print_candy_value",
            &[self.candy_value_pointer_type.into()],
            void_type,
        );

        let candy_fn_type = self.candy_value_pointer_type.fn_type(&[], true);
        self.add_function(
            "get_candy_function_pointer",
            &[self.candy_value_pointer_type.into()],
            candy_fn_type.ptr_type(AddressSpace::default()),
        );
        self.add_function(
            "get_candy_function_environment",
            &[self.candy_value_pointer_type.into()],
            self.candy_value_pointer_type,
        );

        let main_fn = self.add_function("main", &[], i32_type);
        let block = self.context.append_basic_block(main_fn, "entry");

        let run_candy_main = self.add_function(
            "run_candy_main",
            &[
                self.candy_value_pointer_type.into(),
                self.candy_value_pointer_type.into(),
            ],
            self.candy_value_pointer_type,
        );

        let main_info = FunctionInfo {
            function_value: main_fn,
            captured_ids: vec![],
            env_type: None,
        };

        self.builder.position_at_end(block);
        let main_function = self.compile_mir(&self.mir.body.clone(), &main_info);
        // This is `None` iff there is no exported main function.
        self.builder.position_at_end(block);
        if let Some(main_function) = main_function {
            let environment =
                self.module
                    .add_global(self.candy_value_pointer_type, None, "candy_environment");

            let main_result_ptr = self.builder.build_call(
                run_candy_main,
                &[
                    main_function.as_basic_value_enum().into(),
                    environment.as_basic_value_enum().into(),
                ],
                "",
            );

            if print_main_output {
                self.builder.build_call(
                    print_fn,
                    &[main_result_ptr.try_as_basic_value().unwrap_left().into()],
                    "",
                );
                for value in self.module.get_globals() {
                    if value != environment {
                        let val = self.builder.build_load(
                            self.candy_value_pointer_type,
                            value.as_pointer_value(),
                            "",
                        );
                        self.builder.build_call(free_fn, &[val.into()], "");
                    }
                }
            }

            let ret_value = i32_type.const_int(0, false);
            self.builder.build_return(Some(&ret_value));
        }

        if print_llvm_ir {
            self.module.print_to_stderr();
        }
        self.module.verify()?;
        if !path.is_empty() {
            let bc_path = PathBuf::from(format!("{path}.bc"));
            self.module.write_bitcode_to_path(&bc_path);
        }
        Ok(self.module.print_to_string())
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
                .args(["-C", "compiler/backend_inkwell/candy_runtime/", "clean"])
                .spawn()?
                .wait()?;

            std::process::Command::new("make")
                .args([
                    "-C",
                    "compiler/backend_inkwell/candy_runtime/",
                    "candy_runtime.a",
                ])
                .spawn()?
                .wait()?;
        }
        let s_path = PathBuf::from(format!("{path}.s"));
        std::process::Command::new("clang")
            .args([
                s_path.to_str().unwrap(),
                "compiler/backend_inkwell/candy_runtime/candy_runtime.a",
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

    fn compile_mir(
        &mut self,
        mir: &Body,
        function_ctx: &FunctionInfo<'ctx>,
    ) -> Option<impl BasicValue<'ctx>> {
        let mut return_value = None;
        for (id, expr) in mir.expressions.iter() {
            let expr_value = match expr {
                Expression::Int(value) => {
                    // TODO: Use proper BigInts here
                    let i64_type = self.context.i64_type();
                    let v = i64_type.const_int(
                        value
                            .clamp(&i64::MIN.into(), &i64::MAX.into())
                            .try_into()
                            .unwrap(),
                        false,
                    );

                    let make_candy_int = self.module.get_function("make_candy_int").unwrap();
                    let call = self.builder.build_call(make_candy_int, &[v.into()], "");

                    let global = self.create_global(
                        &format!("num_{value}"),
                        id,
                        call.try_as_basic_value().unwrap_left(),
                    );

                    Some(global.as_basic_value_enum())
                }
                Expression::Text(text) => {
                    let string = self.make_str_literal(text);
                    let make_candy_text = self.module.get_function("make_candy_text").unwrap();
                    let call = self
                        .builder
                        .build_call(make_candy_text, &[string.into()], "");

                    let global = self.create_global(
                        &format!("text_{text}"),
                        id,
                        call.try_as_basic_value().unwrap_left(),
                    );

                    Some(global.as_basic_value_enum())
                }
                Expression::Tag { symbol, value } => {
                    let tag_value = match value {
                        Some(value) => self.get_value_with_id(function_ctx, value).unwrap(),
                        None => self
                            .candy_value_pointer_type
                            .const_null()
                            .as_basic_value_enum(),
                    };

                    let string = self.make_str_literal(symbol);
                    let make_candy_tag = self.module.get_function("make_candy_tag").unwrap();
                    let call = self.builder.build_call(
                        make_candy_tag,
                        &[string.into(), tag_value.into()],
                        "",
                    );

                    let global = self.create_global(
                        &format!("tag_{symbol}"),
                        id,
                        call.try_as_basic_value().unwrap_left(),
                    );

                    Some(global.as_basic_value_enum())
                }
                Expression::Builtin(builtin) => {
                    let function = self.get_builtin(*builtin);
                    self.functions.insert(
                        *id,
                        FunctionInfo {
                            function_value: function,
                            captured_ids: vec![],
                            env_type: None,
                        },
                    );

                    let i64_type = self.context.i64_type();
                    let function_ptr = function.as_global_value().as_pointer_value();
                    let make_candy_function =
                        self.module.get_function("make_candy_function").unwrap();
                    let call = self.builder.build_call(
                        make_candy_function,
                        &[
                            function_ptr.into(),
                            self.candy_value_pointer_type.const_null().into(),
                            i64_type.const_zero().into(),
                        ],
                        "",
                    );

                    let global = self.create_global(
                        &format!("fun_candy_builtin_{}", builtin.as_ref()),
                        id,
                        call.try_as_basic_value().unwrap_left(),
                    );

                    Some(global.as_basic_value_enum())
                }
                Expression::List(list) => {
                    let i64_type = self.context.i64_type();

                    let list_array = self.builder.build_array_alloca(
                        self.candy_value_pointer_type,
                        i64_type.const_int(list.len() as u64 + 1, false),
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
                                self.candy_value_pointer_type,
                                list_array,
                                &[i64_type.const_int(idx as u64, false)],
                                "",
                            )
                        };
                        self.builder.build_store(value_position, value);
                    }
                    let end_position = unsafe {
                        self.builder.build_gep(
                            self.candy_value_pointer_type,
                            list_array,
                            &[i64_type.const_int(list.len() as u64, false)],
                            "",
                        )
                    };
                    self.builder
                        .build_store(end_position, self.candy_value_pointer_type.const_null());

                    let make_candy_list = self.module.get_function("make_candy_list").unwrap();
                    let candy_list =
                        self.builder
                            .build_call(make_candy_list, &[list_array.into()], "");

                    let global =
                        self.create_global("", id, candy_list.try_as_basic_value().unwrap_left());

                    Some(global.as_basic_value_enum())
                }
                Expression::Struct(s) => {
                    let i64_type = self.context.i64_type();
                    let make_candy_struct = self.module.get_function("make_candy_struct").unwrap();

                    let keys_array = self.builder.build_array_alloca(
                        self.candy_value_pointer_type,
                        i64_type.const_int(s.len() as u64 + 1, false),
                        "",
                    );
                    let values_array = self.builder.build_array_alloca(
                        self.candy_value_pointer_type,
                        i64_type.const_int(s.len() as u64 + 1, false),
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
                                self.candy_value_pointer_type,
                                keys_array,
                                &[i64_type.const_int(idx as u64, false)],
                                "",
                            )
                        };
                        self.builder.build_store(key_ptr, key);
                        let value_ptr = unsafe {
                            self.builder.build_gep(
                                self.candy_value_pointer_type,
                                values_array,
                                &[i64_type.const_int(idx as u64, false)],
                                "",
                            )
                        };
                        self.builder.build_store(value_ptr, value);
                    }

                    // Null-terminate key/value arrays
                    let key_ptr = unsafe {
                        self.builder.build_gep(
                            self.candy_value_pointer_type,
                            keys_array,
                            &[i64_type.const_int(s.len() as u64, false)],
                            "",
                        )
                    };
                    self.builder
                        .build_store(key_ptr, self.candy_value_pointer_type.const_null());
                    let value_ptr = unsafe {
                        self.builder.build_gep(
                            self.candy_value_pointer_type,
                            values_array,
                            &[i64_type.const_int(s.len() as u64, false)],
                            "",
                        )
                    };
                    self.builder
                        .build_store(value_ptr, self.candy_value_pointer_type.const_null());

                    let struct_value = self
                        .builder
                        .build_call(
                            make_candy_struct,
                            &[keys_array.into(), values_array.into()],
                            "",
                        )
                        .try_as_basic_value()
                        .unwrap_left();

                    self.locals.insert(*id, struct_value);

                    Some(struct_value.into_pointer_value().as_basic_value_enum())
                }
                Expression::Reference(ref_id) => {
                    let value = self.get_value_with_id(function_ctx, ref_id).unwrap();

                    self.locals.insert(*id, value);
                    Some(value)
                }
                Expression::HirId(hir_id) => {
                    let text = format!("{hir_id}");

                    let string = self.make_str_literal(&text);
                    let make_candy_text = self.module.get_function("make_candy_text").unwrap();
                    let call = self
                        .builder
                        .build_call(make_candy_text, &[string.into()], "");

                    let global =
                        self.create_global(&text, id, call.try_as_basic_value().unwrap_left());

                    Some(global.as_basic_value_enum())
                }
                Expression::Function {
                    original_hirs,
                    parameters,
                    body,
                    responsible_parameter,
                } => {
                    self.unrepresented_ids.insert(*responsible_parameter);
                    let name = original_hirs
                        .iter()
                        .sorted()
                        .map(|it| it.to_string().replace([':', '.'], "_"))
                        .join(", ");

                    let captured_ids: Vec<_> = expr
                        .captured_ids()
                        .into_iter()
                        .filter(|cap_id| {
                            !(self.globals.contains_key(cap_id)
                                || self.unrepresented_ids.contains(cap_id))
                        })
                        .collect();

                    let env_types: Vec<_> = captured_ids
                        .iter()
                        .map(|_| self.candy_value_pointer_type.as_basic_type_enum())
                        .collect();

                    let env_struct_type = self.context.struct_type(&env_types, false);

                    let env_ptr = self.builder.build_malloc(env_struct_type, "").unwrap();

                    for (idx, cap_id) in captured_ids.iter().enumerate() {
                        let value = self.get_value_with_id(function_ctx, cap_id);

                        let member = self
                            .builder
                            .build_struct_gep(env_struct_type, env_ptr, idx as u32, "")
                            .unwrap();
                        self.builder.build_store(member, value.unwrap());
                    }

                    let mut params: Vec<_> = parameters
                        .iter()
                        .map(|_| self.candy_value_pointer_type.into())
                        .collect();

                    if !captured_ids.is_empty() {
                        params.push(self.candy_value_pointer_type.into());
                    }

                    let function = self.add_function(&name, &params, self.candy_value_pointer_type);

                    let function_info = FunctionInfo {
                        function_value: function,
                        captured_ids: captured_ids.clone(),
                        env_type: if !captured_ids.is_empty() {
                            Some(env_struct_type)
                        } else {
                            None
                        },
                    };
                    self.functions.insert(*id, function_info.clone());

                    for (id, param) in parameters.iter().zip(function.get_params()) {
                        self.locals.insert(*id, param);
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

                    let global = self.create_global(
                        &format!("fun_{name}"),
                        id,
                        call.try_as_basic_value().unwrap_left(),
                    );

                    let inner_block = self.context.append_basic_block(function, &name);
                    self.builder.position_at_end(inner_block);

                    self.compile_mir(body, &function_info);
                    self.builder.position_at_end(current_block);

                    Some(global.as_basic_value_enum())
                }
                Expression::Parameter => unreachable!(),
                Expression::Call {
                    function,
                    arguments,
                    responsible,
                } => {
                    self.unrepresented_ids.insert(*responsible);
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

                            let fn_object = self.globals.get(function).unwrap_or_else(|| {
                                panic!("Function {function} should have global visibility")
                            });

                            let fn_env_ptr = self.builder.build_call(
                                get_candy_fn_env,
                                &[fn_object.as_pointer_value().into()],
                                "",
                            );

                            args.push(fn_env_ptr.try_as_basic_value().unwrap_left().into());
                        }
                        let call = self.builder.build_call(*function_value, &args, "");
                        let call_value = call.try_as_basic_value().unwrap_left();
                        self.locals.insert(*id, call_value);

                        Some(call_value.as_basic_value_enum())
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

                        let candy_fn_type = self.candy_value_pointer_type.fn_type(&[], true);
                        let inner_fn = fn_ptr
                            .try_as_basic_value()
                            .unwrap_left()
                            .into_pointer_value();

                        let call =
                            self.builder
                                .build_indirect_call(candy_fn_type, inner_fn, &args, "");

                        let call_value = call.try_as_basic_value().unwrap_left();
                        self.locals.insert(*id, call_value);

                        Some(call_value.as_basic_value_enum())
                    }
                }
                Expression::UseModule { .. } => unreachable!(),
                Expression::Panic { reason, .. } => {
                    let panic_fn = self.module.get_function("candy_panic").unwrap();

                    let reason = self.get_value_with_id(function_ctx, reason).unwrap();

                    self.builder.build_call(panic_fn, &[reason.into()], "");

                    self.builder.build_unreachable();

                    // Early return to avoid building a return instruction.
                    return None;
                }
                Expression::TraceCallStarts { .. } => unimplemented!(),
                Expression::TraceCallEnds { .. } => unimplemented!(),
                Expression::TraceExpressionEvaluated { .. } => {
                    unimplemented!()
                }
                Expression::TraceFoundFuzzableFunction { .. } => {
                    unimplemented!()
                }
            };

            if let Some(expr_value) = expr_value {
                return_value.replace(expr_value);
            }
        }
        let fn_name = function_ctx.function_value.get_name().to_string_lossy();
        // This "main" refers to the entrypoint of the compiled program, not to the Candy main function
        // which may be named differently.
        if fn_name != "main" {
            self.builder
                .build_return(return_value.as_ref().map(|v| v as &dyn BasicValue<'ctx>));
        }
        return_value
    }

    fn get_builtin(&mut self, builtin: BuiltinFunction) -> FunctionValue<'ctx> {
        if let Some(function) = self.builtins.get(&builtin) {
            return *function;
        }

        let function = self.add_function(
            &(format!("candy_builtin_{}", builtin.as_ref())),
            vec![self.candy_value_pointer_type.into(); builtin.num_parameters()].as_slice(),
            self.candy_value_pointer_type,
        );
        self.builtins.force_insert(builtin, function);
        function
    }
    fn add_function(
        &self,
        name: &str,
        parameter_types: &[BasicMetadataTypeEnum<'ctx>],
        return_type: impl FunctionReturnType<'ctx>,
    ) -> FunctionValue<'ctx> {
        let function_type = return_type.function_type(parameter_types, false);
        self.module.add_function(name, function_type, None)
    }
    fn create_global(
        &mut self,
        name: &str,
        id: &Id,
        value: impl BasicValue<'ctx>,
    ) -> GlobalValue<'ctx> {
        let global = self
            .module
            .add_global(self.candy_value_pointer_type, None, name);
        self.builder.build_store(global.as_pointer_value(), value);

        global.set_initializer(&self.candy_value_pointer_type.const_null());
        assert!(self.globals.insert(*id, global).is_none());
        global
    }

    fn make_str_literal(&self, text: &str) -> BasicValueEnum<'ctx> {
        let i8_type = self.context.i8_type();
        let i64_type = self.context.i64_type();

        let content: Vec<_> = text
            .chars()
            .chain(std::iter::once('\0'))
            .map(|c| i8_type.const_int(c as u64, false))
            .collect();
        let v = i8_type.const_array(&content);

        let len = i64_type.const_int(text.len() as u64 + 1, false);
        let arr_alloc = self.builder.build_array_alloca(i8_type, len, "");
        self.builder.build_store(arr_alloc, v);

        self.builder
            .build_bitcast(arr_alloc, i8_type.ptr_type(AddressSpace::default()), "")
    }

    fn get_value_with_id(
        &self,
        function_ctx: &FunctionInfo<'ctx>,
        id: &Id,
    ) -> Option<BasicValueEnum<'ctx>> {
        let mut v = self.globals.get(id).map(|a| {
            self.builder
                .build_load(self.candy_value_pointer_type, a.as_pointer_value(), "")
        });
        if v.is_none() && let Some(i) = function_ctx.captured_ids.iter().position(|i| i == id) {
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

            v.replace(self.builder.build_load(self.candy_value_pointer_type, env_value, ""));
        }
        if v.is_none() && let Some(value) = self.locals.get(id) {
            v.replace(*value);
        }
        if self.unrepresented_ids.contains(id) {
            v.replace(
                self.candy_value_pointer_type
                    .const_null()
                    .as_basic_value_enum(),
            );
        }
        v.unwrap_or_else(|| panic!("{id} should be a real ID"))
            .into()
    }
}

trait FunctionReturnType<'ctx> {
    fn function_type(
        self,
        param_types: &[BasicMetadataTypeEnum<'ctx>],
        is_var_args: bool,
    ) -> FunctionType<'ctx>;
}
macro_rules! impl_function_return_type {
    ($($type:ty),*) => {
        $(
            impl<'ctx> FunctionReturnType<'ctx> for $type {
                fn function_type(self, param_types: &[BasicMetadataTypeEnum<'ctx>], is_var_args: bool) -> FunctionType<'ctx> {
                    self.fn_type(param_types, is_var_args)
                }
            }
        )*
    };
}
impl_function_return_type!(IntType<'ctx>, PointerType<'ctx>, VoidType<'ctx>);
