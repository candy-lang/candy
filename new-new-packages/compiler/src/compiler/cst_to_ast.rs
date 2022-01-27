use im::HashMap;

use super::ast::{self, Ast, AstKind, AstString, Int, Lambda, Symbol, Text};
use super::cst::{self, Cst, CstKind};
use super::error::CompilerError;

pub trait LowerCstToAst {
    fn compile_into_ast(self) -> (Vec<Ast>, HashMap<ast::Id, cst::Id>, Vec<CompilerError>);
}
impl LowerCstToAst for Vec<Cst> {
    fn compile_into_ast(self) -> (Vec<Ast>, HashMap<ast::Id, cst::Id>, Vec<CompilerError>) {
        let mut context = LoweringContext::new();
        let asts = (&mut context).lower_csts(self);
        (asts, context.id_mapping, context.errors)
    }
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
    fn lower_csts(&mut self, csts: Vec<Cst>) -> Vec<Ast> {
        csts.into_iter().map(|it| self.lower_cst(it)).collect()
    }
    fn lower_cst(&mut self, cst: Cst) -> Ast {
        match cst.kind {
            CstKind::EqualsSign { .. } => self.create_ast(cst.id, AstKind::Error),
            CstKind::OpeningParenthesis { .. } => self.create_ast(cst.id, AstKind::Error),
            CstKind::ClosingParenthesis { .. } => self.create_ast(cst.id, AstKind::Error),
            CstKind::OpeningCurlyBrace { .. } => self.create_ast(cst.id, AstKind::Error),
            CstKind::ClosingCurlyBrace { .. } => self.create_ast(cst.id, AstKind::Error),
            CstKind::Arrow { .. } => self.create_ast(cst.id, AstKind::Error),
            CstKind::Int { value, .. } => self.create_ast(cst.id, AstKind::Int(Int(value))),
            CstKind::Text { value, .. } => {
                let string = self.create_string(cst.id, value);
                self.create_ast(cst.id, AstKind::Text(Text(string)))
            }
            CstKind::Identifier { .. } => {
                panic!("Tried to lower an identifier from CST to AST.")
            }
            CstKind::Symbol { value, .. } => {
                let string = self.create_string(cst.id, value);
                self.create_ast(cst.id, AstKind::Symbol(Symbol(string)))
            }
            CstKind::LeadingWhitespace { child, .. } => self.lower_cst(*child),
            CstKind::LeadingComment { child, .. } => self.lower_cst(*child),
            CstKind::TrailingWhitespace { child, .. } => self.lower_cst(*child),
            CstKind::TrailingComment { child, .. } => self.lower_cst(*child),
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
                self.lower_cst(*inner)
            }
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
                let name = self.lower_identifier(*name);

                let parameters = self.lower_parameters(parameters);
                assert!(
                    matches!(
                        equals_sign.unwrap_whitespace_and_comment().kind,
                        CstKind::EqualsSign { .. }
                    ),
                    "Expected an equals sign for the assignment."
                );

                let body = self.lower_csts(body);

                self.create_ast(
                    cst.id,
                    AstKind::Assignment(ast::Assignment {
                        name,
                        parameters,
                        body,
                    }),
                )
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

    fn lower_parameters(&mut self, csts: Vec<Cst>) -> Vec<AstString> {
        csts.into_iter()
            .filter_map(|it| self.lower_parameter(it))
            .collect()
    }
    fn lower_parameter(&mut self, cst: Cst) -> Option<AstString> {
        let cst = cst.unwrap_whitespace_and_comment();
        match cst.kind.clone() {
            CstKind::Call { name, arguments } => {
                let name = self.lower_identifier(*name);

                if !arguments.is_empty() {
                    self.errors.push(CompilerError {
                        span: cst.span(),
                        message: "Parameters can't have parameters themselves.".to_owned(),
                    });
                }
                Some(name)
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
    fn lower_identifier(&mut self, cst: Cst) -> AstString {
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
    fn create_next_id(&mut self, cst_id: cst::Id) -> ast::Id {
        let id = ast::Id(self.next_id);
        assert!(matches!(self.id_mapping.insert(id, cst_id), None));
        self.next_id += 1;
        id
    }
}
