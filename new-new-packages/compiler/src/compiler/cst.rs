use super::{rcst::RcstError, rcst_to_cst::RcstToCst};
use crate::input::Input;
use std::{
    fmt::{self, Display, Formatter},
    ops::Range,
};

#[salsa::query_group(CstDbStorage)]
pub trait CstDb: RcstToCst {
    fn find_cst(&self, input: Input, id: Id) -> Option<Cst>;
}

fn find_cst(db: &dyn CstDb, input: Input, id: Id) -> Option<Cst> {
    db.cst(input).unwrap().find(&id).map(|it| it.to_owned())
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Hash)]
pub struct Id(pub usize);

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct Cst {
    pub id: Id,
    pub span: Range<usize>,
    pub kind: CstKind,
}

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub enum CstKind {
    EqualsSign,         // =
    Comma,              // ,
    Colon,              // :
    OpeningParenthesis, // (
    ClosingParenthesis, // )
    OpeningBracket,     // [
    ClosingBracket,     // ]
    OpeningCurlyBrace,  // {
    ClosingCurlyBrace,  // }
    Arrow,              // ->
    DoubleQuote,        // "
    Octothorpe,         // #
    Whitespace(String),
    Newline, // TODO: Support different kinds of newlines.
    Comment {
        octothorpe: Box<Cst>,
        comment: String,
    },
    TrailingWhitespace {
        child: Box<Cst>,
        whitespace: Vec<Cst>,
    },
    Identifier(String),
    Symbol(String),
    Int(u64),
    Text {
        opening_quote: Box<Cst>,
        parts: Vec<Cst>,
        closing_quote: Box<Cst>,
    },
    TextPart(String),
    Parenthesized {
        opening_parenthesis: Box<Cst>,
        inner: Box<Cst>,
        closing_parenthesis: Box<Cst>,
    },
    Call {
        name: Box<Cst>,
        arguments: Vec<Cst>,
    },
    Struct {
        opening_bracket: Box<Cst>,
        fields: Vec<Cst>,
        closing_bracket: Box<Cst>,
    },
    StructField {
        key: Box<Cst>,
        colon: Box<Cst>,
        value: Box<Cst>,
        comma: Option<Box<Cst>>,
    },
    Lambda {
        opening_curly_brace: Box<Cst>,
        parameters_and_arrow: Option<(Vec<Cst>, Box<Cst>)>,
        body: Vec<Cst>,
        closing_curly_brace: Box<Cst>,
    },
    Assignment {
        name: Box<Cst>,
        parameters: Vec<Cst>,
        equals_sign: Box<Cst>,
        body: Vec<Cst>,
    },
    Error {
        unparsable_input: String,
        error: RcstError,
    },
}

impl Display for Cst {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match &self.kind {
            CstKind::EqualsSign => "=".fmt(f),
            CstKind::Comma => ",".fmt(f),
            CstKind::Colon => ":".fmt(f),
            CstKind::OpeningParenthesis => "(".fmt(f),
            CstKind::ClosingParenthesis => ")".fmt(f),
            CstKind::OpeningBracket => "[".fmt(f),
            CstKind::ClosingBracket => "]".fmt(f),
            CstKind::OpeningCurlyBrace => "{".fmt(f),
            CstKind::ClosingCurlyBrace => "}".fmt(f),
            CstKind::Arrow => "->".fmt(f),
            CstKind::DoubleQuote => '"'.fmt(f),
            CstKind::Octothorpe => '#'.fmt(f),
            CstKind::Whitespace(whitespace) => whitespace.fmt(f),
            CstKind::Newline => '\n'.fmt(f),
            CstKind::Comment {
                octothorpe,
                comment,
            } => {
                octothorpe.fmt(f)?;
                comment.fmt(f)
            }
            CstKind::TrailingWhitespace { child, whitespace } => {
                child.fmt(f)?;
                for w in whitespace {
                    w.fmt(f)?;
                }
                Ok(())
            }
            CstKind::Identifier(identifier) => identifier.fmt(f),
            CstKind::Symbol(symbol) => symbol.fmt(f),
            CstKind::Int(int) => int.fmt(f),
            CstKind::Text {
                opening_quote,
                parts,
                closing_quote,
            } => {
                opening_quote.fmt(f)?;
                for part in parts {
                    part.fmt(f)?;
                }
                closing_quote.fmt(f)
            }
            CstKind::TextPart(literal) => literal.fmt(f),
            CstKind::Parenthesized {
                opening_parenthesis,
                inner,
                closing_parenthesis,
            } => {
                opening_parenthesis.fmt(f)?;
                inner.fmt(f)?;
                closing_parenthesis.fmt(f)
            }
            CstKind::Call { name, arguments } => {
                name.fmt(f)?;
                for argument in arguments {
                    argument.fmt(f)?;
                }
                Ok(())
            }
            CstKind::Struct {
                opening_bracket,
                fields,
                closing_bracket,
            } => {
                opening_bracket.fmt(f)?;
                for field in fields {
                    field.fmt(f)?;
                }
                closing_bracket.fmt(f)
            }
            CstKind::StructField {
                key,
                colon,
                value,
                comma,
            } => {
                key.fmt(f)?;
                colon.fmt(f)?;
                value.fmt(f)?;
                if let Some(comma) = comma {
                    comma.fmt(f)?;
                }
                Ok(())
            }
            CstKind::Lambda {
                opening_curly_brace,
                parameters_and_arrow,
                body,
                closing_curly_brace,
            } => {
                opening_curly_brace.fmt(f)?;
                if let Some((parameters, arrow)) = parameters_and_arrow {
                    for parameter in parameters {
                        parameter.fmt(f)?;
                    }
                    arrow.fmt(f)?;
                }
                for expression in body {
                    expression.fmt(f)?;
                }
                closing_curly_brace.fmt(f)
            }
            CstKind::Assignment {
                name,
                parameters,
                equals_sign,
                body,
            } => {
                name.fmt(f)?;
                for parameter in parameters {
                    parameter.fmt(f)?;
                }
                equals_sign.fmt(f)?;
                for expression in body {
                    expression.fmt(f)?;
                }
                Ok(())
            }
            CstKind::Error {
                unparsable_input, ..
            } => unparsable_input.fmt(f),
        }
    }
}

impl Cst {
    /// Returns a span that makes sense to display in the editor.
    ///
    /// For example, if a call contains errors, we want to only underline the
    /// name of the called function itself, not everything including arguments.
    pub fn display_span(&self) -> Range<usize> {
        match &self.kind {
            CstKind::TrailingWhitespace { child, .. } => child.display_span(),
            CstKind::Call { name, .. } => name.display_span(),
            CstKind::Assignment { name, .. } => name.display_span(),
            _ => self.span.clone(),
        }
    }

    fn find(&self, id: &Id) -> Option<&Cst> {
        if id == &self.id {
            return Some(self);
        };

        match &self.kind {
            CstKind::EqualsSign => None,
            CstKind::Comma => None,
            CstKind::Colon => None,
            CstKind::OpeningParenthesis => None,
            CstKind::ClosingParenthesis => None,
            CstKind::OpeningBracket => None,
            CstKind::ClosingBracket => None,
            CstKind::OpeningCurlyBrace => None,
            CstKind::ClosingCurlyBrace => None,
            CstKind::Arrow => None,
            CstKind::DoubleQuote => None,
            CstKind::Octothorpe => None,
            CstKind::Whitespace(_) => None,
            CstKind::Newline => None,
            CstKind::Comment { octothorpe, .. } => octothorpe.find(id),
            CstKind::TrailingWhitespace { child, whitespace } => {
                child.find(id).or_else(|| whitespace.find(id))
            }
            CstKind::Identifier(_) => None,
            CstKind::Symbol(_) => None,
            CstKind::Int(_) => None,
            CstKind::Text {
                opening_quote,
                parts,
                closing_quote,
            } => opening_quote
                .find(id)
                .or_else(|| parts.find(id))
                .or_else(|| closing_quote.find(id)),
            CstKind::TextPart(_) => None,
            CstKind::Parenthesized {
                opening_parenthesis,
                inner,
                closing_parenthesis,
            } => opening_parenthesis
                .find(id)
                .or_else(|| inner.find(id))
                .or_else(|| closing_parenthesis.find(id)),
            CstKind::Call { name, arguments } => name.find(id).or_else(|| arguments.find(id)),
            CstKind::Struct {
                opening_bracket,
                fields,
                closing_bracket,
            } => opening_bracket
                .find(id)
                .or_else(|| fields.find(id))
                .or_else(|| closing_bracket.find(id)),
            CstKind::StructField {
                key,
                colon,
                value,
                comma,
            } => key
                .find(id)
                .or_else(|| colon.find(id))
                .or_else(|| value.find(id))
                .or_else(|| comma.as_ref().and_then(|comma| comma.find(id))),
            CstKind::Lambda {
                opening_curly_brace,
                parameters_and_arrow,
                body,
                closing_curly_brace,
            } => opening_curly_brace
                .find(id)
                .or_else(|| {
                    parameters_and_arrow
                        .as_ref()
                        .and_then(|(parameters, arrow)| {
                            parameters.find(id).or_else(|| arrow.find(id))
                        })
                })
                .or_else(|| body.find(id))
                .or_else(|| closing_curly_brace.find(id)),
            CstKind::Assignment {
                name,
                parameters,
                equals_sign,
                body,
            } => name
                .find(id)
                .or_else(|| parameters.find(id))
                .or_else(|| equals_sign.find(id))
                .or_else(|| body.find(id)),
            CstKind::Error { .. } => None,
        }
    }

    pub fn unwrap_whitespace_and_comment(&self) -> &Self {
        match &self.kind {
            CstKind::TrailingWhitespace { child, .. } => child.unwrap_whitespace_and_comment(),
            _ => self,
        }
    }
}

pub trait CstVecExtension {
    fn find(&self, id: &Id) -> Option<&Cst>;
}
impl<T> CstVecExtension for T
where
    T: AsRef<[Cst]>,
{
    fn find(&self, id: &Id) -> Option<&Cst> {
        let slice = self.as_ref();
        let child_index = slice
            .binary_search_by_key(id, |it| it.id)
            .or_else(|err| if err == 0 { Err(()) } else { Ok(err - 1) })
            .ok()?;
        slice[child_index].find(id)
    }
}
