use candy_frontend::{
    cst::{CstDb, CstKind},
    hir::{self, Body, Expression, HirDb, Lambda},
    module::{Module, ModuleDb, ModuleKind},
    position::PositionConversionDb,
};
use lsp_types::{
    DocumentHighlight, DocumentHighlightKind, DocumentHighlightParams, Location, ReferenceParams,
    TextDocumentPositionParams,
};
use std::{collections::HashSet, path::PathBuf};
use tracing::{debug, info};

use crate::utils::{module_from_package_root_and_url, LspPositionConversion};

pub fn find_references<DB>(
    db: &DB,
    project_directory: PathBuf,
    params: ReferenceParams,
) -> Option<Vec<Location>>
where
    DB: HirDb + ModuleDb + PositionConversionDb + ReferencesDb,
{
    let position = params.text_document_position;
    let references = find(
        db,
        project_directory,
        position.clone(),
        params.context.include_declaration,
    )?
    .into_iter()
    .map(|it| Location {
        uri: position.text_document.uri.clone(),
        range: it.range,
    })
    .collect();
    Some(references)
}

pub fn find_document_highlights<DB>(
    db: &DB,
    project_directory: PathBuf,
    params: DocumentHighlightParams,
) -> Option<Vec<DocumentHighlight>>
where
    DB: HirDb + ModuleDb + PositionConversionDb + ReferencesDb,
{
    find(
        db,
        project_directory,
        params.text_document_position_params,
        true,
    )
}

fn find<DB>(
    db: &DB,
    project_directory: PathBuf,
    params: TextDocumentPositionParams,
    include_declaration: bool,
) -> Option<Vec<DocumentHighlight>>
where
    DB: HirDb + ModuleDb + PositionConversionDb + ReferencesDb,
{
    let module = module_from_package_root_and_url(
        project_directory,
        params.text_document.uri,
        ModuleKind::Code,
    );
    let position = params.position;
    let offset = db.lsp_position_to_offset(module.clone(), position);
    let query = query_for_offset(db, module, offset)?;
    Some(db.references(query, include_declaration))
}

fn query_for_offset<DB: CstDb>(db: &DB, module: Module, offset: usize) -> Option<ReferenceQuery>
where
    DB: HirDb,
{
    let origin_cst = db.find_cst_by_offset(module.clone(), offset);
    info!("Finding references for {origin_cst:?}");
    let query = match origin_cst.kind {
        CstKind::Identifier(identifier) if identifier == "needs" => {
            Some(ReferenceQuery::Needs(module))
        }
        CstKind::Identifier { .. } => {
            let hir_id = db.cst_to_hir_id(module, origin_cst.id)?;
            let target_id = if let Some(hir_expr) = db.find_expression(hir_id.clone()) {
                let containing_body = db.containing_body_of(hir_id.clone());
                if containing_body.identifiers.contains_key(&hir_id) {
                    // A local variable was declared. Find references to that variable.
                    hir_id
                } else {
                    // An intermediate reference. Find references to its target.
                    match hir_expr {
                        Expression::Reference(target_id) => target_id,
                        Expression::Symbol(_) => {
                            // TODO: Handle struct access
                            return None;
                        }
                        Expression::Error { .. } => return None,
                        _ => panic!("Expected a reference, got {hir_expr}."),
                    }
                }
            } else {
                // Parameter
                hir_id
            };
            Some(ReferenceQuery::Id(target_id))
        }
        CstKind::Symbol(symbol) => Some(ReferenceQuery::Symbol(module, symbol)),
        _ => None,
    };
    debug!("Reference query: {query:?}");
    query
}

#[salsa::query_group(ReferencesDbStorage)]
pub trait ReferencesDb: HirDb + ModuleDb + PositionConversionDb {
    fn references(
        &self,
        query: ReferenceQuery,
        include_declaration: bool,
    ) -> Vec<DocumentHighlight>;
}

fn references(
    db: &dyn ReferencesDb,
    query: ReferenceQuery,
    include_declaration: bool,
) -> Vec<DocumentHighlight> {
    // TODO: search all files
    let module = match &query {
        ReferenceQuery::Id(id) => id.module.clone(),
        ReferenceQuery::Symbol(module, _) => module.to_owned(),
        ReferenceQuery::Needs(module) => module.to_owned(),
    };
    let (hir, _) = db.hir(module).unwrap();

    let mut context = Context::new(db, query, include_declaration);
    context.visit_body(hir.as_ref());
    context.references
}

struct Context<'a, DB: PositionConversionDb + ReferencesDb + ?Sized> {
    db: &'a DB,
    query: ReferenceQuery,
    include_declaration: bool,
    discovered_references: HashSet<hir::Id>,
    references: Vec<DocumentHighlight>,
}
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ReferenceQuery {
    Id(hir::Id),
    Symbol(Module, String),
    Needs(Module),
}
impl<'a, DB> Context<'a, DB>
where
    DB: PositionConversionDb + ReferencesDb + ?Sized,
{
    fn new(db: &'a DB, query: ReferenceQuery, include_declaration: bool) -> Self {
        Self {
            db,
            query,
            include_declaration,
            discovered_references: HashSet::new(),
            references: vec![],
        }
    }

    fn visit_body(&mut self, body: &Body) {
        if let ReferenceQuery::Id(id) = &self.query.clone() {
            if body.identifiers.contains_key(id) {
                self.add_reference(id.clone(), DocumentHighlightKind::WRITE);
            }
        }
        for (id, expression) in &body.expressions {
            self.visit_expression(id.to_owned(), expression);
        }
    }
    fn visit_ids(&mut self, ids: &[hir::Id]) {
        for id in ids {
            self.visit_id(id.to_owned());
        }
    }
    fn visit_id(&mut self, id: hir::Id) {
        let expression = match self.db.find_expression(id.to_owned()) {
            Some(expression) => expression,
            None => return, // Generated code
        };
        self.visit_expression(id, &expression);
    }
    fn visit_expression(&mut self, id: hir::Id, expression: &Expression) {
        match expression {
            Expression::Int(_) | Expression::Text(_) => {}
            Expression::Reference(target) => {
                if let ReferenceQuery::Id(target_id) = &self.query && target == target_id {
                    self.add_reference(id, DocumentHighlightKind::READ);
                }
            }
            Expression::Symbol(symbol) => {
                if let ReferenceQuery::Symbol(_, target) = &self.query && symbol == target {
                    self.add_reference(id, DocumentHighlightKind::READ);
                }
            }
            Expression::List(_)
            | Expression::Struct(_)
            | Expression::Destructure { .. }
            | Expression::PatternIdentifierReference (_) => {},
            Expression::Match { cases, .. } => {
                for (_, body) in cases {
                    self.visit_body(body);
                }
            },
            Expression::Lambda(Lambda { body, .. }) => {
                // We don't need to visit the parameters: They can only be the
                // declaration of an identifier and don't reference it any other
                // way. Therfore, we already visit them in [visit_body].
                self.visit_body(body);
            }
            Expression::Builtin(_) => {}
            Expression::Call {
                function,
                arguments,
            } => {
                if let ReferenceQuery::Id(target_id) = &self.query && function == target_id {
                    self.add_reference(id, DocumentHighlightKind::READ);
                }
                self.visit_ids(arguments);
            }
            Expression::UseModule { .. } => {} // only occurs in generated code
            Expression::Needs { .. } => {
                if let ReferenceQuery::Needs(_) = &self.query {
                    self.add_reference(id, DocumentHighlightKind::READ);
                }
            }
            Expression::Error { child, .. } => {
                if let Some(child) = child {
                    self.visit_id(child.clone());
                }
            }
        }
    }

    fn add_reference(&mut self, id: hir::Id, kind: DocumentHighlightKind) {
        if let ReferenceQuery::Id(target_id) = &self.query {
            if &id == target_id && !self.include_declaration {
                return;
            }
        }

        if self.discovered_references.contains(&id) {
            return;
        }
        self.discovered_references.insert(id.clone());

        if let Some(span) = self.db.hir_id_to_span(id.clone()) {
            self.references.push(DocumentHighlight {
                range: self.db.range_to_lsp_range(id.module, span),
                kind: Some(kind),
            });
        }
    }
}
