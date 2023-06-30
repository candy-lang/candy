use candy_frontend::mir::{Body, Id, Mir};
use inkwell::{
    basic_block::BasicBlock,
    builder::Builder,
    context::Context,
    module::{Linkage, Module},
    types::{PointerType, StructType},
    values::{BasicValue, FunctionValue},
    AddressSpace,
};

pub use inkwell;
use std::{collections::HashMap, rc::Rc, sync::Arc};

pub struct CodeGen<'ctx> {
    context: &'ctx Context,
    module: Module<'ctx>,
    builder: Builder<'ctx>,
    mir: Arc<Mir>,
    tags: HashMap<String, Option<Id>>,
    values: HashMap<Id, Rc<dyn BasicValue<'ctx> + 'ctx>>,
    candy_type: Option<Rc<PointerType<'ctx>>>,
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
            candy_type: None,
        }
    }

    pub fn compile(mut self) {
        let i32_type = self.context.i32_type();
        let i64_type = self.context.i64_type();

        let candy_type = self.context.opaque_struct_type("candy_type");
        let candy_type_ptr = candy_type.ptr_type(AddressSpace::default());

        self.candy_type.replace(Rc::new(candy_type_ptr));

        let make_int_fn_type = candy_type_ptr.fn_type(&[i32_type.into()], false);
        self.module
            .add_function("make_candy_int", make_int_fn_type, Some(Linkage::External));

        let main_type = i32_type.fn_type(&[], false);
        let main_fn = self.module.add_function("main", main_type, None);
        let block = self.context.append_basic_block(main_fn, "entry");
        self.builder.position_at_end(block);
        self.compile_mir(&self.mir.body.clone());
        self.builder.position_at_end(block);
        self.builder
            .build_call(self.module.get_function("candy_main").unwrap(), &[], "");
        let ret_value = i32_type.const_int(0, false);
        self.builder.build_return(Some(&ret_value));
        self.module.print_to_stderr();
        self.module
            .write_bitcode_to_path(std::path::Path::new("module.bc"));
    }

    pub fn compile_mir(&mut self, mir: &Body) {
        for (idx, (id, expr)) in mir.expressions.iter().enumerate() {
            //dbg!(expr);
            match expr {
                candy_frontend::mir::Expression::Int(value) => {
                    let i128_type = self.context.i128_type();
                    let v = i128_type.const_int(value.try_into().unwrap(), false);
                    let make_candy_int = self.module.get_function("make_candy_int").unwrap();
                    self.builder.build_call(make_candy_int, &[v.into()], "");

                    //self.values.insert(*id, v);
                }
                candy_frontend::mir::Expression::Text(_) => todo!(),
                candy_frontend::mir::Expression::Tag { symbol, value } => {
                    self.tags.insert(symbol.clone(), *value);
                    let i32_type = self.context.i32_type();

                    let global = self.module.add_global(i32_type, None, &symbol);

                    let tag = i32_type.const_int(self.tags.len().try_into().unwrap(), false);
                    global.set_initializer(&tag);
                }
                candy_frontend::mir::Expression::Builtin(_) => todo!(),
                candy_frontend::mir::Expression::List(_) => todo!(),
                candy_frontend::mir::Expression::Struct(s) => {
                    for (id1, id2) in s {
                        dbg!(id1, id2);
                    }
                    self.context.struct_type(&[], false);
                }
                candy_frontend::mir::Expression::Reference(id) => {
                    if let Some(v) = self.values.get(id) {
                        self.builder.build_return(Some(v.as_ref()));
                    }
                }
                candy_frontend::mir::Expression::HirId(_) => todo!(),
                candy_frontend::mir::Expression::Function {
                    original_hirs,
                    parameters,
                    responsible_parameter,
                    body,
                } => {
                    let original_name = &original_hirs.iter().next().unwrap().keys[0].to_string();
                    let name = match original_name.as_str() {
                        "main" => "candy_main",
                        other => other,
                    };

                    let fn_type = self
                        .context
                        .get_struct_type("candy_type")
                        .unwrap()
                        .ptr_type(AddressSpace::default())
                        .fn_type(&[], false);

                    let function = self.module.add_function(name, fn_type, None);

                    let inner_block = self.context.append_basic_block(function, name);
                    self.builder.position_at_end(inner_block);
                    self.compile_mir(body);
                }
                candy_frontend::mir::Expression::Parameter => todo!(),
                candy_frontend::mir::Expression::Call {
                    function,
                    arguments,
                    responsible,
                } => todo!(),
                candy_frontend::mir::Expression::UseModule {
                    current_module,
                    relative_path,
                    responsible,
                } => todo!(),
                candy_frontend::mir::Expression::Panic {
                    reason,
                    responsible,
                } => todo!(),
                candy_frontend::mir::Expression::TraceCallStarts {
                    hir_call,
                    function,
                    arguments,
                    responsible,
                } => todo!(),
                candy_frontend::mir::Expression::TraceCallEnds { return_value } => todo!(),
                candy_frontend::mir::Expression::TraceExpressionEvaluated {
                    hir_expression,
                    value,
                } => todo!(),
                candy_frontend::mir::Expression::TraceFoundFuzzableFunction {
                    hir_definition,
                    function,
                } => todo!(),
            }
        }
    }
}
