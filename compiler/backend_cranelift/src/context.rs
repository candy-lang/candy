use crate::runtime::RuntimeFunction;
use candy_frontend::lir::{BodyId, ConstantId, Id};
use cranelift::codegen::ir::{Signature, Value};
use cranelift_module::{DataId, FuncId};
use rustc_hash::FxHashMap;

#[derive(Default)]
pub struct CodegenContext {
    constants: FxHashMap<ConstantId, DataId>,
    functions: Vec<FunctionContext>,
}

impl CodegenContext {
    pub fn insert_constant(&mut self, id: ConstantId, data_id: DataId) {
        self.constants.insert(id, data_id);
    }

    pub fn constants(&self) -> impl Iterator<Item = (ConstantId, DataId)> {
        self.constants.clone().into_iter()
    }

    pub fn get_constant(&self, id: &ConstantId) -> Option<DataId> {
        self.constants.get(id).cloned()
    }

    pub fn insert_function(&mut self, function: FunctionContext) {
        self.functions.push(function);
    }

    pub fn get_function(&self, name: &str) -> Option<FunctionContext> {
        self.functions.iter().find(|f| f.name == name).cloned()
    }

    pub fn get_function_by_body(&self, id: BodyId) -> Option<FunctionContext> {
        self.functions
            .iter()
            .find(|f| f.body_id == Some(id))
            .cloned()
    }

    pub fn get_runtime_function(&self, function: &RuntimeFunction) -> FunctionContext {
        self.get_function(function.name()).unwrap()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionContext {
    pub id: FuncId,
    pub body_id: Option<BodyId>,
    pub name: String,
    pub signature: Signature,
    pub variables: FxHashMap<Id, Value>,
    pub captured: Vec<Id>,
}
