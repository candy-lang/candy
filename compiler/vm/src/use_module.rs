use super::{
    context::UseProvider,
    fiber::Fiber,
    heap::{Closure, Pointer, Text},
};
use candy_frontend::{
    hir::Id,
    module::{Module, UsePath},
};

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

        let lir = use_provider
            .use_module(module.clone())
            .ok_or_else(|| format!("`use` couldn't import the module `{}`.", module))?;
        let closure = self
            .heap
            .create_closure(Closure::of_module_lir(lir.as_ref().to_owned()));
        let responsible = self.heap.create_hir_id(Id::dummy());
        self.call(closure, vec![], responsible);

        Ok(())
    }
}
