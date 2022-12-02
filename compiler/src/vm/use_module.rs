use super::{
    context::{UseProvider, UseResult},
    heap::{Closure, Pointer, Text},
    Fiber,
};
use crate::{
    compiler::hir::Id,
    module::{Module, UsePath},
};
use itertools::Itertools;

impl Fiber {
    pub fn use_module(
        &mut self,
        use_provider: &dyn UseProvider,
        current_module: Module,
        relative_path: Pointer,
    ) -> Result<(), String> {
        let path: Text = self
            .heap
            .get(relative_path)
            .data
            .clone()
            .try_into()
            .map_err(|_| "the path has to be a text".to_string())?;
        let target = UsePath::parse(path.value.as_str())?;
        let module = target.resolve_relative_to(current_module)?;

        match use_provider.use_module(module)? {
            UseResult::Asset(bytes) => {
                let bytes = bytes
                    .iter()
                    .map(|byte| self.heap.create_int((*byte).into()))
                    .collect_vec();
                let list = self.heap.create_list(bytes);
                self.data_stack.push(list);
            }
            UseResult::Code(lir) => {
                let closure = self.heap.create_closure(Closure::of_module_lir(lir));
                let responsible = self.heap.create_hir_id(Id::dummy());
                self.call(closure, vec![], responsible);
            }
        }

        Ok(())
    }
}
