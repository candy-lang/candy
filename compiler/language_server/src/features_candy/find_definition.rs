use candy_frontend::{
    ast_to_hir::AstToHir,
    cst::{CstDb, CstKind},
    hir::{Expression, HirDb},
    module::{Module, ModuleDb},
    position::{Offset, PositionConversionDb},
    rcst_to_cst::RcstToCst,
};
use lsp_types::LocationLink;

use crate::utils::{module_to_url, LspPositionConversion};

pub fn find_definition<DB>(db: &DB, module: Module, offset: Offset) -> Option<LocationLink>
where
    DB: AstToHir + CstDb + HirDb + ModuleDb + PositionConversionDb + RcstToCst,
{
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

    Some(LocationLink {
        origin_selection_range: Some(db.range_to_lsp_range(module.clone(), origin_cst.span)),
        target_uri: module_to_url(&module).unwrap(),
        target_range: db.range_to_lsp_range(module.clone(), target_cst.span.clone()),
        target_selection_range: db.range_to_lsp_range(module, target_cst.display_span()),
    })
}
