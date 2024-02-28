use crate::{
    database::Database,
    utils::{module_to_url, LspPositionConversion},
};
use candy_frontend::{
    ast_to_hir::AstToHir,
    cst::{CstDb, CstKind},
    hir::{Expression, HirDb},
    module::Module,
    position::Offset,
};
use lsp_types::LocationLink;
use tracing::{debug, info};

pub fn find_definition(db: &Database, module: Module, offset: Offset) -> Option<LocationLink> {
    let origin_cst = db.find_cst_by_offset(module.clone(), offset);
    info!("Finding definition for {origin_cst:?}");
    match origin_cst.kind {
        CstKind::Identifier { .. } => {}
        _ => return None,
    }

    let origin_hir_id = db.cst_to_last_hir_id(module.clone(), origin_cst.data.id)?;
    let origin_expression = db.find_expression(origin_hir_id)?;
    debug!("Origin HIR: {origin_expression}");
    let Expression::Reference(target_hir_id) = origin_expression else {
        return None;
    };
    let target_cst_id = db.hir_to_cst_id(&target_hir_id)?;
    let target_cst = db.find_cst(module.clone(), target_cst_id);
    debug!("Target CST: {target_cst:?}");

    Some(LocationLink {
        origin_selection_range: Some(db.range_to_lsp_range(module.clone(), origin_cst.data.span)),
        target_uri: module_to_url(&module, &db.packages_path).unwrap(),
        target_range: db.range_to_lsp_range(module.clone(), target_cst.data.span.clone()),
        target_selection_range: db.range_to_lsp_range(module, target_cst.display_span()),
    })
}
