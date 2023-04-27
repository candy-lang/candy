use super::{
    context::UseProvider,
    fiber::Fiber,
    heap::{Closure, Text},
};
use crate::heap::{HirId, InlineObject};
use candy_frontend::{
    hir::Id,
    module::{Module, UsePath},
    rich_ir::ToRichIr,
};

impl Fiber {
    pub fn use_module(
        &mut self,
        use_provider: &dyn UseProvider,
        current_module: Module,
        relative_path: InlineObject,
    ) -> Result<(), String> {
        let path: Text = relative_path
            .try_into()
            .map_err(|_| "The path has to be a text.".to_string())?;
        let target = UsePath::parse(path.get())?;
        let module = target.resolve_relative_to(current_module)?;

        let lir = use_provider.use_module(module.clone()).ok_or_else(|| {
            format!(
                "`use` couldn't import the module `{}`.",
                module.to_rich_ir(),
            )
        })?;
        let closure = Closure::create_from_module_lir(&mut self.heap, lir.as_ref().to_owned());
        let responsible = HirId::create(&mut self.heap, Id::dummy());
        self.call_closure(closure, &[], responsible);

        Ok(())
    }
}
