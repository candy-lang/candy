use super::{
    heap::ObjectPointer,
    value::{Closure, Value},
    Vm,
};
use crate::{
    compiler::{
        hir_to_lir::HirToLir,
        lir::{Instruction, Lir},
    },
    database::Database,
    module::{Module, ModuleDb},
};
use itertools::Itertools;

pub trait UseProvider {
    fn use_asset_module(&self, module: Module) -> Result<Vec<u8>, String>;
    fn use_code_module(&self, module: Module) -> Option<Lir>;
}

pub struct DbUseProvider<'a> {
    pub db: &'a Database,
}
impl<'a> UseProvider for DbUseProvider<'a> {
    fn use_asset_module(&self, module: Module) -> Result<Vec<u8>, String> {
        self.db
            .get_module_content(module.clone())
            .map(|bytes| (*bytes).clone())
            .ok_or_else(|| format!("Couldn't import file '{}'.", module))
    }

    fn use_code_module(&self, module: Module) -> Option<Lir> {
        self.db.lir(module).map(|lir| (*lir).clone())
    }
}

impl Vm {
    pub fn use_asset_module<U: UseProvider>(
        &mut self,
        use_provider: &U,
        current_module: Module,
        relative_path: ObjectPointer,
    ) -> Result<Value, String> {
        let target = Target::parse(self.heap.export(relative_path))?;
        let module = target.resolve_asset_module(current_module)?;
        let content = use_provider.use_asset_module(module)?;
        Ok(Value::list(
            content
                .iter()
                .map(|byte| Value::Int(*byte as u64))
                .collect_vec(),
        ))
    }

    pub fn use_code_module<U: UseProvider>(
        &mut self,
        use_provider: &U,
        current_module: Module,
        relative_path: ObjectPointer,
    ) -> Result<(), String> {
        let target = Target::parse(self.heap.export(relative_path))?;
        let possible_module_locations = target.resolve_code_module(current_module)?;
        let (module, lir) = 'find_existing_module: {
            for possible_module in possible_module_locations {
                if let Some(lir) = use_provider.use_code_module(possible_module.clone()) {
                    break 'find_existing_module (possible_module, lir);
                }
            }
            return Err("couldn't import module".to_string());
        };

        let module_closure = Value::Closure(Closure::of_lir(module.clone(), lir));
        let address = self.heap.import(module_closure);
        self.data_stack.push(address);
        self.run_instruction(use_provider, Instruction::Call { num_args: 0 });
        Ok(())
    }
}

struct Target {
    parent_navigations: usize,
    path: String,
}
impl Target {
    const PARENT_NAVIGATION_CHAR: char = '.';

    fn parse(path: Value) -> Result<Self, String> {
        let path = match path {
            Value::Text(path) => path,
            _ => return Err("the path has to be a text".to_string()),
        };
        let mut path = path.as_str();
        let parent_navigations = {
            let mut navigations = 0;
            while path.chars().next() == Some(Target::PARENT_NAVIGATION_CHAR) {
                navigations += 1;
                path = &path[Target::PARENT_NAVIGATION_CHAR.len_utf8()..];
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
        Ok(Target {
            parent_navigations,
            path,
        })
    }

    fn resolve_asset_module(&self, current_module: Module) -> Result<Module, String> {
        let mut path = current_module.path;
        if self.parent_navigations == 0 && path.last() != Some(&".candy".to_string()) {
            return Err(
                "importing child files (starting with a single dot) only works from `.candy` files"
                    .to_string(),
            );
        }
        for _ in 0..self.parent_navigations {
            if path.pop() == None {
                return Err("too many parent navigations".to_string());
            }
        }
        path.push(self.path.to_string());
        Ok(Module {
            package: current_module.package,
            path: path.clone(),
        })
    }

    fn resolve_code_module(&self, current_module: Module) -> Result<Vec<Module>, String> {
        if self.path.contains('.') {
            return Err("the target name contains a file ending".to_string());
        }

        let mut path = current_module.path;
        for _ in 0..self.parent_navigations {
            if path.pop() == None {
                return Err("too many parent navigations".to_string());
            }
        }
        let possible_paths = vec![
            path.clone()
                .into_iter()
                .chain([format!("{}.candy", self.path)])
                .collect_vec(),
            path.clone()
                .into_iter()
                .chain([self.path.to_string(), ".candy".to_string()])
                .collect_vec(),
        ];
        let mut possible_modules = vec![];
        for path in possible_paths {
            possible_modules.push(Module {
                package: current_module.package.clone(),
                path,
            });
        }
        Ok(possible_modules)
    }
}
