use std::ops::Range;
use std::sync::Arc;

use im::HashMap;

use super::ast::{self, Ast, AstKind, AstString, Identifier, Int, Lambda, Symbol, Text};
use super::cst::{self, Cst, CstDb, CstKind};
use super::error::CompilerError;
use super::string_to_cst::StringToCst;
use crate::compiler::ast::Struct;
use crate::input::Input;

#[salsa::query_group(CstToAstStorage)]
pub trait CstToAst: CstDb + StringToCst {
    fn ast_to_cst_id(&self, input: Input, id: ast::Id) -> Option<cst::Id>;
    fn ast_id_to_span(&self, input: Input, id: ast::Id) -> Option<Range<usize>>;

    fn cst_to_ast_id(&self, input: Input, id: cst::Id) -> Option<ast::Id>;

    fn ast(&self, input: Input) -> Option<(Arc<Vec<Ast>>, HashMap<ast::Id, cst::Id>)>;
    fn ast_raw(
        &self,
        input: Input,
    ) -> Option<(Arc<Vec<Ast>>, HashMap<ast::Id, cst::Id>, Vec<CompilerError>)>;
}

fn ast_to_cst_id(db: &dyn CstToAst, input: Input, id: ast::Id) -> Option<cst::Id> {
    let (_, ast_to_cst_id_mapping) = db.ast(input).unwrap();
    ast_to_cst_id_mapping.get(&id).cloned()
}
fn ast_id_to_span(db: &dyn CstToAst, input: Input, id: ast::Id) -> Option<Range<usize>> {
    let id = db.ast_to_cst_id(input.clone(), id)?;
    Some(db.find_cst(input, id).span())
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
    let cst = db.cst(input)?;
    let mut context = LoweringContext::new();
    let asts = (&mut context).lower_csts(&cst);
    Some((Arc::new(asts), context.id_mapping, context.errors))
}

struct LoweringContext {
    next_id: usize,
    id_mapping: HashMap<ast::Id, cst::Id>,
    errors: Vec<CompilerError>,
}
impl LoweringContext {
    fn new() -> LoweringContext {
        LoweringContext {
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
            CstKind::EqualsSign { .. } => self.create_ast(cst.id, AstKind::Error),
            CstKind::Colon { .. } => self.create_ast(cst.id, AstKind::Error),
            CstKind::Comma { .. } => self.create_ast(cst.id, AstKind::Error),
            CstKind::OpeningParenthesis { .. } => self.create_ast(cst.id, AstKind::Error),
            CstKind::ClosingParenthesis { .. } => self.create_ast(cst.id, AstKind::Error),
            CstKind::OpeningBracket { .. } => self.create_ast(cst.id, AstKind::Error),
            CstKind::ClosingBracket { .. } => self.create_ast(cst.id, AstKind::Error),
            CstKind::OpeningCurlyBrace { .. } => self.create_ast(cst.id, AstKind::Error),
            CstKind::ClosingCurlyBrace { .. } => self.create_ast(cst.id, AstKind::Error),
            CstKind::Arrow { .. } => self.create_ast(cst.id, AstKind::Error),
            CstKind::Int { value, .. } => {
                self.create_ast(cst.id, AstKind::Int(Int(value.to_owned())))
            }
            CstKind::Text { value, .. } => {
                let string = self.create_string_without_id_mapping(value.to_owned());
                self.create_ast(cst.id, AstKind::Text(Text(string)))
            }
            CstKind::Identifier { value, .. } => {
                let string = self.create_string_without_id_mapping(value.to_owned());
                self.create_ast(cst.id, AstKind::Identifier(Identifier(string)))
            }
            CstKind::Symbol { value, .. } => {
                let string = self.create_string_without_id_mapping(value.to_owned());
                self.create_ast(cst.id, AstKind::Symbol(Symbol(string)))
            }
            CstKind::LeadingWhitespace { child, .. } => self.lower_cst(child),
            CstKind::LeadingComment { child, .. } => self.lower_cst(child),
            CstKind::TrailingWhitespace { child, .. } => self.lower_cst(child),
            CstKind::TrailingComment { child, .. } => self.lower_cst(child),
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
            CstKind::Struct {
                opening_bracket,
                entries,
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
                let entries = entries
                    .into_iter()
                    .map(|entry| {
                        let entry = entry.unwrap_whitespace_and_comment();
                        if let CstKind::StructEntry {
                            key,
                            colon: _,
                            value,
                            comma: _,
                        } = &entry.kind
                        {
                            if key.is_none() {
                                self.errors.push(CompilerError {
                                    span: cst.span(),
                                    message: format!("Expected key, found `{}`.", cst),
                                });
                                return (
                                    self.create_ast(cst.id, AstKind::Error),
                                    self.create_ast(cst.id, AstKind::Error),
                                );
                            }

                            if value.is_none() {
                                self.errors.push(CompilerError {
                                    span: cst.span(),
                                    message: format!("Expected value, found `{}`.", cst),
                                });
                                return (
                                    self.create_ast(cst.id, AstKind::Error),
                                    self.create_ast(cst.id, AstKind::Error),
                                );
                            }
                            let key = self.lower_cst(&key.clone().unwrap());
                            let value = self.lower_cst(&value.clone().unwrap());
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
                self.create_ast(cst.id, AstKind::Struct(Struct { entries }))
            }
            CstKind::StructEntry { .. } => self.create_ast(cst.id, AstKind::Error),
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
            CstKind::Call { name, arguments } => {
                let name = name.unwrap_whitespace_and_comment();
                let name = match name {
                    Cst {
                        id,
                        kind: CstKind::Identifier { value, .. },
                    } => self.create_string(id.to_owned(), value.to_owned()),
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
            CstKind::Error { ref message, .. } => {
                self.errors.push(CompilerError {
                    span: cst.span(),
                    message: message.to_owned(),
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
            CstKind::Identifier { value, .. } => {
                Some(self.create_string(cst.id.to_owned(), value.clone()))
            }
            _ => {
                self.errors.push(CompilerError {
                    span: cst.span(),
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
                kind: CstKind::Identifier { value, .. },
            } => self.create_string(id.to_owned(), value.clone()),
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
        assert!(matches!(self.id_mapping.insert(id, cst_id), None));
        id
    }
    fn create_next_id_without_mapping(&mut self) -> ast::Id {
        let id = ast::Id(self.next_id);
        self.next_id += 1;
        id
    }
}
