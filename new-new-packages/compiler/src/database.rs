use im::HashMap;

use crate::{
    compiler::{
        ast_to_hir::AstToHirStorage, cst_to_ast::CstToAstStorage, string_to_cst::StringToCstStorage,
    },
    input::{GetOpenInputQuery, InputReference, InputStorage, InputWatcher},
    language_server::{
        folding_range::FoldingRangeDbStorage, hints::HintsDbStorage,
        semantic_tokens::SemanticTokenDbStorage, utils::LspPositionConversionStorage,
    },
};

// HintsDbStorage
#[salsa::database(
    AstToHirStorage,
    CstToAstStorage,
    FoldingRangeDbStorage,
    InputStorage,
    LspPositionConversionStorage,
    SemanticTokenDbStorage,
    StringToCstStorage,
    HintsDbStorage
)]
#[derive(Default)]
pub struct Database {
    storage: salsa::Storage<Self>,
    open_inputs: HashMap<InputReference, String>,
}
impl<'a> salsa::Database for Database {}

impl Database {
    pub fn did_open_input(&mut self, input_reference: &InputReference, content: String) {
        let current_value = self.open_inputs.insert(input_reference.clone(), content);
        assert!(current_value.is_none());
        GetOpenInputQuery
            .in_db_mut(self)
            .invalidate(input_reference);
    }
    pub fn did_change_input(&mut self, input_reference: &InputReference, content: String) {
        self.open_inputs
            .entry(input_reference.to_owned())
            .and_modify(|it| *it = content)
            .or_insert_with(|| panic!("Received a change for an input that was not opened."));
        GetOpenInputQuery
            .in_db_mut(self)
            .invalidate(input_reference);
    }
    pub fn did_close_input(&mut self, input_reference: &InputReference) {
        self.open_inputs
            .remove(input_reference)
            .expect("Input was closed without being opened.");
        GetOpenInputQuery
            .in_db_mut(self)
            .invalidate(input_reference);
    }
}
impl InputWatcher for Database {
    fn get_open_input_raw(&self, input_reference: &InputReference) -> Option<String> {
        self.open_inputs.get(input_reference).cloned()
    }
}
