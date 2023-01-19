use itertools::Itertools;

use super::{
    ast::{
        self, Ast, AstError, AstKind, AstString, Identifier, Int, Lambda, Symbol, Text, TextPart,
    },
    cst::{self, Cst, CstDb, CstKind},
    error::{CompilerError, CompilerErrorPayload},
    rcst_to_cst::RcstToCst,
    string_to_rcst::InvalidModuleError,
    utils::AdjustCasingOfFirstLetter,
};
use crate::{
    compiler::{
        ast::{AssignmentBody, List, Struct},
        cst::UnwrapWhitespaceAndComment,
    },
    module::Module,
};
use std::{collections::HashMap, ops::Range, sync::Arc};
use tracing::warn;

#[salsa::query_group(CstToAstStorage)]
pub trait CstToAst: CstDb + RcstToCst {
    fn ast_to_cst_id(&self, id: ast::Id) -> Option<cst::Id>;
    fn ast_id_to_span(&self, id: ast::Id) -> Option<Range<usize>>;
    fn ast_id_to_display_span(&self, id: ast::Id) -> Option<Range<usize>>;

    fn cst_to_ast_id(&self, module: Module, id: cst::Id) -> Option<ast::Id>;

    fn ast(&self, module: Module) -> Option<AstResult>;
}

type AstResult = (Arc<Vec<Ast>>, Arc<HashMap<ast::Id, cst::Id>>);

fn ast_to_cst_id(db: &dyn CstToAst, id: ast::Id) -> Option<cst::Id> {
    let (_, ast_to_cst_id_mapping) = db.ast(id.module.clone()).unwrap();
    ast_to_cst_id_mapping.get(&id).cloned()
}
fn ast_id_to_span(db: &dyn CstToAst, id: ast::Id) -> Option<Range<usize>> {
    let cst_id = db.ast_to_cst_id(id.clone())?;
    Some(db.find_cst(id.module, cst_id).span)
}
fn ast_id_to_display_span(db: &dyn CstToAst, id: ast::Id) -> Option<Range<usize>> {
    let cst_id = db.ast_to_cst_id(id.clone())?;
    Some(db.find_cst(id.module, cst_id).display_span())
}

fn cst_to_ast_id(db: &dyn CstToAst, module: Module, id: cst::Id) -> Option<ast::Id> {
    let (_, ast_to_cst_id_mapping) = db.ast(module).unwrap();
    ast_to_cst_id_mapping
        .iter()
        .find_map(|(key, &value)| if value == id { Some(key) } else { None })
        .cloned()
}

fn ast(db: &dyn CstToAst, module: Module) -> Option<AstResult> {
    let mut context = LoweringContext::new(module.clone());

    let asts = match db.cst(module.clone()) {
        Ok(cst) => {
            let cst = cst.unwrap_whitespace_and_comment();
            context.lower_csts(&cst)
        }
        Err(InvalidModuleError::DoesNotExist) => {
            warn!("Tried to get AST of module that doesn't exist: {module}.");
            return None;
        }
        Err(InvalidModuleError::IsToolingModule) => {
            warn!("Tried to get AST of tooling module: {module}.");
            return None;
        }
        Err(InvalidModuleError::InvalidUtf8) => {
            vec![Ast {
                id: context.create_next_id_without_mapping(),
                kind: AstKind::Error {
                    child: None,
                    errors: vec![CompilerError {
                        module,
                        span: 0..0,
                        payload: CompilerErrorPayload::InvalidUtf8,
                    }],
                },
            }]
        }
    };

    Some((Arc::new(asts), Arc::new(context.id_mapping)))
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum LoweringType {
    Expression,
    Pattern,
    PatternLiteralPart,
}

struct LoweringContext {
    module: Module,
    next_id: usize,
    id_mapping: HashMap<ast::Id, cst::Id>,
}
impl LoweringContext {
    fn new(module: Module) -> LoweringContext {
        LoweringContext {
            module,
            next_id: 0,
            id_mapping: HashMap::new(),
        }
    }
    fn lower_csts(&mut self, csts: &[Cst]) -> Vec<Ast> {
        csts.iter()
            .map(|it| self.lower_cst(it, LoweringType::Expression))
            .collect()
    }
    fn lower_cst(&mut self, cst: &Cst, lowering_type: LoweringType) -> Ast {
        match &cst.kind {
            CstKind::EqualsSign
            | CstKind::Comma
            | CstKind::Dot
            | CstKind::Colon
            | CstKind::ColonEqualsSign
            | CstKind::Bar
            | CstKind::OpeningParenthesis
            | CstKind::ClosingParenthesis
            | CstKind::OpeningBracket
            | CstKind::ClosingBracket
            | CstKind::OpeningCurlyBrace
            | CstKind::ClosingCurlyBrace
            | CstKind::Arrow
            | CstKind::SingleQuote
            | CstKind::DoubleQuote
            | CstKind::Octothorpe => self.create_ast(
                cst.id,
                AstKind::Error {
                    child: None,
                    errors: vec![self.create_error(cst, AstError::UnexpectedPunctuation)],
                },
            ),
            CstKind::Whitespace(_)
            | CstKind::Newline(_)
            | CstKind::Comment { .. }
            | CstKind::TrailingWhitespace { .. } => {
                panic!("Whitespace should have been removed before lowering to AST.")
            }
            CstKind::Identifier(identifier) => {
                let string = self.create_string_without_id_mapping(identifier.to_string());
                let mut kind = AstKind::Identifier(Identifier(string));
                if lowering_type == LoweringType::PatternLiteralPart {
                    kind = AstKind::Error {
                        child: None,
                        errors: vec![self.create_error(
                            cst,
                            AstError::PatternLiteralPartContainsInvalidExpression,
                        )],
                    };
                };
                self.create_ast(cst.id, kind)
            }
            CstKind::Symbol(symbol) => {
                let string = self.create_string_without_id_mapping(symbol.to_string());
                self.create_ast(cst.id, AstKind::Symbol(Symbol(string)))
            }
            CstKind::Int { value, .. } => self.create_ast(cst.id, AstKind::Int(Int(value.clone()))),
            CstKind::Text {
                opening,
                parts,
                closing,
            } => {
                let mut errors = vec![];

                let opening_single_quote_count = match &opening.kind {
                    CstKind::OpeningText {
                        opening_single_quotes,
                        opening_double_quote: box Cst {
                            kind: CstKind::DoubleQuote,
                            ..
                        }
                    } if opening_single_quotes
                        .iter()
                        .all(|single_quote| -> bool {
                            matches!(single_quote.kind, CstKind::SingleQuote)
                        }) => opening_single_quotes.len(),
                    _ => panic!("Text needs to start with any number of single quotes followed by a double quote, but started with {}.", opening)
                };

                let mut lowered_parts = vec![];
                for part in parts {
                    match &part.kind {
                        CstKind::TextPart(text) => {
                            let string = self.create_string_without_id_mapping(text.clone());
                            let text_part =
                                self.create_ast(part.id, AstKind::TextPart(TextPart(string)));
                            lowered_parts.push(text_part);
                        },
                        CstKind::TextInterpolation {
                            opening_curly_braces,
                            expression,
                            closing_curly_braces,
                        } => {
                            if lowering_type != LoweringType::Expression {
                                errors.push(
                                    self.create_error(
                                        part,
                                        AstError::PatternContainsInvalidExpression,
                                    ),
                                );
                            };

                            if opening_curly_braces.len() != (opening_single_quote_count + 1)
                                || opening_curly_braces
                                    .iter()
                                    .all(|opening_curly_brace| !matches!(opening_curly_brace.kind, CstKind::OpeningCurlyBrace))
                            {
                                panic!(
                                    "Text interpolation needs to start with {} opening curly braces, but started with {}.", 
                                    opening_single_quote_count + 1,
                                    opening_curly_braces.iter().map(|cst| format!("{}", cst)).join(""),
                                )
                            }

                            let ast = self.lower_cst(expression, LoweringType::Expression);

                            if closing_curly_braces.len() == opening_single_quote_count + 1
                                && closing_curly_braces
                                    .iter()
                                    .all(|closing_curly_brace| matches!(closing_curly_brace.kind, CstKind::ClosingCurlyBrace))
                            {
                                lowered_parts.push(ast);
                            } else {
                                let error = self.create_ast(
                                    part.id,
                                    AstKind::Error {
                                        child: Some(Box::new(ast)),
                                        errors: vec![CompilerError {
                                            module: self.module.clone(),
                                            span: part.span.clone(),
                                            payload: CompilerErrorPayload::Ast(
                                                AstError::TextInterpolationWithoutClosingCurlyBraces,
                                            ),
                                        }],
                                    },
                                );
                                lowered_parts.push(error);
                            }
                        },
                        CstKind::Error { error, .. } => errors.push(CompilerError {
                            module: self.module.clone(),
                            span: part.span.clone(),
                            payload: CompilerErrorPayload::Rcst(error.clone()),
                        }),
                        _ => panic!("Text contains non-TextPart. Whitespaces should have been removed already."),
                    }
                }
                let mut text = self.create_ast(cst.id, AstKind::Text(Text(lowered_parts)));

                if !matches!(
                    &closing.kind,
                    CstKind::ClosingText {
                        closing_double_quote: box Cst {
                            kind: CstKind::DoubleQuote,
                            ..
                        },
                        closing_single_quotes
                    } if closing_single_quotes
                        .iter()
                        .all(|single_quote| -> bool {
                            matches!(single_quote.kind, CstKind::SingleQuote)
                        }) && opening_single_quote_count == closing_single_quotes.len()
                ) {
                    errors.push(self.create_error(closing, AstError::TextWithoutClosingQuote));
                }

                if !errors.is_empty() {
                    text = self.create_ast(
                        cst.id,
                        AstKind::Error {
                            child: Some(Box::new(text)),
                            errors,
                        },
                    );
                }
                text
            }
            CstKind::OpeningText { .. } => panic!("OpeningText should only occur in Text."),
            CstKind::ClosingText { .. } => panic!("ClosingText should only occur in Text."),
            CstKind::TextPart(_) => panic!("TextPart should only occur in Text."),
            CstKind::TextInterpolation { .. } => {
                panic!("TextInterpolation should only occur in Text.")
            }
            CstKind::Pipe {
                receiver,
                bar,
                call,
            } => {
                match lowering_type {
                    LoweringType::Expression => {}
                    LoweringType::Pattern | LoweringType::PatternLiteralPart => {
                        return self.create_ast(
                            cst.id,
                            AstKind::Error {
                                child: None,
                                errors: vec![self.create_error(cst, AstError::PipeInPattern)],
                            },
                        )
                    }
                }

                let ast = self.lower_cst(receiver, LoweringType::Expression);

                assert!(
                    matches!(bar.kind, CstKind::Bar),
                    "Pipe must contain a bar, but instead contained a {}.",
                    bar,
                );

                let call = self.lower_cst(call, LoweringType::Expression);
                let call = match call {
                    Ast {
                        kind:
                            AstKind::Call(ast::Call {
                                receiver,
                                mut arguments,
                            }),
                        ..
                    } => {
                        arguments.insert(0, ast);
                        ast::Call {
                            receiver,
                            arguments,
                        }
                    }
                    call => ast::Call {
                        receiver: Box::new(call),
                        arguments: vec![ast],
                    },
                };
                self.create_ast(cst.id, AstKind::Call(call))
            }
            CstKind::Parenthesized {
                opening_parenthesis,
                inner,
                closing_parenthesis,
            } => {
                match lowering_type {
                    LoweringType::Expression => {
                        return self.create_ast(
                            cst.id,
                            AstKind::Error {
                                child: None,
                                errors: vec![
                                    self.create_error(cst, AstError::ParenthesizedInPattern)
                                ],
                            },
                        )
                    }
                    LoweringType::Pattern => {}
                    LoweringType::PatternLiteralPart => {}
                }

                let mut ast = self.lower_cst(inner, LoweringType::Expression);

                assert!(
                    matches!(opening_parenthesis.kind, CstKind::OpeningParenthesis),
                    "Parenthesized needs to start with opening parenthesis, but started with {}.",
                    opening_parenthesis,
                );
                if !matches!(closing_parenthesis.kind, CstKind::ClosingParenthesis) {
                    ast = self.create_ast(
                        closing_parenthesis.id,
                        AstKind::Error {
                            child: Some(Box::new(ast)),
                            errors: vec![self.create_error(
                                closing_parenthesis,
                                AstError::ParenthesizedWithoutClosingParenthesis,
                            )],
                        },
                    );
                }

                ast
            }
            CstKind::Call {
                receiver,
                arguments,
            } => {
                match lowering_type {
                    LoweringType::Expression => {}
                    LoweringType::Pattern | LoweringType::PatternLiteralPart => {
                        return self.create_ast(
                            cst.id,
                            AstKind::Error {
                                child: None,
                                errors: vec![self.create_error(cst, AstError::CallInPattern)],
                            },
                        )
                    }
                }

                let mut receiver_kind = &receiver.kind;
                loop {
                    receiver_kind = match receiver_kind {
                        CstKind::Parenthesized {
                            opening_parenthesis,
                            inner,
                            closing_parenthesis,
                        } => {
                            assert!(
                                matches!(opening_parenthesis.kind, CstKind::OpeningParenthesis),
                                "Parenthesized needs to start with opening parenthesis, but started with {}.",
                                opening_parenthesis
                            );
                            assert!(
                                matches!(
                                    closing_parenthesis.kind,
                                    CstKind::ClosingParenthesis
                                ),
                                "Parenthesized for a call receiver needs to end with closing parenthesis, but ended with {}.", closing_parenthesis
                            );
                            &inner.kind
                        }
                        _ => break,
                    };
                }
                let receiver = self.lower_cst(receiver, LoweringType::Expression);
                let arguments = self.lower_csts(arguments);

                self.create_ast(
                    cst.id,
                    AstKind::Call(ast::Call {
                        receiver: receiver.into(),
                        arguments,
                    }),
                )
            }
            CstKind::List {
                opening_parenthesis,
                items,
                closing_parenthesis,
            } => {
                let mut errors = vec![];

                if lowering_type == LoweringType::PatternLiteralPart {
                    errors.push(
                        self.create_error(
                            cst,
                            AstError::PatternLiteralPartContainsInvalidExpression,
                        ),
                    );
                };

                assert!(
                    matches!(opening_parenthesis.kind, CstKind::OpeningParenthesis),
                    "List should always have an opening parenthesis, but instead had {}.",
                    opening_parenthesis
                );

                let mut ast_items = vec![];
                if items.len() == 1 && let CstKind::Comma = items[0].kind {
                    // Empty list (`(,)`), do nothing.
                } else {
                    for item in items {
                        let CstKind::ListItem {
                            value,
                            comma,
                        } = &item.kind else {
                            errors.push(self.create_error(cst, AstError::ListWithNonListItem));
                            continue;
                        };

                        let mut value = self.lower_cst(&value.clone(), lowering_type);

                        if let Some(comma) = comma {
                            if !matches!(comma.kind, CstKind::Comma) {
                                value = self.create_ast(
                                    comma.id,
                                    AstKind::Error {
                                        child: Some(Box::new(value)),
                                        errors: vec![self.create_error(comma, AstError::ListItemWithoutComma)],
                                    },
                                );
                            }
                        }

                        ast_items.push(value);
                    }
                }

                if !matches!(closing_parenthesis.kind, CstKind::ClosingParenthesis) {
                    errors.push(self.create_error(
                        closing_parenthesis,
                        AstError::ListWithoutClosingParenthesis,
                    ));
                }

                let mut ast = self.create_ast(cst.id, AstKind::List(List(ast_items)));
                if !errors.is_empty() {
                    ast = self.create_ast(
                        cst.id,
                        AstKind::Error {
                            child: Some(Box::new(ast)),
                            errors,
                        },
                    );
                }
                ast
            }
            CstKind::ListItem { .. } => panic!("ListItem should only appear in List."),
            CstKind::Struct {
                opening_bracket,
                fields,
                closing_bracket,
            } => {
                let mut errors = vec![];

                if lowering_type == LoweringType::PatternLiteralPart {
                    errors.push(
                        self.create_error(
                            cst,
                            AstError::PatternLiteralPartContainsInvalidExpression,
                        ),
                    );
                };

                assert!(
                    matches!(opening_bracket.kind, CstKind::OpeningBracket),
                    "Struct should always have an opening bracket, but instead had {}.",
                    opening_bracket
                );

                let fields = fields
                    .iter()
                    .filter_map(|field| {
                        let CstKind::StructField {
                            key_and_colon,
                            value,
                            comma,
                        } = &field.kind else {
                            errors.push(self.create_error(cst, AstError::StructWithNonStructField));
                           return None;
                        };

                        if let Some(box (key, colon)) = key_and_colon {
                            // Normal syntax, e.g. `[foo: bar]`.

                            let key_lowering_type = match lowering_type {
                                LoweringType::Expression => LoweringType::Expression,
                                LoweringType::Pattern | LoweringType::PatternLiteralPart => {
                                    LoweringType::PatternLiteralPart
                                }
                            };
                            let mut key = self.lower_cst(key, key_lowering_type);

                            if !matches!(colon.kind, CstKind::Colon) {
                                key = self.create_ast(
                                    colon.id,
                                    AstKind::Error {
                                        child: Some(Box::new(key)),
                                        errors: vec![self
                                            .create_error(colon, AstError::StructKeyWithoutColon)],
                                    },
                                )
                            }

                            let mut value = self.lower_cst(&value.clone(), lowering_type);

                            if let Some(comma) = comma {
                                if !matches!(comma.kind, CstKind::Comma) {
                                    value = self.create_ast(
                                        comma.id,
                                        AstKind::Error {
                                            child: Some(Box::new(value)),
                                            errors: vec![self.create_error(
                                                comma,
                                                AstError::StructValueWithoutComma,
                                            )],
                                        },
                                    )
                                }
                            }
                            Some((Some(key), value))
                        } else {
                            // Shorthand syntax, e.g. `[foo]`.
                            let mut ast = self.lower_cst(&value.clone(), lowering_type);

                            if !matches!(ast.kind, AstKind::Identifier(_)) {
                                ast = self.create_ast(
                                    value.id,
                                    AstKind::Error {
                                        child: Some(Box::new(ast)),
                                        errors: vec![self.create_error(
                                            value,
                                            AstError::StructShorthandWithNotIdentifier,
                                        )],
                                    },
                                )
                            }

                            if let Some(comma) = comma {
                                if !matches!(comma.kind, CstKind::Comma) {
                                    ast = self.create_ast(
                                        comma.id,
                                        AstKind::Error {
                                            child: Some(Box::new(ast)),
                                            errors: vec![self.create_error(
                                                comma,
                                                AstError::StructValueWithoutComma,
                                            )],
                                        },
                                    )
                                }
                            }
                            Some((None, ast))
                        }
                    })
                    .collect();

                if !matches!(closing_bracket.kind, CstKind::ClosingBracket) {
                    errors.push(
                        self.create_error(closing_bracket, AstError::StructWithoutClosingBrace),
                    );
                }

                let mut ast = self.create_ast(cst.id, AstKind::Struct(Struct { fields }));
                if !errors.is_empty() {
                    ast = self.create_ast(
                        cst.id,
                        AstKind::Error {
                            child: Some(Box::new(ast)),
                            errors,
                        },
                    );
                }
                ast
            }
            CstKind::StructField { .. } => panic!("StructField should only appear in Struct."),
            CstKind::StructAccess { struct_, dot, key } => {
                let mut errors = vec![];

                if lowering_type != LoweringType::Expression {
                    errors.push(self.create_error(cst, AstError::PatternContainsInvalidExpression));
                };

                let mut ast = self.lower_struct_access(cst.id, struct_, dot, key);
                if !errors.is_empty() {
                    ast = self.create_ast(
                        cst.id,
                        AstKind::Error {
                            child: Some(Box::new(ast)),
                            errors,
                        },
                    );
                }
                ast
            }
            CstKind::Lambda {
                opening_curly_brace,
                parameters_and_arrow,
                body,
                closing_curly_brace,
            } => {
                let mut errors = vec![];

                if lowering_type != LoweringType::Expression {
                    errors.push(self.create_error(cst, AstError::PatternContainsInvalidExpression));
                };

                assert!(
                    matches!(opening_curly_brace.kind, CstKind::OpeningCurlyBrace),
                    "Expected an opening curly brace at the beginning of a lambda, but found {}.",
                    opening_curly_brace,
                );

                let (parameters, mut parameter_errors) =
                    if let Some((parameters, arrow)) = parameters_and_arrow {
                        assert!(
                            matches!(arrow.kind, CstKind::Arrow),
                            "Expected an arrow after the parameters in a lambda, but found `{}`.",
                            arrow
                        );
                        self.lower_parameters(parameters)
                    } else {
                        (vec![], vec![])
                    };
                errors.append(&mut parameter_errors);

                let body = self.lower_csts(body);

                if !matches!(closing_curly_brace.kind, CstKind::ClosingCurlyBrace) {
                    errors.push(self.create_error(
                        closing_curly_brace,
                        AstError::LambdaWithoutClosingCurlyBrace,
                    ));
                }

                let mut ast = self.create_ast(
                    cst.id,
                    AstKind::Lambda(Lambda {
                        parameters,
                        body,
                        fuzzable: false,
                    }),
                );
                if !errors.is_empty() {
                    ast = self.create_ast(
                        cst.id,
                        AstKind::Error {
                            child: None,
                            errors,
                        },
                    );
                }
                ast
            }
            CstKind::Assignment {
                name_or_pattern,
                parameters,
                assignment_sign,
                body,
            } => {
                assert!(
                    matches!(assignment_sign.kind, CstKind::EqualsSign | CstKind::ColonEqualsSign),
                    "Expected an equals sign or colon equals sign for the assignment, but found {} instead.",
                    assignment_sign,
                );

                let body = self.lower_csts(body);
                let (body, errors) = if parameters.is_empty() {
                    let body = AssignmentBody::Body {
                        pattern: Box::new(self.lower_cst(name_or_pattern, LoweringType::Pattern)),
                        body,
                    };
                    (body, vec![])
                } else {
                    let name = match &name_or_pattern.kind {
                        CstKind::Identifier(identifier) => {
                            self.create_string(name_or_pattern.id.to_owned(), identifier.to_owned())
                        }
                        CstKind::Error { error, .. } => {
                            return self.create_ast(
                                cst.id,
                                AstKind::Error {
                                    child: None,
                                    errors: vec![CompilerError {
                                        module: self.module.clone(),
                                        span: name_or_pattern.span.clone(),
                                        payload: CompilerErrorPayload::Rcst(error.clone()),
                                    }],
                                },
                            );
                        }
                        _ => {
                            return self.create_ast(
                                cst.id,
                                AstKind::Error {
                                    child: None,
                                    errors: vec![CompilerError {
                                        module: self.module.clone(),
                                        span: name_or_pattern.span.clone(),
                                        payload: CompilerErrorPayload::Ast(
                                            AstError::ExpectedNameOrPatternInAssignment,
                                        ),
                                    }],
                                },
                            );
                        }
                    };

                    let (parameters, errors) = self.lower_parameters(parameters);
                    let body = AssignmentBody::Lambda {
                        name,
                        lambda: Lambda {
                            parameters,
                            body,
                            fuzzable: true,
                        },
                    };
                    (body, errors)
                };

                let mut ast = self.create_ast(
                    cst.id,
                    AstKind::Assignment(ast::Assignment {
                        is_public: matches!(assignment_sign.kind, CstKind::ColonEqualsSign),
                        body,
                    }),
                );
                if !errors.is_empty() {
                    ast = self.create_ast(
                        cst.id,
                        AstKind::Error {
                            child: None,
                            errors,
                        },
                    );
                }
                ast
            }
            CstKind::Error { error, .. } => self.create_ast(
                cst.id,
                AstKind::Error {
                    child: None,
                    errors: vec![CompilerError {
                        module: self.module.clone(),
                        span: cst.span.clone(),
                        payload: CompilerErrorPayload::Rcst(error.clone()),
                    }],
                },
            ),
        }
    }

    fn lower_struct_access(&mut self, id: cst::Id, struct_: &Cst, dot: &Cst, key: &Cst) -> Ast {
        let struct_ = self.lower_cst(struct_, LoweringType::Expression);

        assert!(
            matches!(dot.kind, CstKind::Dot),
            "Struct access should always have a dot, but instead had {}.",
            dot
        );

        let key = match key.kind.clone() {
            CstKind::Identifier(identifier) => {
                self.create_string(key.id.to_owned(), identifier.uppercase_first_letter())
            }
            // TODO: handle CstKind::Error
            _ => panic!(
                "Expected an identifier after the dot in a struct access, but found `{}`.",
                key
            ),
        };

        self.create_ast(
            id,
            AstKind::StructAccess(ast::StructAccess {
                struct_: Box::new(struct_),
                key,
            }),
        )
    }

    fn lower_parameters(&mut self, csts: &[Cst]) -> (Vec<AstString>, Vec<CompilerError>) {
        let mut errors = vec![];
        let parameters = csts
            .iter()
            .enumerate()
            .map(|(index, it)| match self.lower_parameter(it) {
                Ok(parameter) => parameter,
                Err(error) => {
                    errors.push(error);
                    self.create_string(it.id, format!("<invalid#{index}>"))
                }
            })
            .collect();
        (parameters, errors)
    }
    fn lower_parameter(&mut self, cst: &Cst) -> Result<AstString, CompilerError> {
        if let CstKind::Identifier(identifier) = &cst.kind {
            Ok(self.create_string(cst.id.to_owned(), identifier.clone()))
        } else {
            Err(self.create_error(cst, AstError::ExpectedParameter))
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
        let id = ast::Id::new(self.module.clone(), self.next_id);
        self.next_id += 1;
        id
    }

    fn create_error(&self, cst: &Cst, error: AstError) -> CompilerError {
        CompilerError {
            module: self.module.clone(),
            span: cst.span.clone(),
            payload: CompilerErrorPayload::Ast(error),
        }
    }
}
