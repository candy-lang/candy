use lsp_types::{GotoDefinitionParams, GotoDefinitionResponse, LocationLink};

use crate::{
    compiler::{
        ast_to_hir::AstToHir,
        cst::{CstDb, CstKind},
        hir::{Expression, HirDb},
    },
    database::Database,
    input::Input,
};

use super::utils::LspPositionConversion;

pub fn find_definition(
    db: &Database,
    params: GotoDefinitionParams,
) -> Option<GotoDefinitionResponse> {
    let params = params.text_document_position_params;
    let input: Input = params.text_document.uri.clone().into();
    let position = params.position;
    let offset = db.position_to_utf8_byte_offset(position.line, position.character, input.clone());

    let origin_cst = db.find_cst_by_offset(input.clone(), offset);
    match origin_cst.kind {
        CstKind::Identifier { .. } => {}
        _ => return None,
    }

    let origin_hir_id = db.cst_to_hir_id(input.clone(), origin_cst.id)?;
    let origin_expression = db.find_expression(input.clone(), origin_hir_id)?;
    let target_hir_id = match origin_expression {
        Expression::Reference(id) => id,
        _ => return None,
    };
    let target_cst_id = db.hir_to_cst_id(input.clone(), target_hir_id)?;
    let target_cst = db.find_cst(input.clone(), target_cst_id);

    let result = GotoDefinitionResponse::Link(vec![LocationLink {
        origin_selection_range: Some(db.range_to_lsp(input.clone(), origin_cst.span())),
        target_uri: params.text_document.uri,
        target_range: db.range_to_lsp(input.clone(), target_cst.span()),
        target_selection_range: db.range_to_lsp(input, target_cst.display_span()),
    }]);
    Some(result)
}
