use crate::{
    compiler::{hir_to_lir::HirToLir, lir::Lir},
    database::Database,
    input::{Input, InputDb},
};

pub trait UseProvider {
    fn use_asset(&self, input: Input) -> Result<Vec<u8>, String>;
    fn use_local_module(&self, input: Input) -> Option<Lir>;
}

pub struct DbUseProvider<'a> {
    pub db: &'a Database,
}
impl<'a> UseProvider for DbUseProvider<'a> {
    fn use_asset(&self, input: Input) -> Result<Vec<u8>, String> {
        self.db
            .get_input(input.clone())
            .map(|bytes| (*bytes).clone())
            .ok_or_else(|| format!("Couldn't import file '{}'.", input))
    }

    fn use_local_module(&self, input: Input) -> Option<Lir> {
        self.db.lir(input).map(|lir| (*lir).clone())
    }
}

pub struct FunctionUseProvider<'a> {
    pub use_asset: &'a dyn Fn(Input) -> Result<Vec<u8>, String>,
    pub use_local_module: &'a dyn Fn(Input) -> Option<Lir>,
}
impl<'a> UseProvider for FunctionUseProvider<'a> {
    fn use_asset(&self, input: Input) -> Result<Vec<u8>, String> {
        (self.use_asset)(input)
        // self.db
        //     .get_input(input)
        //     .map(|bytes| (*bytes).clone())
        //     .ok_or_else(|| format!("Couldn't import file '{}'.", input))
    }

    fn use_local_module(&self, input: Input) -> Option<Lir> {
        (self.use_local_module)(input)
        // self.db.lir(input).map(|lir| (*lir).clone())
    }
}
