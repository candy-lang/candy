use std::ops::Range;
use std::sync::Arc;

use im::HashMap;
use itertools::Itertools;

use super::ast::{self, Ast, AstKind, AstString, Identifier, Int, Lambda, Symbol, Text};
use super::cst::{self, Cst, CstDb, CstKind};
use super::error::CompilerError;
use super::rcst_to_cst::RcstToCst;
use crate::compiler::ast::Struct;
use crate::compiler::cst::UnwrapWhitespaceAndComment;
use crate::input::Input;

#[salsa::query_group(CstToAstStorage)]
pub trait CstToAst: CstDb + RcstToCst {
    fn ast_to_cst_id(&self, id: ast::Id) -> Option<cst::Id>;
    fn ast_id_to_span(&self, id: ast::Id) -> Option<Range<usize>>;

    fn cst_to_ast_id(&self, input: Input, id: cst::Id) -> Option<ast::Id>;

    fn ast(&self, input: Input) -> Option<(Arc<Vec<Ast>>, HashMap<ast::Id, cst::Id>)>;
    fn ast_raw(
        &self,
        input: Input,
    ) -> Option<(Arc<Vec<Ast>>, HashMap<ast::Id, cst::Id>, Vec<CompilerError>)>;
}

fn ast_to_cst_id(db: &dyn CstToAst, id: ast::Id) -> Option<cst::Id> {
    let (_, ast_to_cst_id_mapping) = db.ast(id.input.clone()).unwrap();
    ast_to_cst_id_mapping.get(&id).cloned()
}
fn ast_id_to_span(db: &dyn CstToAst, id: ast::Id) -> Option<Range<usize>> {
    let cst_id = db.ast_to_cst_id(id.clone())?;
    Some(db.find_cst(id.input, cst_id).span)
}

fn cst_to_ast_id(db: &dyn CstToAst, input: Input, id: cst::Id) -> Option<ast::Id> {
    let (_, ast_to_cst_id_mapping) = db.ast(input).unwrap();
    ast_to_cst_id_mapping
        .iter()
        .find_map(|(key, &value)| if value == id { Some(key) } else { None })
        .cloned()
}

fn ast(db: &dyn CstToAst, input: Input) -> Option<(Arc<Vec<Ast>>, HashMap<ast::Id, cst::Id>)> {
    db.ast_raw(input)
        .map(|(ast, id_mapping, _)| (ast, id_mapping))
}
fn ast_raw(
    db: &dyn CstToAst,
    input: Input,
) -> Option<(Arc<Vec<Ast>>, HashMap<ast::Id, cst::Id>, Vec<CompilerError>)> {
    let cst = db.cst(input.clone())?;
    let cst = cst.unwrap_whitespace_and_comment();
    let mut context = LoweringContext::new(input);
    let asts = (&mut context).lower_csts(&cst);
    Some((Arc::new(asts), context.id_mapping, context.errors))
}

struct LoweringContext {
    input: Input,
    next_id: usize,
    id_mapping: HashMap<ast::Id, cst::Id>,
    errors: Vec<CompilerError>,
}
impl LoweringContext {
    fn new(input: Input) -> LoweringContext {
        LoweringContext {
            input,
            next_id: 0,
            id_mapping: HashMap::new(),
            errors: vec![],
        }
    }
    fn lower_csts(&mut self, csts: &[Cst]) -> Vec<Ast> {
        csts.iter().map(|it| self.lower_cst(it)).collect()
    }
    fn lower_cst(&mut self, cst: &Cst) -> Ast {
        match &cst.kind {
            CstKind::EqualsSign => self.create_ast(cst.id, AstKind::Error),
            CstKind::Comma => self.create_ast(cst.id, AstKind::Error),
            CstKind::Colon => self.create_ast(cst.id, AstKind::Error),
            CstKind::OpeningParenthesis => self.create_ast(cst.id, AstKind::Error),
            CstKind::ClosingParenthesis => self.create_ast(cst.id, AstKind::Error),
            CstKind::OpeningBracket => self.create_ast(cst.id, AstKind::Error),
            CstKind::ClosingBracket => self.create_ast(cst.id, AstKind::Error),
            CstKind::OpeningCurlyBrace => self.create_ast(cst.id, AstKind::Error),
            CstKind::ClosingCurlyBrace => self.create_ast(cst.id, AstKind::Error),
            CstKind::Arrow => self.create_ast(cst.id, AstKind::Error),
            CstKind::DoubleQuote => self.create_ast(cst.id, AstKind::Error),
            CstKind::Octothorpe => self.create_ast(cst.id, AstKind::Error),
            CstKind::Whitespace(_) => self.create_ast(cst.id, AstKind::Error),
            CstKind::Newline => self.create_ast(cst.id, AstKind::Error),
            CstKind::Comment { .. } => self.create_ast(cst.id, AstKind::Error),
            CstKind::TrailingWhitespace { child, .. } => self.lower_cst(child),
            CstKind::Identifier(identifier) => {
                let string = self.create_string_without_id_mapping(identifier.to_string());
                self.create_ast(cst.id, AstKind::Identifier(Identifier(string)))
            }
            CstKind::Symbol(symbol) => {
                let string = self.create_string_without_id_mapping(symbol.to_string());
                self.create_ast(cst.id, AstKind::Symbol(Symbol(string)))
            }
            CstKind::Int(value) => self.create_ast(cst.id, AstKind::Int(Int(*value))),
            CstKind::Text { parts, .. } => {
                let text = parts
                    .into_iter()
                    .filter_map(|it| match it {
                        Cst {
                            kind: CstKind::TextPart(text),
                            ..
                        } => Some(text),
                        _ => None,
                    })
                    .join("");
                let string = self.create_string_without_id_mapping(text);
                self.create_ast(cst.id, AstKind::Text(Text(string)))
            }
            CstKind::TextPart(_) => self.create_ast(cst.id, AstKind::Error),
            CstKind::Parenthesized {
                opening_parenthesis,
                inner,
                closing_parenthesis,
            } => {
                assert!(
                matches!(opening_parenthesis.unwrap_whitespace_and_comment().kind, CstKind::OpeningParenthesis { .. }),
                "Expected an opening parenthesis to start a parenthesized expression, but found `{}`.",
                *opening_parenthesis
            );
                assert!(
                matches!(
                    closing_parenthesis.unwrap_whitespace_and_comment().kind,
                    CstKind::ClosingParenthesis { .. }
                ),
                "Expected a closing parenthesis to end a parenthesized expression, but found `{}`.",
                *closing_parenthesis
            );
                self.lower_cst(inner)
            }
            CstKind::Call { name, arguments } => {
                let name = name.unwrap_whitespace_and_comment();
                let name = match name {
                    Cst {
                        id,
                        kind: CstKind::Identifier(identifier),
                        ..
                    } => self.create_string(id.to_owned(), identifier.to_owned()),
                    _ => {
                        panic!(
                            "Expected a symbol for the name of a call, but found `{}`.",
                            name
                        );
                    }
                };

                let arguments = self.lower_csts(arguments);
                self.create_ast(cst.id, AstKind::Call(ast::Call { name, arguments }))
            }
            CstKind::Struct {
                opening_bracket,
                fields,
                closing_bracket: _,
            } => {
                assert!(
                    matches!(
                        opening_bracket.unwrap_whitespace_and_comment().kind,
                        CstKind::OpeningBracket { .. }
                    ),
                    "Expected an opening bracket to start a struct, but found `{}`.",
                    *opening_bracket
                );
                let fields = fields
                    .into_iter()
                    .map(|field| {
                        let field = field.unwrap_whitespace_and_comment();
                        if let CstKind::StructField {
                            key,
                            colon: _,
                            value,
                            comma: _,
                        } = &field.kind
                        {
                            let key = self.lower_cst(&key.clone());
                            let value = self.lower_cst(&value.clone());
                            (key, value)
                        } else {
                            // TODO: Register error.
                            return (
                                self.create_ast(cst.id, AstKind::Error),
                                self.create_ast(cst.id, AstKind::Error),
                            );
                        }
                    })
                    .collect();
                self.create_ast(cst.id, AstKind::Struct(Struct { fields }))
            }
            CstKind::StructField { .. } => self.create_ast(cst.id, AstKind::Error),
            CstKind::Lambda {
                opening_curly_brace,
                parameters_and_arrow,
                body,
                closing_curly_brace,
            } => {
                assert!(
                    matches!(
                        opening_curly_brace.unwrap_whitespace_and_comment().kind,
                        CstKind::OpeningCurlyBrace { .. }
                    ),
                    "Expected an opening curly brace at the beginning of a lambda, but found {}.",
                    opening_curly_brace,
                );
                let parameters = if let Some((parameters, arrow)) = parameters_and_arrow {
                    let parameters = self.lower_parameters(parameters);

                    assert!(
                        matches!(
                            arrow.unwrap_whitespace_and_comment().kind,
                            CstKind::Arrow { .. }
                        ),
                        "Expected an arrow after the parameters in a lambda, but found `{}`.",
                        arrow
                    );

                    parameters
                } else {
                    vec![]
                };

                let body = self.lower_csts(body);

                assert!(
                    matches!(
                        closing_curly_brace.unwrap_whitespace_and_comment().kind,
                        CstKind::ClosingCurlyBrace { .. }
                    ),
                    "Expected a closing curly brace at the end of a lambda, but found {}.",
                    closing_curly_brace
                );

                self.create_ast(cst.id, AstKind::Lambda(Lambda { parameters, body }))
            }
            CstKind::Assignment {
                name,
                parameters,
                equals_sign,
                body,
            } => {
                let name = self.lower_identifier(name);

                let parameters = self.lower_parameters(parameters);
                assert!(
                    matches!(
                        equals_sign.unwrap_whitespace_and_comment().kind,
                        CstKind::EqualsSign { .. }
                    ),
                    "Expected an equals sign for the assignment."
                );

                let mut body = self.lower_csts(body);

                if !parameters.is_empty() {
                    body =
                        vec![self.create_ast(cst.id, AstKind::Lambda(Lambda { parameters, body }))];
                }

                self.create_ast(cst.id, AstKind::Assignment(ast::Assignment { name, body }))
            }
            CstKind::Error { error, .. } => {
                self.errors.push(CompilerError {
                    span: cst.span.clone(),
                    // TODO: make this beautiful
                    message: format!("{:?}", error),
                });
                self.create_ast(cst.id, AstKind::Error)
            }
        }
    }

    fn lower_parameters(&mut self, csts: &[Cst]) -> Vec<AstString> {
        csts.into_iter()
            .enumerate()
            .map(|(index, it)| {
                self.lower_parameter(it)
                    .unwrap_or_else(|| self.create_string(it.id, format!("<invalid#{}", index)))
            })
            .collect()
    }
    fn lower_parameter(&mut self, cst: &Cst) -> Option<AstString> {
        let cst = cst.unwrap_whitespace_and_comment();
        match &cst.kind {
            CstKind::Identifier(identifier) => {
                Some(self.create_string(cst.id.to_owned(), identifier.clone()))
            }
            _ => {
                self.errors.push(CompilerError {
                    span: cst.span.clone(),
                    message: format!("Expected parameter, found `{}`.", cst),
                });
                None
            }
        }
    }
    fn lower_identifier(&mut self, cst: &Cst) -> AstString {
        let cst = cst.unwrap_whitespace_and_comment();
        match cst {
            Cst {
                id,
                kind: CstKind::Identifier(identifier),
                ..
            } => self.create_string(id.to_owned(), identifier.clone()),
            _ => {
                panic!("Expected an identifier, but found `{}`.", cst);
            }
        }
    }

    fn create_ast(&mut self, cst_id: cst::Id, kind: AstKind) -> Ast {
        Ast {
            id: self.create_next_id(cst_id),
            kind,
        }
    }
    fn create_string(&mut self, cst_id: cst::Id, value: String) -> AstString {
        AstString {
            id: self.create_next_id(cst_id),
            value,
        }
    }
    fn create_string_without_id_mapping(&mut self, value: String) -> AstString {
        AstString {
            id: self.create_next_id_without_mapping(),
            value,
        }
    }
    fn create_next_id(&mut self, cst_id: cst::Id) -> ast::Id {
        let id = self.create_next_id_without_mapping();
        assert!(matches!(self.id_mapping.insert(id.clone(), cst_id), None));
        id
    }
    fn create_next_id_without_mapping(&mut self) -> ast::Id {
        let id = ast::Id::new(self.input.clone(), self.next_id);
        self.next_id += 1;
        id
    }
}
