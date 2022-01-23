use std::{
    fmt::{self, Display, Formatter},
    ops::Range,
};

use itertools::Itertools;

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub struct CstId(pub usize);

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct Cst {
    pub id: CstId,
    pub kind: CstKind,
}

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub enum CstKind {
    // Simple characters.
    EqualsSign {
        offset: usize,
    },
    OpeningParenthesis {
        offset: usize,
    },
    ClosingParenthesis {
        offset: usize,
    },
    OpeningCurlyBrace {
        offset: usize,
    },
    ClosingCurlyBrace {
        offset: usize,
    },
    Arrow {
        offset: usize,
    },

    // Self-contained atoms of the language.
    Int {
        offset: usize,
        value: u64,
        source: String,
    },
    Text {
        offset: usize,
        value: String,
    },
    Identifier {
        offset: usize,
        value: String,
    },
    Symbol {
        offset: usize,
        value: String,
    },

    // Decorators.
    LeadingWhitespace {
        value: String,
        child: Box<Cst>,
    },
    LeadingComment {
        value: String, // without #
        child: Box<Cst>,
    },
    TrailingWhitespace {
        child: Box<Cst>,
        value: String,
    },
    TrailingComment {
        child: Box<Cst>,
        value: String, // without #
    },

    // Compound expressions.
    Parenthesized {
        opening_parenthesis: Box<Cst>,
        inner: Box<Cst>,
        closing_parenthesis: Box<Cst>,
    },
    Lambda {
        opening_curly_brace: Box<Cst>,
        parameters_and_arrow: Option<(Vec<Cst>, Box<Cst>)>,
        body: Vec<Cst>,
        closing_curly_brace: Box<Cst>,
    },
    Call {
        name: Box<Cst>,
        arguments: Vec<Cst>,
    },
    Assignment {
        name: Box<Cst>,
        parameters: Vec<Cst>,
        equals_sign: Box<Cst>,
        body: Vec<Cst>,
    },

    /// Indicates a parsing of some subtree did not succeed.
    Error {
        offset: usize,
        unparsable_input: String,
        message: String,
    },
}

impl Display for Cst {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match &self.kind {
            CstKind::EqualsSign { .. } => write!(f, "="),
            CstKind::OpeningParenthesis { .. } => write!(f, "("),
            CstKind::ClosingParenthesis { .. } => write!(f, ")"),
            CstKind::OpeningCurlyBrace { .. } => write!(f, "{{"),
            CstKind::ClosingCurlyBrace { .. } => write!(f, "}}"),
            CstKind::Arrow { .. } => write!(f, "->"),
            CstKind::Int { source, .. } => write!(f, "{}", source),
            CstKind::Text { value, .. } => write!(f, "\"{}\"", value),
            CstKind::Identifier { value, .. } => write!(f, "{}", value),
            CstKind::Symbol { value, .. } => write!(f, "{}", value),
            CstKind::LeadingWhitespace { value, child } => write!(f, "{}{}", value, child),
            CstKind::LeadingComment { value, child } => write!(f, "{}{}", value, child),
            CstKind::TrailingWhitespace { child, value } => write!(f, "{}{}", child, value),
            CstKind::TrailingComment { child, value } => write!(f, "{}#{}", child, value),
            CstKind::Parenthesized {
                opening_parenthesis,
                inner,
                closing_parenthesis,
            } => write!(f, "{}{}{}", opening_parenthesis, inner, closing_parenthesis),
            CstKind::Lambda {
                opening_curly_brace,
                parameters_and_arrow,
                body,
                closing_curly_brace,
            } => write!(
                f,
                "{}{}{}{}",
                opening_curly_brace,
                parameters_and_arrow
                    .as_ref()
                    .map(|(parameters, arrow)| format!(
                        "{}{}",
                        parameters.iter().map(|it| format!("{}", it)).join(""),
                        arrow
                    ))
                    .unwrap_or("".into()),
                body.iter().map(|it| format!("{}", it)).join(""),
                closing_curly_brace,
            ),
            CstKind::Call { name, arguments } => {
                write!(
                    f,
                    "{}{}",
                    name,
                    arguments.iter().map(|it| format!("{}", it)).join("")
                )
            }
            CstKind::Assignment {
                name,
                parameters,
                equals_sign,
                body,
            } => write!(
                f,
                "{}{}{}{}",
                name,
                parameters.iter().map(|it| format!("{}", it)).join(""),
                equals_sign,
                body.iter().map(|it| format!("{}", it)).join(""),
            ),
            CstKind::Error {
                unparsable_input, ..
            } => write!(f, "{}", unparsable_input),
        }
    }
}

impl Cst {
    pub fn span(&self) -> Range<usize> {
        match &self.kind {
            CstKind::EqualsSign { offset } => *offset..(*offset + 1),
            CstKind::OpeningParenthesis { offset } => *offset..(*offset + 1),
            CstKind::ClosingParenthesis { offset } => *offset..(*offset + 1),
            CstKind::OpeningCurlyBrace { offset } => *offset..(*offset + 1),
            CstKind::ClosingCurlyBrace { offset } => *offset..(*offset + 1),
            CstKind::Arrow { offset } => *offset..(*offset + 2),
            CstKind::Int { offset, source, .. } => *offset..(*offset + source.len()),
            CstKind::Text { offset, value } => *offset..(*offset + value.len() + 2),
            CstKind::Identifier { offset, value } => *offset..(*offset + value.len()),
            CstKind::Symbol { offset, value } => *offset..(*offset + value.len()),
            CstKind::LeadingWhitespace { value, child } => {
                let child_span = child.span();
                (child_span.start - value.len())..child_span.end
            }
            CstKind::LeadingComment { value, child } => {
                let child_span = child.span();
                (child_span.start - value.len() - 1)..child_span.end
            }
            CstKind::TrailingWhitespace { child, value } => {
                let child_span = child.span();
                child_span.start..(child_span.end + value.len())
            }
            CstKind::TrailingComment { child, value } => {
                let child_span = child.span();
                child_span.start..(child_span.end + value.len() + 1)
            }
            CstKind::Parenthesized {
                opening_parenthesis,
                closing_parenthesis,
                ..
            } => opening_parenthesis.span().start..closing_parenthesis.span().end,
            CstKind::Lambda {
                opening_curly_brace,
                closing_curly_brace,
                ..
            } => opening_curly_brace.span().start..closing_curly_brace.span().end,
            CstKind::Call { name, arguments } => {
                if arguments.is_empty() {
                    name.span()
                } else {
                    name.span().start..arguments.last().unwrap().span().end
                }
            }
            CstKind::Assignment {
                name,
                equals_sign,
                body,
                ..
            } => {
                let last_cst = body.last().unwrap_or(&*equals_sign);
                name.span().start..last_cst.span().end
            }
            CstKind::Error {
                offset,
                unparsable_input,
                ..
            } => *offset..(*offset + unparsable_input.len()),
        }
    }

    pub fn unwrap_whitespace_and_comment(&self) -> &Self {
        match &self.kind {
            CstKind::LeadingWhitespace { child, .. } => child.unwrap_whitespace_and_comment(),
            CstKind::LeadingComment { child, .. } => child.unwrap_whitespace_and_comment(),
            CstKind::TrailingWhitespace { child, .. } => child.unwrap_whitespace_and_comment(),
            CstKind::TrailingComment { child, .. } => child.unwrap_whitespace_and_comment(),
            _ => self,
        }
    }
}
