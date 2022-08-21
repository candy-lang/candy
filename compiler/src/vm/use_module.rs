use super::{
    context::{Context, UseResult},
    heap::Data,
    Closure, Fiber, Heap, Pointer,
};
use crate::{
    compiler::lir::Instruction,
    module::{Module, ModuleKind},
};
use itertools::Itertools;

impl Fiber {
    pub fn use_module<C: Context>(
        &mut self,
        context: &C,
        current_module: Module,
        relative_path: Pointer,
    ) -> Result<(), String> {
        let target = UsePath::parse(&self.heap, relative_path)?;
        let module = target.resolve_relative_to(current_module)?;

        match context.use_module(module.clone())? {
            UseResult::Asset(bytes) => {
                let bytes = bytes
                    .iter()
                    .map(|byte| self.heap.create_int((*byte).into()))
                    .collect_vec();
                let list = self.heap.create_list(&bytes);
                self.data_stack.push(list);
            }
            UseResult::Code(lir) => {
                let module_closure = Closure::of_lir(module, lir);
                let address = self.heap.create_closure(module_closure);
                self.data_stack.push(address);
                self.run_instruction(context, Instruction::Call { num_args: 0 });
            }
        }

        Ok(())
    }
}

struct UsePath {
    parent_navigations: usize,
    path: String,
}
impl UsePath {
    const PARENT_NAVIGATION_CHAR: char = '.';

    fn parse(heap: &Heap, path: Pointer) -> Result<Self, String> {
        let path = match &heap.get(path).data {
            Data::Text(path) => path.value.clone(),
            _ => return Err("the path has to be a text".to_string()),
        };
        let mut path = path.as_str();
        let parent_navigations = {
            let mut navigations = 0;
            while path.starts_with(UsePath::PARENT_NAVIGATION_CHAR) {
                navigations += 1;
                path = &path[UsePath::PARENT_NAVIGATION_CHAR.len_utf8()..];
            }
            match navigations {
                0 => return Err("the target must start with at least one dot".to_string()),
                i => i - 1, // two dots means one parent navigation
            }
        };
        let path = {
            if !path.chars().all(|c| c.is_ascii_alphanumeric() || c == '.') {
                return Err("the target name can only contain letters and dots".to_string());
            }
            path.to_string()
        };
        Ok(UsePath {
            parent_navigations,
            path,
        })
    }

    fn resolve_relative_to(&self, current_module: Module) -> Result<Module, String> {
        let kind = if self.path.contains('.') {
            ModuleKind::Asset
        } else {
            ModuleKind::Code
        };

        let mut path = current_module.path;
        for _ in 0..self.parent_navigations {
            if path.pop() == None {
                return Err("too many parent navigations".to_string());
            }
        }
        path.push(self.path.to_string());

        Ok(Module {
            package: current_module.package,
            path: path.clone(),
            kind,
        })
    }
}
