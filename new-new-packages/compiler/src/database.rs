use im::HashMap;

use crate::{
    compiler::{
        ast_to_hir::AstToHirStorage, cst::CstDbStorage, cst_to_ast::CstToAstStorage,
        hir::HirDbStorage, rcst_to_cst::RcstToCstStorage, string_to_rcst::StringToRcstStorage,
    },
    discover::run::DiscoverStorage,
    input::{GetOpenInputQuery, Input, InputDbStorage, InputWatcher},
    language_server::{
        folding_range::FoldingRangeDbStorage, hints::HintsDbStorage,
        semantic_tokens::SemanticTokenDbStorage, utils::LspPositionConversionStorage,
    },
};

#[salsa::database(
    AstToHirStorage,
    CstDbStorage,
    CstToAstStorage,
    DiscoverStorage,
    FoldingRangeDbStorage,
    HintsDbStorage,
    HirDbStorage,
    InputDbStorage,
    LspPositionConversionStorage,
    SemanticTokenDbStorage,
    StringToRcstStorage,
    RcstToCstStorage
)]
#[derive(Default)]
pub struct Database {
    storage: salsa::Storage<Self>,
    open_inputs: HashMap<Input, String>,
}
impl<'a> salsa::Database for Database {}

impl Database {
    pub fn did_open_input(&mut self, input: &Input, content: String) {
        let old_value = self.open_inputs.insert(input.clone(), content);
        if let Some(_) = old_value {
            log::warn!("Input {:?} was opened, but it was already open.", input);
        }

        GetOpenInputQuery.in_db_mut(self).invalidate(input);
    }
    pub fn did_change_input(&mut self, input: &Input, content: String) {
        let old_value = self.open_inputs.insert(input.to_owned(), content);
        if let None = old_value {
            log::warn!("Input {:?} was changed, but it wasn't open before.", input);
        }

        GetOpenInputQuery.in_db_mut(self).invalidate(input);
    }
    pub fn did_close_input(&mut self, input: &Input) {
        let old_value = self.open_inputs.remove(input);
        if let None = old_value {
            log::warn!("Input {:?} was closed, but it wasn't open before.", input);
        }

        GetOpenInputQuery.in_db_mut(self).invalidate(input);
    }
}
impl InputWatcher for Database {
    fn get_open_input_raw(&self, input: &Input) -> Option<String> {
        self.open_inputs.get(input).cloned()
    }
}
