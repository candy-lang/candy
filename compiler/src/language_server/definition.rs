use super::utils::LspPositionConversion;
use crate::{
    compiler::{
        ast_to_hir::AstToHir,
        cst::{CstDb, CstKind},
        hir::{Expression, HirDb},
    },
    database::Database,
    module::{Module, ModuleKind},
};
use lsp_types::{GotoDefinitionParams, GotoDefinitionResponse, LocationLink};
use std::path::PathBuf;

pub fn find_definition(
    db: &Database,
    project_directory: PathBuf,
    params: GotoDefinitionParams,
) -> Option<GotoDefinitionResponse> {
    let params = params.text_document_position_params;
    let module = Module::from_package_root_and_url(
        project_directory,
        params.text_document.uri.clone(),
        ModuleKind::Code,
    );
    let position = params.position;
    let offset = db.offset_from_lsp(module.clone(), position.line, position.character);

    let origin_cst = db.find_cst_by_offset(module.clone(), offset);
    match origin_cst.kind {
        CstKind::Identifier { .. } => {}
        _ => return None,
    }

    let origin_hir_id = db.cst_to_hir_id(module.clone(), origin_cst.id)?;
    let origin_expression = db.find_expression(origin_hir_id)?;
    let target_hir_id = match origin_expression {
        Expression::Reference(id) => id,
        _ => return None,
    };
    let target_cst_id = db.hir_to_cst_id(target_hir_id)?;
    let target_cst = db.find_cst(module.clone(), target_cst_id);

    let result = GotoDefinitionResponse::Link(vec![LocationLink {
        origin_selection_range: Some(db.range_to_lsp(module.clone(), origin_cst.span)),
        target_uri: params.text_document.uri,
        target_range: db.range_to_lsp(module.clone(), target_cst.span.clone()),
        target_selection_range: db.range_to_lsp(module, target_cst.display_span()),
    }]);
    Some(result)
}
