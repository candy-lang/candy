use im::HashSet;
use lsp_types::{
    DocumentHighlight, DocumentHighlightKind, DocumentHighlightParams, Location, ReferenceParams,
    TextDocumentPositionParams,
};

use crate::{
    compiler::{
        ast_to_hir::AstToHir,
        cst::{CstDb, CstKind},
        hir::{self, Body, Expression, HirDb, Lambda},
    },
    database::Database,
    input::Input,
};

use super::utils::{LspPositionConversion, TupleToPosition};

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

    let origin_cst = db.find_cst_by_offset(input.clone(), offset);
    match origin_cst.kind {
        CstKind::Identifier { .. } => {}
        _ => return None,
    }

    let origin_hir_id = db.cst_to_hir_id(input.clone(), origin_cst.id)?;
    Some(db.references(input, origin_hir_id, include_declaration))
}

#[salsa::query_group(ReferencesDbStorage)]
pub trait ReferencesDb: HirDb + LspPositionConversion {
    fn references(
        &self,
        input: Input,
        id: hir::Id,
        include_declaration: bool,
    ) -> Vec<DocumentHighlight>;
}

fn references(
    db: &dyn ReferencesDb,
    input: Input,
    id: hir::Id,
    include_declaration: bool,
) -> Vec<DocumentHighlight> {
    let (hir, _) = db.hir(input.clone()).unwrap();

    let mut context = Context::new(db, input, id, include_declaration);
    context.visit_body(hir.as_ref());
    context.references
}

struct Context<'a> {
    db: &'a dyn ReferencesDb,
    input: Input,
    id: hir::Id,
    include_declaration: bool,
    discovered_references: HashSet<hir::Id>,
    references: Vec<DocumentHighlight>,
}
impl<'a> Context<'a> {
    fn new(db: &'a dyn ReferencesDb, input: Input, id: hir::Id, include_declaration: bool) -> Self {
        Self {
            db,
            input,
            id,
            include_declaration,
            discovered_references: HashSet::new(),
            references: vec![],
        }
    }

    fn visit_body(&mut self, body: &Body) {
        if body.identifiers.contains_key(&self.id) {
            self.add_reference(self.id.clone(), DocumentHighlightKind::WRITE);
        }
        for (id, expression) in &body.expressions {
            self.visit_expression(id.to_owned(), expression);
        }
    }
    fn visit_expressions(&mut self, ids: &[hir::Id]) {
        for id in ids {
            self.visit_id(id.to_owned());
        }
    }
    fn visit_id(&mut self, id: hir::Id) {
        let expression = self
            .db
            .find_expression(self.input.clone(), id.to_owned())
            .unwrap();
        self.visit_expression(id, &expression);
    }
    fn visit_expression(&mut self, id: hir::Id, expression: &Expression) {
        match expression {
            Expression::Int(_) => {}
            Expression::Text(_) => {}
            Expression::Reference(target) => {
                if target == &self.id {
                    self.add_reference(id, DocumentHighlightKind::READ);
                }
            }
            Expression::Symbol(_) => {}
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
            Expression::Body(body) => {
                self.visit_body(body);
            }
            Expression::Call {
                function,
                arguments,
            } => {
                if function == &self.id {
                    self.add_reference(id, DocumentHighlightKind::READ);
                }
                self.visit_expressions(arguments);
            }
            Expression::Error => {}
        }
    }

    fn add_reference(&mut self, id: hir::Id, kind: DocumentHighlightKind) {
        if id == self.id && !self.include_declaration {
            return;
        }

        if self.discovered_references.contains(&id) {
            return;
        }
        self.discovered_references.insert(id.clone());

        let span = self.db.hir_id_to_span(self.input.clone(), id).unwrap();
        self.references.push(DocumentHighlight {
            range: lsp_types::Range {
                start: self
                    .db
                    .offset_to_lsp(self.input.clone(), span.start)
                    .to_position(),
                end: self
                    .db
                    .offset_to_lsp(self.input.clone(), span.end)
                    .to_position(),
            },
            kind: Some(kind),
        });
    }
}
