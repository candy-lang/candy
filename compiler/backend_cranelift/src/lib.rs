mod context;
mod runtime;

use crate::runtime::RuntimeFunction;
use candy_frontend::{
    builtin_functions::BuiltinFunction,
    id::CountableId,
    lir::{Body, BodyId, Constant, ConstantId, Expression, Id, Lir},
};
use context::{CodegenContext, FunctionContext};
use cranelift::codegen::{
    ir::{immediates::Offset32, Function, UserFuncName},
    isa::{lookup, TargetIsa},
    settings::Flags,
    verifier::VerifierErrors,
    CodegenError, Context,
};
use cranelift::prelude::*;
use cranelift_module::{DataDescription, Linkage, Module, ModuleError};
use cranelift_object::{ObjectBuilder, ObjectModule};
use derive_more::From;
use rustc_hash::FxHashMap;
use std::{fs::File, sync::Arc};
use strum::IntoEnumIterator;

#[derive(Debug, From)]
pub enum CraneliftError {
    ModuleError(ModuleError),
    CodegenError(CodegenError),
    VerifierErrors(VerifierErrors),
    Other(String),
}

pub struct CodeGen {
    lir: Arc<Lir>,
    module: ObjectModule,
    mod_name: String,
    context: CodegenContext,
}

impl CodeGen {
    pub fn new(lir: Arc<Lir>, mod_name: String) -> Self {
        let mut shared_builder = settings::builder();
        shared_builder.enable("is_pic").unwrap();
        let shared_flags = Flags::new(shared_builder);
        let target = lookup(target_lexicon::HOST)
            .unwrap()
            .finish(shared_flags)
            .unwrap();
        let obj_builder = ObjectBuilder::new(
            target.clone(),
            mod_name.bytes().collect::<Vec<_>>(),
            cranelift_module::default_libcall_names(),
        )
        .unwrap();

        Self {
            lir,
            module: ObjectModule::new(obj_builder),
            mod_name,
            context: CodegenContext::default(),
        }
    }

    fn target(&self) -> &dyn TargetIsa {
        self.module.isa()
    }

    fn declare_runtime_functions(&mut self) -> Result<(), CraneliftError> {
        for rt_builtin in RuntimeFunction::iter() {
            let mut sig = self.module.make_signature();
            sig.params.extend(vec![
                AbiParam::new(self.target().pointer_type());
                rt_builtin.num_params()
            ]);
            if rt_builtin.has_return_value() {
                sig.returns
                    .push(AbiParam::new(self.target().pointer_type()));
            }

            let func_id = self
                .module
                .declare_function(rt_builtin.name(), Linkage::Import, &sig)?;

            self.context.insert_function(FunctionContext {
                id: func_id,
                name: rt_builtin.name().to_string(),
                signature: sig.clone(),
                variables: FxHashMap::default(),
                captured: vec![],
                body_id: None,
            });
        }

        Ok(())
    }

    pub fn compile(mut self) -> Result<(), CraneliftError> {
        let lir = self.lir.clone();
        self.declare_runtime_functions()?;
        for (id, _) in lir.constants().ids_and_constants() {
            let data_id =
                self.module
                    .declare_data(&id.to_string(), Linkage::Export, true, false)?;
            let mut data = DataDescription::new();
            data.define_zeroinit(self.target().pointer_bytes() as usize);
            self.module.define_data(data_id, &data)?;
            self.context.insert_constant(id, data_id);
        }
        for (id, body) in lir.bodies().ids_and_bodies() {
            self.compile_body(body, id)?;
        }
        self.compile_main()?;

        let product = self.module.finish();

        let obj_name = format!("{}.o", self.mod_name.trim_end_matches(".candy"));
        let mut file = File::create(obj_name).unwrap();
        product.object.write_stream(&mut file).unwrap();

        Ok(())
    }

    fn compile_main(&mut self) -> Result<(), CraneliftError> {
        let lir = self.lir.clone();
        let mut signature = self.module.make_signature();
        signature.returns.push(AbiParam::new(types::I32));
        let func_id =
            self.module
                .declare_function("main", cranelift_module::Linkage::Export, &signature)?;
        let func_name = UserFuncName::user(0, func_id.as_u32());

        let mut func = Function::with_name_signature(func_name, signature);
        let mut func_ctx = FunctionBuilderContext::new();
        let mut func_builder = FunctionBuilder::new(&mut func, &mut func_ctx);

        let fn_entry = func_builder.create_block();
        func_builder.append_block_params_for_function_params(fn_entry);
        func_builder.switch_to_block(fn_entry);
        func_builder.seal_block(fn_entry);

        for (const_id, data_id) in self.context.constants() {
            let global = self.module.declare_data_in_func(data_id, func_builder.func);
            let constant = lir.constants().get(const_id);
            let value = self.compile_constant(constant, const_id, &mut func_builder)?;
            let addr = func_builder
                .ins()
                .global_value(self.target().pointer_type(), global);
            func_builder
                .ins()
                .store(MemFlags::new(), value, addr, Offset32::new(0));
        }

        let (last_body_id, _) = lir.bodies().ids_and_bodies().last().unwrap();
        let last_func_ctx = self
            .context
            .get_function(&last_body_id.to_string())
            .unwrap();
        let last_func = self
            .module
            .declare_func_in_func(last_func_ctx.id, func_builder.func);
        let null = func_builder.ins().iconst(self.target().pointer_type(), 0);
        let res = func_builder.ins().call(last_func, &[null, null]);
        let main_addr = func_builder.inst_results(res)[0];

        let run_main_ctx = self.context.get_function("run_candy_main").unwrap();
        let run_main = self
            .module
            .declare_func_in_func(run_main_ctx.id, func_builder.func);
        let env_id = self
            .module
            .declare_data("candy_environment", Linkage::Import, true, false)?;
        let env = self.module.declare_data_in_func(env_id, func_builder.func);
        let env_value = func_builder
            .ins()
            .global_value(self.target().pointer_type(), env);
        let res = func_builder.ins().call(run_main, &[main_addr, env_value]);
        let return_value = func_builder.inst_results(res)[0];

        let print_ctx = self.context.get_function("print_candy_value").unwrap();
        let print_fun = self
            .module
            .declare_func_in_func(print_ctx.id, func_builder.func);
        func_builder.ins().call(print_fun, &[return_value]);

        let zero = func_builder.ins().iconst(types::I32, 0);
        func_builder.ins().return_(&[zero]);
        func_builder.finalize();

        println!("{}", func.display());

        let mut fn_ctx = Context::for_function(func);

        fn_ctx.compute_cfg();
        fn_ctx.compute_domtree();
        if let Err(errors) = fn_ctx.verify(self.target()) {
            for error in errors.0 {
                println!("{error}");
            }
            std::process::exit(1);
        }
        fn_ctx.dce(self.target())?;
        fn_ctx.eliminate_unreachable_code(self.target())?;
        fn_ctx.replace_redundant_loads()?;
        fn_ctx.egraph_pass(self.target())?;

        self.module.define_function(func_id, &mut fn_ctx)?;
        Ok(())
    }

    fn compile_body(&mut self, body: &Body, id: BodyId) -> Result<(), CraneliftError> {
        let mut signature = self.module.make_signature();
        signature.params.extend(
            vec![AbiParam::new(self.target().pointer_type()); body.parameter_count() + 2].iter(),
        );
        signature
            .returns
            .push(AbiParam::new(self.target().pointer_type()));

        let func_id = self.module.declare_function(
            &id.to_string(),
            cranelift_module::Linkage::Export,
            &signature,
        )?;
        let func_name = UserFuncName::user(0, id.to_usize() as u32);

        let mut func = Function::with_name_signature(func_name, signature.clone());
        let mut func_ctx = FunctionBuilderContext::new();
        let mut func_builder = FunctionBuilder::new(&mut func, &mut func_ctx);

        let fn_entry = func_builder.create_block();
        func_builder.append_block_params_for_function_params(fn_entry);
        func_builder.switch_to_block(fn_entry);
        func_builder.seal_block(fn_entry);

        let mut ctx = FunctionContext {
            id: func_id,
            name: id.to_string(),
            signature,
            variables: FxHashMap::default(),
            captured: body.captured_ids().collect(),
            body_id: Some(id),
        };

        let params = func_builder.block_params(fn_entry);
        for (param_id, param) in body.parameter_ids().zip(params) {
            ctx.variables.insert(param_id, *param);
        }
        ctx.variables
            .insert(body.responsible_parameter_id(), params[params.len() - 2]);

        for (id, expr) in body.ids_and_expressions() {
            let val = self.compile_expr(expr, &mut func_builder, &mut ctx, id)?;
            ctx.variables.insert(id, val);
            if id == body.last_expression_id().unwrap() {
                func_builder.ins().return_(&[val]);
            }
        }

        println!("{}", func_builder.func.display());

        func_builder.finalize();
        let mut fn_ctx = Context::for_function(func);

        fn_ctx.compute_cfg();
        fn_ctx.compute_domtree();
        if let Err(errors) = fn_ctx.verify(self.target()) {
            for error in errors.0 {
                println!("{error}");
            }
            std::process::exit(1);
        }
        fn_ctx.dce(self.target())?;
        fn_ctx.eliminate_unreachable_code(self.target())?;
        fn_ctx.replace_redundant_loads()?;
        fn_ctx.egraph_pass(self.target())?;

        self.module.define_function(func_id, &mut fn_ctx)?;
        self.context.insert_function(ctx);
        Ok(())
    }

    fn get_builtin(
        &mut self,
        builtin: &BuiltinFunction,
    ) -> Result<FunctionContext, CraneliftError> {
        match self.context.get_function(builtin.as_ref()) {
            Some(b) => Ok(b),
            None => {
                let mut signature = self.module.make_signature();
                signature.params.extend(vec![
                    AbiParam::new(self.target().pointer_type());
                    builtin.num_parameters()
                ]);
                signature
                    .returns
                    .push(AbiParam::new(self.target().pointer_type()));
                let func = self.module.declare_function(
                    &format!("candy_builtin_{}", builtin.as_ref()),
                    Linkage::Import,
                    &signature,
                )?;

                let ctx = FunctionContext {
                    id: func,
                    name: builtin.as_ref().to_string(),
                    signature,
                    variables: FxHashMap::default(),
                    captured: vec![],
                    body_id: None,
                };
                self.context.insert_function(ctx);
                Ok(self.context.get_function(builtin.as_ref()).unwrap())
            }
        }
    }

    fn compile_constant(
        &mut self,
        constant: &Constant,
        id: ConstantId,
        func_builder: &mut FunctionBuilder,
    ) -> Result<Value, CraneliftError> {
        match constant {
            candy_frontend::lir::Constant::Int(value) => {
                let val: i64 = value.try_into().unwrap();
                let func = self.context.get_runtime_function(&RuntimeFunction::MakeInt);
                let func_ref = self.module.declare_func_in_func(func.id, func_builder.func);
                let args = [func_builder.ins().iconst(types::I64, val)];
                let call_inst = func_builder.ins().call(func_ref, &args);
                Ok(func_builder.inst_results(call_inst)[0])
            }
            candy_frontend::lir::Constant::Text(value) => {
                let text_data = self
                    .module
                    .declare_data(value, Linkage::Export, true, false)?;
                let mut data = DataDescription::new();
                data.define(value.bytes().collect());
                self.module.define_data(text_data, &data)?;
                let data_value = self
                    .module
                    .declare_data_in_func(text_data, func_builder.func);

                let func = self
                    .context
                    .get_runtime_function(&RuntimeFunction::MakeText);
                let func_ref = self.module.declare_func_in_func(func.id, func_builder.func);
                let args = [func_builder
                    .ins()
                    .global_value(self.target().pointer_type(), data_value)];
                let call_inst = func_builder.ins().call(func_ref, &args);
                Ok(func_builder.inst_results(call_inst)[0])
            }
            candy_frontend::lir::Constant::Tag { symbol, value } => {
                let func = self.context.get_runtime_function(&RuntimeFunction::MakeTag);
                let func_ref = self.module.declare_func_in_func(func.id, func_builder.func);
                let symbol_data = self
                    .module
                    .declare_data(symbol, Linkage::Export, true, false)?;
                let mut data = DataDescription::new();
                data.define(symbol.bytes().collect());
                self.module.define_data(symbol_data, &data)?;
                let data_value = self
                    .module
                    .declare_data_in_func(symbol_data, func_builder.func);
                let args = vec![
                    func_builder
                        .ins()
                        .global_value(self.target().pointer_type(), data_value),
                    func_builder.ins().iconst(self.target().pointer_type(), 0),
                ];
                let call_inst = func_builder.ins().call(func_ref, &args);
                Ok(func_builder.inst_results(call_inst)[0])
            }
            candy_frontend::lir::Constant::Builtin(value) => {
                let builtin = self.get_builtin(value)?;
                let func = self
                    .module
                    .declare_func_in_func(builtin.id, func_builder.func);
                let addr = func_builder
                    .ins()
                    .func_addr(self.target().pointer_type(), func);
                let make_func_ctx = self
                    .context
                    .get_runtime_function(&RuntimeFunction::MakeFunction);
                let make_func = self
                    .module
                    .declare_func_in_func(make_func_ctx.id, func_builder.func);
                let null = func_builder.ins().iconst(self.target().pointer_type(), 0);
                let call_inst = func_builder.ins().call(make_func, &[addr, null, null]);
                Ok(func_builder.inst_results(call_inst)[0])
            }
            candy_frontend::lir::Constant::List(values) => {
                let data = StackSlotData::new(
                    StackSlotKind::ExplicitSlot,
                    values.len() as u32 * self.target().pointer_bytes() as u32,
                );
                let slot = func_builder.create_sized_stack_slot(data);
                for (idx, id) in values.iter().enumerate() {
                    let data = self.context.get_constant(id).unwrap();
                    let val = self.module.declare_data_in_func(data, func_builder.func);
                    let val = func_builder
                        .ins()
                        .global_value(self.target().pointer_type(), val);
                    func_builder.ins().stack_store(
                        val,
                        slot,
                        Offset32::new(idx as i32 * self.target().pointer_bytes() as i32),
                    );
                }

                let make_list_ctx = self
                    .context
                    .get_runtime_function(&RuntimeFunction::MakeList);
                let make_list = self
                    .module
                    .declare_func_in_func(make_list_ctx.id, func_builder.func);

                let arg = func_builder.ins().stack_addr(
                    self.target().pointer_type(),
                    slot,
                    Offset32::new(0),
                );
                let ret = func_builder.ins().call(make_list, &[arg]);
                Ok(func_builder.inst_results(ret)[0])
            }
            candy_frontend::lir::Constant::Struct(value) => {
                let size = value.len() as u32 * self.target().pointer_bytes() as u32;
                let data = StackSlotData::new(StackSlotKind::ExplicitSlot, size);
                let keys_slot = func_builder.create_sized_stack_slot(data.clone());
                let values_slot = func_builder.create_sized_stack_slot(data);
                for (idx, (key_id, value_id)) in value.iter().enumerate() {
                    let key = self.context.get_constant(key_id).unwrap();
                    let value = self.context.get_constant(value_id).unwrap();

                    let key_val = self.module.declare_data_in_func(key, func_builder.func);
                    let key_val = func_builder
                        .ins()
                        .global_value(self.target().pointer_type(), key_val);

                    let value_val = self.module.declare_data_in_func(value, func_builder.func);
                    let value_val = func_builder
                        .ins()
                        .global_value(self.target().pointer_type(), value_val);

                    func_builder.ins().stack_store(
                        key_val,
                        keys_slot,
                        Offset32::new(idx as i32 * self.target().pointer_bytes() as i32),
                    );
                    func_builder.ins().stack_store(
                        value_val,
                        values_slot,
                        Offset32::new(idx as i32 * self.target().pointer_bytes() as i32),
                    );
                }

                let make_struct_ctx = self
                    .context
                    .get_runtime_function(&RuntimeFunction::MakeStruct);

                let make_struct = self
                    .module
                    .declare_func_in_func(make_struct_ctx.id, func_builder.func);

                let size_val = func_builder.ins().iconst(types::I64, size as i64);
                let keys_addr = func_builder.ins().stack_addr(
                    self.target().pointer_type(),
                    keys_slot,
                    Offset32::new(0),
                );
                let values_addr = func_builder.ins().stack_addr(
                    self.target().pointer_type(),
                    values_slot,
                    Offset32::new(0),
                );

                let call_inst = func_builder
                    .ins()
                    .call(make_struct, &[keys_addr, values_addr, size_val]);

                Ok(func_builder.inst_results(call_inst)[0])
            }
            candy_frontend::lir::Constant::HirId(value) => {
                let func = self.context.get_runtime_function(&RuntimeFunction::MakeTag);
                let func_ref = self.module.declare_func_in_func(func.id, func_builder.func);
                let symbol_data =
                    self.module
                        .declare_data(&value.to_string(), Linkage::Export, true, false)?;
                let mut data = DataDescription::new();
                data.define(value.to_string().bytes().collect());
                self.module.define_data(symbol_data, &data)?;
                let data_value = self
                    .module
                    .declare_data_in_func(symbol_data, func_builder.func);
                let args = vec![
                    func_builder
                        .ins()
                        .global_value(self.target().pointer_type(), data_value),
                    func_builder.ins().iconst(self.target().pointer_type(), 0),
                ];
                let call_inst = func_builder.ins().call(func_ref, &args);
                Ok(func_builder.inst_results(call_inst)[0])
            }
            candy_frontend::lir::Constant::Function(value) => {
                let func = self
                    .context
                    .get_runtime_function(&RuntimeFunction::MakeFunction);
                let func_ref = self.module.declare_func_in_func(func.id, func_builder.func);
                let referenced_func = self.context.get_function(&value.to_string()).unwrap();
                let referenced_func_ref = self
                    .module
                    .declare_func_in_func(referenced_func.id, func_builder.func);
                let addr = func_builder
                    .ins()
                    .func_addr(self.target().pointer_type(), referenced_func_ref);
                let null = func_builder.ins().iconst(self.target().pointer_type(), 0);

                let res = func_builder.ins().call(func_ref, &[addr, null, null]);
                Ok(func_builder.inst_results(res)[0])
            }
        }
    }

    fn compile_expr(
        &mut self,
        expr: &Expression,
        func_builder: &mut FunctionBuilder,
        ctx: &mut FunctionContext,
        id: Id,
    ) -> Result<Value, CraneliftError> {
        match expr {
            Expression::CreateTag { symbol, value } => todo!(),
            Expression::CreateList(values) => {
                let data = StackSlotData::new(
                    StackSlotKind::ExplicitSlot,
                    values.len() as u32 * self.target().pointer_bytes() as u32,
                );
                let slot = func_builder.create_sized_stack_slot(data);
                for (idx, id) in values.iter().enumerate() {
                    let val = self.resolve_id(*id, ctx, func_builder).unwrap();
                    func_builder.ins().stack_store(
                        val,
                        slot,
                        Offset32::new(idx as i32 * self.target().pointer_bytes() as i32),
                    );
                }

                let make_list_ctx = self
                    .context
                    .get_runtime_function(&RuntimeFunction::MakeList);
                let make_list = self
                    .module
                    .declare_func_in_func(make_list_ctx.id, func_builder.func);

                let arg = func_builder.ins().stack_addr(
                    self.target().pointer_type(),
                    slot,
                    Offset32::new(0),
                );
                let ret = func_builder.ins().call(make_list, &[arg]);
                Ok(func_builder.inst_results(ret)[0])
            }
            Expression::CreateStruct(struct_def) => {
                let size = struct_def.len() as u32 * self.target().pointer_bytes() as u32;
                let data = StackSlotData::new(StackSlotKind::ExplicitSlot, size);
                let keys_slot = func_builder.create_sized_stack_slot(data.clone());
                let values_slot = func_builder.create_sized_stack_slot(data);
                for (idx, (key_id, value_id)) in struct_def.iter().enumerate() {
                    let key = self.resolve_id(*key_id, ctx, func_builder).unwrap();
                    let value = self.resolve_id(*value_id, ctx, func_builder).unwrap();

                    func_builder.ins().stack_store(
                        key,
                        keys_slot,
                        Offset32::new(idx as i32 * self.target().pointer_bytes() as i32),
                    );
                    func_builder.ins().stack_store(
                        value,
                        values_slot,
                        Offset32::new(idx as i32 * self.target().pointer_bytes() as i32),
                    );
                }

                let make_struct_ctx = self
                    .context
                    .get_runtime_function(&RuntimeFunction::MakeStruct);

                let make_struct = self
                    .module
                    .declare_func_in_func(make_struct_ctx.id, func_builder.func);

                let size_val = func_builder.ins().iconst(types::I64, size as i64);
                let keys_addr = func_builder.ins().stack_addr(
                    self.target().pointer_type(),
                    keys_slot,
                    Offset32::new(0),
                );
                let values_addr = func_builder.ins().stack_addr(
                    self.target().pointer_type(),
                    values_slot,
                    Offset32::new(0),
                );

                let call_inst = func_builder
                    .ins()
                    .call(make_struct, &[keys_addr, values_addr, size_val]);

                Ok(func_builder.inst_results(call_inst)[0])
            }
            Expression::CreateFunction { captured, body_id } => {
                let func_ctx = self
                    .context
                    .get_runtime_function(&RuntimeFunction::MakeFunction);

                let make_func = self
                    .module
                    .declare_func_in_func(func_ctx.id, func_builder.func);

                let new_fun_ctx = self.context.get_function_by_body(*body_id).unwrap();
                let new_fun = self
                    .module
                    .declare_func_in_func(new_fun_ctx.id, func_builder.func);
                let addr = func_builder
                    .ins()
                    .func_addr(self.target().pointer_type(), new_fun);

                let capture_size = captured.len() as u32 * self.target().pointer_bytes() as u32;

                let data = StackSlotData::new(StackSlotKind::ExplicitSlot, capture_size);
                let capture_slot = func_builder.create_sized_stack_slot(data);

                for (idx, capture) in captured.iter().enumerate() {
                    let val = self
                        .resolve_id(*capture, ctx, func_builder)
                        .unwrap_or_else(|| {
                            panic!(
                                "Captured ID {capture} could not be resolved in function `{}`",
                                ctx.body_id.unwrap()
                            )
                        });
                    func_builder.ins().stack_store(
                        val,
                        capture_slot,
                        Offset32::new(self.target().pointer_bytes() as i32 * idx as i32),
                    );
                }

                let capture_slot_val = func_builder.ins().stack_addr(
                    self.target().pointer_type(),
                    capture_slot,
                    Offset32::new(0),
                );
                let capture_size_val = func_builder.ins().iconst(types::I64, capture_size as i64);
                let call_inst = func_builder
                    .ins()
                    .call(make_func, &[addr, capture_slot_val, capture_size_val]);
                Ok(func_builder.inst_results(call_inst)[0])
            }
            Expression::Constant(const_id) => {
                let data_id = self.context.get_constant(const_id).unwrap();
                let data_value = self.module.declare_data_in_func(data_id, func_builder.func);
                let data = func_builder
                    .ins()
                    .global_value(self.target().pointer_type(), data_value);
                let data = func_builder.ins().load(
                    self.target().pointer_type(),
                    MemFlags::new(),
                    data,
                    Offset32::new(0),
                );
                ctx.variables.insert(id, data);
                Ok(data)
            }
            Expression::Reference(id) => Ok(self.resolve_id(*id, ctx, func_builder).unwrap()),
            Expression::Dup { id, amount } => {
                let dup_ctx = self
                    .context
                    .get_runtime_function(&RuntimeFunction::DupValue);
                let dup_fn = self
                    .module
                    .declare_func_in_func(dup_ctx.id, func_builder.func);

                let arg = self.resolve_id(*id, ctx, func_builder).unwrap();
                let amt = func_builder.ins().iconst(types::I64, *amount as i64);
                func_builder.ins().call(dup_fn, &[arg, amt]);

                Ok(func_builder.ins().iconst(self.target().pointer_type(), 0))
            }
            Expression::Drop(id) => {
                let drop_ctx = self.context.get_function("drop_candy_value").unwrap();
                let drop_fn = self
                    .module
                    .declare_func_in_func(drop_ctx.id, func_builder.func);
                let dropped_value = self.resolve_id(*id, ctx, func_builder).unwrap();
                func_builder.ins().call(drop_fn, &[dropped_value]);
                Ok(func_builder.ins().iconst(self.target().pointer_type(), 0))
            }
            Expression::Call {
                function,
                arguments,
                responsible,
            } => {
                let func_val = self.resolve_id(*function, ctx, func_builder).unwrap();

                let get_func_ctx = self
                    .context
                    .get_runtime_function(&RuntimeFunction::GetFunction);
                let get_func = self
                    .module
                    .declare_func_in_func(get_func_ctx.id, func_builder.func);
                let inst = func_builder.ins().call(get_func, &[func_val]);
                let func = func_builder.inst_results(inst)[0];

                let get_func_caps_ctx = self
                    .context
                    .get_runtime_function(&RuntimeFunction::GetCapture);
                let get_func_caps = self
                    .module
                    .declare_func_in_func(get_func_caps_ctx.id, func_builder.func);
                let inst = func_builder.ins().call(get_func_caps, &[func_val]);
                let captures = func_builder.inst_results(inst)[0];

                let resp = self.resolve_id(*responsible, ctx, func_builder);
                let args: Vec<Value> = arguments
                    .iter()
                    .map(|arg| {
                        self.resolve_id(*arg, ctx, func_builder)
                            .unwrap_or_else(|| panic!("Argument `{arg}` could not be resolved"))
                    })
                    .chain(std::iter::once(resp.unwrap_or_else(|| {
                        panic!("Function `{function}` does not contain responsible `{responsible}`")
                    })))
                    .chain(std::iter::once(captures))
                    .collect();
                let mut sig = self.module.make_signature();
                sig.returns
                    .push(AbiParam::new(self.target().pointer_type()));
                sig.params.extend(vec![
                    AbiParam::new(self.target().pointer_type());
                    args.len()
                ]);
                let sig_ref = func_builder.import_signature(sig);
                let call_inst = func_builder
                    .ins()
                    .call_indirect(sig_ref, func, args.as_ref());
                Ok(func_builder.inst_results(call_inst)[0])
            }
            Expression::Panic {
                reason,
                responsible,
            } => {
                let val = self.resolve_id(*reason, ctx, func_builder).unwrap();
                let panic_ctx = self.context.get_runtime_function(&RuntimeFunction::Panic);
                let panic_fun = self
                    .module
                    .declare_func_in_func(panic_ctx.id, func_builder.func);

                func_builder.ins().call(panic_fun, &[val]);
                Ok(func_builder.ins().iconst(self.target().pointer_type(), 0))
            }
            Expression::TraceCallStarts {
                hir_call,
                function,
                arguments,
                responsible,
            } => todo!(),
            Expression::TraceCallEnds { return_value } => todo!(),
            Expression::TraceExpressionEvaluated {
                hir_expression,
                value,
            } => todo!(),
            Expression::TraceFoundFuzzableFunction {
                hir_definition,
                function,
            } => todo!(),
        }
    }

    fn resolve_id(
        &self,
        id: Id,
        ctx: &FunctionContext,
        func_builder: &mut FunctionBuilder,
    ) -> Option<Value> {
        match ctx.variables.get(&id) {
            Some(val) => Some(*val),
            None => {
                if ctx.captured.contains(&id) {
                    let block = func_builder.current_block().unwrap();
                    let params = func_builder.block_params(block).to_vec();

                    let last = params.last().unwrap();
                    Some(func_builder.ins().load(
                        self.target().pointer_type(),
                        MemFlags::new(),
                        *last,
                        Offset32::new(id.to_usize() as i32 * self.target().pointer_bytes() as i32),
                    ))
                } else {
                    None
                }
            }
        }
    }
}
