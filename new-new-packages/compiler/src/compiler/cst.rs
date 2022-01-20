use std::{
    fmt::{self, Display, Formatter},
    ops::Range,
};

use itertools::Itertools;

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub enum Cst {
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
        match self {
            Cst::EqualsSign { .. } => write!(f, "="),
            Cst::OpeningParenthesis { .. } => write!(f, "("),
            Cst::ClosingParenthesis { .. } => write!(f, ")"),
            Cst::OpeningCurlyBrace { .. } => write!(f, "{{"),
            Cst::ClosingCurlyBrace { .. } => write!(f, "}}"),
            Cst::Arrow { .. } => write!(f, "->"),
            Cst::Int { source, .. } => write!(f, "{}", source),
            Cst::Text { value, .. } => write!(f, "\"{}\"", value),
            Cst::Identifier { value, .. } => write!(f, "{}", value),
            Cst::Symbol { value, .. } => write!(f, "{}", value),
            Cst::LeadingWhitespace { value, child } => write!(f, "{}{}", value, child),
            Cst::LeadingComment { value, child } => write!(f, "{}{}", value, child),
            Cst::TrailingWhitespace { child, value } => write!(f, "{}{}", child, value),
            Cst::TrailingComment { child, value } => write!(f, "{}#{}", child, value),
            Cst::Parenthesized {
                opening_parenthesis,
                inner,
                closing_parenthesis,
            } => write!(f, "{}{}{}", opening_parenthesis, inner, closing_parenthesis),
            Cst::Lambda {
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
            Cst::Call { name, arguments } => {
                write!(
                    f,
                    "{}{}",
                    name,
                    arguments.iter().map(|it| format!("{}", it)).join("")
                )
            }
            Cst::Assignment {
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
            Cst::Error {
                unparsable_input, ..
            } => write!(f, "{}", unparsable_input),
        }
    }
}

impl Cst {
    pub fn span(&self) -> Range<usize> {
        match self {
            Cst::EqualsSign { offset } => *offset..(*offset + 2),
            Cst::OpeningParenthesis { offset } => *offset..(*offset + 2),
            Cst::ClosingParenthesis { offset } => *offset..(*offset + 2),
            Cst::OpeningCurlyBrace { offset } => *offset..(*offset + 2),
            Cst::ClosingCurlyBrace { offset } => *offset..(*offset + 2),
            Cst::Arrow { offset } => *offset..(*offset + 3),
            Cst::Int { offset, source, .. } => *offset..(*offset + source.len() + 1),
            Cst::Text { offset, value } => *offset..(*offset + value.len() + 3),
            Cst::Identifier { offset, value } => *offset..(*offset + value.len() + 1),
            Cst::Symbol { offset, value } => *offset..(*offset + value.len() + 1),
            Cst::LeadingWhitespace { value, child } => {
                let child_span = child.span();
                (child_span.start - value.len())..child_span.end
            }
            Cst::LeadingComment { value, child } => {
                let child_span = child.span();
                (child_span.start - value.len() - 1)..child_span.end
            }
            Cst::TrailingWhitespace { child, value } => {
                let child_span = child.span();
                child_span.start..(child_span.end + value.len())
            }
            Cst::TrailingComment { child, value } => {
                let child_span = child.span();
                child_span.start..(child_span.end + value.len() + 1)
            }
            Cst::Parenthesized {
                opening_parenthesis,
                closing_parenthesis,
                ..
            } => opening_parenthesis.span().start..closing_parenthesis.span().end,
            Cst::Lambda {
                opening_curly_brace,
                closing_curly_brace,
                ..
            } => opening_curly_brace.span().start..closing_curly_brace.span().end,
            Cst::Call { name, arguments } => {
                if arguments.is_empty() {
                    name.span()
                } else {
                    name.span().start..arguments.last().unwrap().span().end
                }
            }
            Cst::Assignment {
                name,
                equals_sign,
                body,
                ..
            } => {
                let last_cst = body.last().unwrap_or(equals_sign);
                name.span().start..last_cst.span().end
            }
            Cst::Error {
                offset,
                unparsable_input,
                ..
            } => *offset..(*offset + unparsable_input.len() + 1),
        }
    }

    pub fn unwrap_whitespace_and_comment(&self) -> &Self {
        match self {
            Cst::LeadingWhitespace { child, .. } => child.unwrap_whitespace_and_comment(),
            Cst::LeadingComment { child, .. } => child.unwrap_whitespace_and_comment(),
            Cst::TrailingWhitespace { child, .. } => child.unwrap_whitespace_and_comment(),
            Cst::TrailingComment { child, .. } => child.unwrap_whitespace_and_comment(),
            it => it,
        }
    }
}
