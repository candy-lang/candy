use super::{
    context::{PanickingUseProvider, UseProvider, UseResult},
    heap::{Closure, Pointer, Text},
    tracer::{dummy::DummyTracer, Tracer},
    Fiber, FiberId,
};
use crate::{
    compiler::lir::Instruction,
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

        match use_provider.use_module(module.clone())? {
            UseResult::Asset(bytes) => {
                let bytes = bytes
                    .iter()
                    .map(|byte| self.heap.create_int((*byte).into()))
                    .collect_vec();
                let list = self.heap.create_list(&bytes);
                self.data_stack.push(list);
            }
            UseResult::Code(lir) => {
                let module_closure = Closure::of_module_lir(lir);
                let address = self.heap.create_closure(module_closure);
                self.data_stack.push(address);
                self.run_instruction(
                    &PanickingUseProvider,
                    &mut DummyTracer.for_fiber(FiberId::root()),
                    Instruction::Call { num_args: 0 },
                );
            }
        }

        Ok(())
    }
}
