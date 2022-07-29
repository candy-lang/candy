use super::utils::{LspPositionConversion, TupleToPosition};
use crate::{
    compiler::{
        ast_to_hir::AstToHir,
        cst::{CstDb, CstKind},
        hir::{self, Body, Expression, HirDb, Lambda},
    },
    database::Database,
    input::Input,
};
use im::HashSet;
use lsp_types::{
    DocumentHighlight, DocumentHighlightKind, DocumentHighlightParams, Location, ReferenceParams,
    TextDocumentPositionParams,
};

pub fn find_references(db: &Database, params: ReferenceParams) -> Option<Vec<Location>> {
    let position = params.text_document_position;
    let references = find(db, position.clone(), params.context.include_declaration)?
        .into_iter()
        .map(|it| Location {
            uri: position.text_document.uri.clone(),
            range: it.range,
        })
        .collect();
    Some(references)
}

pub fn find_document_highlights(
    db: &Database,
    params: DocumentHighlightParams,
) -> Option<Vec<DocumentHighlight>> {
    find(db, params.text_document_position_params, true)
}

fn find(
    db: &Database,
    params: TextDocumentPositionParams,
    include_declaration: bool,
) -> Option<Vec<DocumentHighlight>> {
    let input: Input = params.text_document.uri.clone().into();
    let position = params.position;
    let offset = db.offset_from_lsp(input.clone(), position.line, position.character);
    let query = query_for_offset(db, input, offset)?;
    Some(db.references(query, include_declaration))
}

fn query_for_offset(db: &Database, input: Input, offset: usize) -> Option<ReferenceQuery> {
    let origin_cst = db.find_cst_by_offset(input.clone(), offset);
    match origin_cst.kind {
        CstKind::Identifier(identifier) if identifier == "needs" => {
            Some(ReferenceQuery::Needs(input))
        }
        CstKind::Identifier { .. } => {
            let hir_id = db.cst_to_hir_id(input, origin_cst.id)?;
            let target_id = if let Some(hir_expr) = db.find_expression(hir_id.clone()) {
                let containing_body = db.containing_body_of(hir_id.clone());
                if containing_body.identifiers.contains_key(&hir_id) {
                    // A local variable was declared. Find references to that variable.
                    hir_id
                } else {
                    // An intermediate reference. Find references to it's target.
                    match hir_expr {
                        Expression::Reference(target_id) => target_id,
                        Expression::Symbol(_) => {
                            // TODO: Handle struct access
                            return None;
                        }
                        _ => panic!("Expected a reference, got {:?}", hir_expr),
                    }
                }
            } else {
                // Parameter
                hir_id
            };
            Some(ReferenceQuery::Id(target_id))
        }
        CstKind::Symbol(symbol) => Some(ReferenceQuery::Symbol(input, symbol)),
        _ => None,
    }
}

#[salsa::query_group(ReferencesDbStorage)]
pub trait ReferencesDb: HirDb + LspPositionConversion {
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
    let input = match &query {
        ReferenceQuery::Id(id) => id.input.clone(),
        ReferenceQuery::Symbol(input, _) => input.to_owned(),
        ReferenceQuery::Needs(input) => input.to_owned(),
    };
    let (hir, _) = db.hir(input).unwrap();

    let mut context = Context::new(db, query, include_declaration);
    context.visit_body(hir.as_ref());
    context.references
}

struct Context<'a> {
    db: &'a dyn ReferencesDb,
    query: ReferenceQuery,
    include_declaration: bool,
    discovered_references: HashSet<hir::Id>,
    references: Vec<DocumentHighlight>,
}
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ReferenceQuery {
    Id(hir::Id),
    Symbol(Input, String),
    Needs(Input),
}
impl<'a> Context<'a> {
    fn new(db: &'a dyn ReferencesDb, query: ReferenceQuery, include_declaration: bool) -> Self {
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
            Expression::Int(_) => {}
            Expression::Text(_) => {}
            Expression::Reference(target) => {
                if let ReferenceQuery::Id(target_id) = &self.query {
                    if target == target_id {
                        self.add_reference(id, DocumentHighlightKind::READ);
                    }
                }
            }
            Expression::Symbol(symbol) => {
                if let ReferenceQuery::Symbol(_, target) = &self.query {
                    if symbol == target {
                        self.add_reference(id, DocumentHighlightKind::READ);
                    }
                }
            }
            Expression::Struct(entries) => {
                for (key_id, value_id) in entries {
                    self.visit_id(key_id.to_owned());
                    self.visit_id(value_id.to_owned());
                }
            }
            Expression::Lambda(Lambda { body, .. }) => {
                // We don't need to visit the parameters: They can only be the
                // declaration of an identifier and don't reference it any other
                // way. Therfore, we already visit them in [visit_body].
                self.visit_body(body);
            }
            Expression::Call {
                function,
                arguments,
            } => {
                if let ReferenceQuery::Id(target_id) = &self.query {
                    if function == target_id {
                        self.add_reference(id, DocumentHighlightKind::READ);
                    }
                }
                self.visit_ids(arguments);
            }
            Expression::Builtin(_) => {}
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
                range: lsp_types::Range {
                    start: self
                        .db
                        .offset_to_lsp(id.input.clone(), span.start)
                        .to_position(),
                    end: self.db.offset_to_lsp(id.input, span.end).to_position(),
                },
                kind: Some(kind),
            });
        }
    }
}
