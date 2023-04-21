use crate::module::Module;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum MirError {
    UseWithInvalidPath { module: Module, path: String },
    UseHasTooManyParentNavigations { module: Module, path: String },
    ModuleNotFound { module: Module, path: String },
    UseNotStaticallyResolvable { containing_module: Module },
    ModuleHasCycle { cycle: Vec<String> },
}
