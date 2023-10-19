use candy_frontend::{ast_to_hir::AstToHir, hir, module::ModuleDb, position::PositionConversionDb};
use lsp_types::Position;

use crate::utils::LspPositionConversion;

pub trait IdToEndOfLine {
    fn id_to_end_of_line(&self, id: hir::Id) -> Option<Position>;
}
impl<DB> IdToEndOfLine for DB
where
    DB: AstToHir + ModuleDb + PositionConversionDb,
{
    fn id_to_end_of_line(&self, id: hir::Id) -> Option<Position> {
        let span = self.hir_id_to_display_span(&id)?;
        let line = self
            .offset_to_lsp_position(id.module.clone(), span.start)
            .line;
        let line_start_offsets = self.line_start_offsets(id.module.clone());
        let last_characer_of_line = if line as usize == line_start_offsets.len() - 1 {
            self.get_module_content(id.module.clone()).unwrap().len()
        } else {
            *line_start_offsets[(line + 1) as usize] - 1
        };
        Some(self.offset_to_lsp_position(id.module, last_characer_of_line.into()))
    }
}
