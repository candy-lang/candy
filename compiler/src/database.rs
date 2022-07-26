use crate::{
    compiler::{
        ast_to_hir::AstToHirStorage, cst::CstDbStorage, cst_to_ast::CstToAstStorage,
        hir::HirDbStorage, hir_to_lir::HirToLirStorage, rcst_to_cst::RcstToCstStorage,
        string_to_rcst::StringToRcstStorage,
    },
    input::{GetOpenInputQuery, Input, InputDbStorage, InputWatcher},
    language_server::{
        folding_range::FoldingRangeDbStorage, references::ReferencesDbStorage,
        semantic_tokens::SemanticTokenDbStorage, utils::LspPositionConversionStorage,
    },
};
use im::HashMap;
use lazy_static::lazy_static;
use std::{path::PathBuf, sync::Mutex};

#[salsa::database(
    AstToHirStorage,
    CstDbStorage,
    CstToAstStorage,
    FoldingRangeDbStorage,
    HirDbStorage,
    HirToLirStorage,
    InputDbStorage,
    LspPositionConversionStorage,
    RcstToCstStorage,
    ReferencesDbStorage,
    SemanticTokenDbStorage,
    StringToRcstStorage
)]
#[derive(Default)]
pub struct Database {
    storage: salsa::Storage<Self>,
    pub open_inputs: HashMap<Input, Vec<u8>>,
}
impl<'a> salsa::Database for Database {}

impl Database {
    pub fn did_open_input(&mut self, input: &Input, content: Vec<u8>) {
        let old_value = self.open_inputs.insert(input.clone(), content);
        if let Some(_) = old_value {
            log::warn!("Input {input} was opened, but it was already open.");
        }

        GetOpenInputQuery.in_db_mut(self).invalidate(input);
    }
    pub fn did_change_input(&mut self, input: &Input, content: Vec<u8>) {
        let old_value = self.open_inputs.insert(input.to_owned(), content);
        if let None = old_value {
            log::warn!("Input {input} was changed, but it wasn't open before.");
        }

        GetOpenInputQuery.in_db_mut(self).invalidate(input);
    }
    pub fn did_close_input(&mut self, input: &Input) {
        let old_value = self.open_inputs.remove(input);
        if let None = old_value {
            log::warn!("Input {input} was closed, but it wasn't open before.");
        }

        GetOpenInputQuery.in_db_mut(self).invalidate(input);
    }
}
impl InputWatcher for Database {
    fn get_open_input_raw(&self, input: &Input) -> Option<Vec<u8>> {
        self.open_inputs.get(input).cloned()
    }
}

lazy_static! {
    pub static ref PROJECT_DIRECTORY: Mutex<Option<PathBuf>> = Mutex::new(None);
}
