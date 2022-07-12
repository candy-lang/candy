use crate::{
    compiler::{ast_to_hir::AstToHir, hir},
    database::Database,
    input::{Input, InputDb},
    language_server::utils::{LspPositionConversion, TupleToPosition},
};
use lsp_types::Position;

pub fn id_to_end_of_line(db: &Database, input: Input, id: hir::Id) -> Option<Position> {
    let span = db.hir_id_to_display_span(id.clone())?;

    let line = db
        .offset_to_lsp(input.clone(), span.start)
        .to_position()
        .line;
    let line_start_offsets = db.line_start_utf8_byte_offsets(input.clone());
    let last_characer_of_line = if line as usize == line_start_offsets.len() - 1 {
        db.get_input(input.clone()).unwrap().len()
    } else {
        line_start_offsets[(line + 1) as usize] - 1
    };
    let position = db
        .offset_to_lsp(input.clone(), last_characer_of_line)
        .to_position();
    Some(position)
}
