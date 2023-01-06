use num_bigint::BigUint;

use super::{rcst::RcstError, rcst_to_cst::RcstToCst};
use crate::module::Module;
use std::{
    fmt::{self, Display, Formatter},
    ops::Range,
};

#[salsa::query_group(CstDbStorage)]
pub trait CstDb: RcstToCst {
    fn find_cst(&self, module: Module, id: Id) -> Cst;
    fn find_cst_by_offset(&self, module: Module, offset: usize) -> Cst;
}

fn find_cst(db: &dyn CstDb, module: Module, id: Id) -> Cst {
    db.cst(module).unwrap().find(&id).unwrap().to_owned()
}
fn find_cst_by_offset(db: &dyn CstDb, module: Module, offset: usize) -> Cst {
    db.cst(module)
        .unwrap()
        .find_by_offset(&offset)
        .unwrap()
        .to_owned()
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Hash)]
pub struct Id(pub usize);
impl Display for Id {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "CstId({})", self.0)
    }
}

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
    Dot,                // .
    Colon,              // :
    ColonEqualsSign,    // :=
    Bar,                // |
    OpeningParenthesis, // (
    ClosingParenthesis, // )
    OpeningBracket,     // [
    ClosingBracket,     // ]
    OpeningCurlyBrace,  // {
    ClosingCurlyBrace,  // }
    Arrow,              // ->
    SingleQuote,        // '
    DoubleQuote,        // "
    Octothorpe,         // #
    Whitespace(String),
    Newline(String),
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
    Int {
        value: BigUint,
        string: String,
    },
    OpeningText {
        opening_single_quotes: Vec<Cst>,
        opening_double_quote: Box<Cst>,
    },
    ClosingText {
        closing_double_quote: Box<Cst>,
        closing_single_quotes: Vec<Cst>,
    },
    Text {
        opening: Box<Cst>,
        parts: Vec<Cst>,
        closing: Box<Cst>,
    },
    TextPart(String),
    TextInterpolation {
        opening_curly_braces: Vec<Cst>,
        expression: Box<Cst>,
        closing_curly_braces: Vec<Cst>,
    },
    Pipe {
        receiver: Box<Cst>,
        bar: Box<Cst>,
        call: Box<Cst>,
    },
    Parenthesized {
        opening_parenthesis: Box<Cst>,
        inner: Box<Cst>,
        closing_parenthesis: Box<Cst>,
    },
    Call {
        receiver: Box<Cst>,
        arguments: Vec<Cst>,
    },
    List {
        opening_parenthesis: Box<Cst>,
        items: Vec<Cst>,
        closing_parenthesis: Box<Cst>,
    },
    ListItem {
        value: Box<Cst>,
        comma: Option<Box<Cst>>,
    },
    Struct {
        opening_bracket: Box<Cst>,
        fields: Vec<Cst>,
        closing_bracket: Box<Cst>,
    },
    StructField {
        key_and_colon: Option<Box<(Cst, Cst)>>,
        value: Box<Cst>,
        comma: Option<Box<Cst>>,
    },
    StructAccess {
        struct_: Box<Cst>,
        dot: Box<Cst>,
        key: Box<Cst>,
    },
    Lambda {
        opening_curly_brace: Box<Cst>,
        parameters_and_arrow: Option<(Vec<Cst>, Box<Cst>)>,
        body: Vec<Cst>,
        closing_curly_brace: Box<Cst>,
    },
    Assignment {
        name_or_pattern: Box<Cst>,
        parameters: Vec<Cst>,
        assignment_sign: Box<Cst>,
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
            CstKind::EqualsSign => '='.fmt(f),
            CstKind::Comma => ','.fmt(f),
            CstKind::Dot => '.'.fmt(f),
            CstKind::Colon => ':'.fmt(f),
            CstKind::ColonEqualsSign => ":=".fmt(f),
            CstKind::Bar => "|".fmt(f),
            CstKind::OpeningParenthesis => '('.fmt(f),
            CstKind::ClosingParenthesis => ')'.fmt(f),
            CstKind::OpeningBracket => '['.fmt(f),
            CstKind::ClosingBracket => ']'.fmt(f),
            CstKind::OpeningCurlyBrace => '{'.fmt(f),
            CstKind::ClosingCurlyBrace => '}'.fmt(f),
            CstKind::Arrow => "->".fmt(f),
            CstKind::SingleQuote => "'".fmt(f),
            CstKind::DoubleQuote => '"'.fmt(f),
            CstKind::Octothorpe => '#'.fmt(f),
            CstKind::Whitespace(whitespace) => whitespace.fmt(f),
            CstKind::Newline(newline) => newline.fmt(f),
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
            CstKind::Int { string, .. } => string.fmt(f),
            CstKind::OpeningText {
                opening_single_quotes,
                opening_double_quote,
            } => {
                for opening_single_quote in opening_single_quotes {
                    opening_single_quote.fmt(f)?;
                }
                opening_double_quote.fmt(f)
            }
            CstKind::ClosingText {
                closing_double_quote,
                closing_single_quotes,
            } => {
                closing_double_quote.fmt(f)?;
                for closing_single_quote in closing_single_quotes {
                    closing_single_quote.fmt(f)?;
                }
                Ok(())
            }
            CstKind::Text {
                opening,
                parts,
                closing,
            } => {
                opening.fmt(f)?;
                for part in parts {
                    part.fmt(f)?;
                }
                closing.fmt(f)
            }
            CstKind::TextPart(literal) => literal.fmt(f),
            CstKind::TextInterpolation {
                opening_curly_braces,
                expression,
                closing_curly_braces,
            } => {
                for opening_curly_brace in opening_curly_braces {
                    opening_curly_brace.fmt(f)?;
                }
                expression.fmt(f)?;
                for closing_curly_brace in closing_curly_braces {
                    closing_curly_brace.fmt(f)?;
                }
                Ok(())
            }
            CstKind::Pipe {
                receiver,
                bar,
                call,
            } => write!(f, "{}{}{}", receiver, bar, call),
            CstKind::Parenthesized {
                opening_parenthesis,
                inner,
                closing_parenthesis,
            } => write!(f, "{}{}{}", opening_parenthesis, inner, closing_parenthesis),
            CstKind::Call {
                receiver,
                arguments,
            } => {
                receiver.fmt(f)?;
                for argument in arguments {
                    argument.fmt(f)?;
                }
                Ok(())
            }
            CstKind::List {
                opening_parenthesis,
                items,
                closing_parenthesis,
            } => {
                opening_parenthesis.fmt(f)?;
                for field in items {
                    field.fmt(f)?;
                }
                closing_parenthesis.fmt(f)
            }
            CstKind::ListItem { value, comma } => {
                value.fmt(f)?;
                if let Some(comma) = comma {
                    comma.fmt(f)?;
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
                key_and_colon,
                value,
                comma,
            } => {
                if let Some(box (key, colon)) = key_and_colon {
                    key.fmt(f)?;
                    colon.fmt(f)?;
                }
                value.fmt(f)?;
                if let Some(comma) = comma {
                    comma.fmt(f)?;
                }
                Ok(())
            }
            CstKind::StructAccess { struct_, dot, key } => {
                struct_.fmt(f)?;
                dot.fmt(f)?;
                key.fmt(f)
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
                name_or_pattern,
                parameters,
                assignment_sign,
                body,
            } => {
                name_or_pattern.fmt(f)?;
                for parameter in parameters {
                    parameter.fmt(f)?;
                }
                assignment_sign.fmt(f)?;
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
            CstKind::Call { receiver, .. } => receiver.display_span(),
            CstKind::Assignment {
                name_or_pattern, ..
            } => name_or_pattern.display_span(),
            _ => self.span.clone(),
        }
    }

    fn is_whitespace(&self) -> bool {
        match &self.kind {
            CstKind::Whitespace(_) | CstKind::Newline(_) | CstKind::Comment { .. } => true,
            CstKind::TrailingWhitespace { child, .. } => child.is_whitespace(),
            _ => false,
        }
    }
}

pub trait UnwrapWhitespaceAndComment {
    fn unwrap_whitespace_and_comment(&self) -> Self;
}
impl UnwrapWhitespaceAndComment for Cst {
    fn unwrap_whitespace_and_comment(&self) -> Self {
        let kind = match &self.kind {
            kind @ (CstKind::EqualsSign
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
            | CstKind::Octothorpe
            | CstKind::Whitespace(_)
            | CstKind::Newline(_)
            | CstKind::Comment { .. }) => kind.clone(),
            CstKind::TrailingWhitespace { child, .. } => {
                return child.unwrap_whitespace_and_comment()
            }
            kind @ (CstKind::Identifier(_) | CstKind::Symbol(_) | CstKind::Int { .. }) => {
                kind.clone()
            }
            CstKind::OpeningText {
                opening_single_quotes,
                opening_double_quote,
            } => CstKind::OpeningText {
                opening_single_quotes: opening_single_quotes.unwrap_whitespace_and_comment(),
                opening_double_quote: Box::new(
                    opening_double_quote.unwrap_whitespace_and_comment(),
                ),
            },
            CstKind::ClosingText {
                closing_double_quote,
                closing_single_quotes,
            } => CstKind::ClosingText {
                closing_double_quote: Box::new(
                    closing_double_quote.unwrap_whitespace_and_comment(),
                ),
                closing_single_quotes: closing_single_quotes.unwrap_whitespace_and_comment(),
            },
            CstKind::Text {
                opening,
                parts,
                closing,
            } => CstKind::Text {
                opening: Box::new(opening.unwrap_whitespace_and_comment()),
                parts: parts.unwrap_whitespace_and_comment(),
                closing: Box::new(closing.unwrap_whitespace_and_comment()),
            },
            kind @ CstKind::TextPart(_) => kind.clone(),
            CstKind::TextInterpolation {
                opening_curly_braces,
                expression,
                closing_curly_braces,
            } => CstKind::TextInterpolation {
                opening_curly_braces: opening_curly_braces.unwrap_whitespace_and_comment(),
                expression: Box::new(expression.unwrap_whitespace_and_comment()),
                closing_curly_braces: closing_curly_braces.unwrap_whitespace_and_comment(),
            },
            CstKind::Pipe {
                receiver,
                bar,
                call,
            } => CstKind::Pipe {
                receiver: Box::new(receiver.unwrap_whitespace_and_comment()),
                bar: Box::new(bar.unwrap_whitespace_and_comment()),
                call: Box::new(call.unwrap_whitespace_and_comment()),
            },
            CstKind::Parenthesized {
                opening_parenthesis,
                inner,
                closing_parenthesis,
            } => CstKind::Parenthesized {
                opening_parenthesis: Box::new(opening_parenthesis.unwrap_whitespace_and_comment()),
                inner: Box::new(inner.unwrap_whitespace_and_comment()),
                closing_parenthesis: Box::new(closing_parenthesis.unwrap_whitespace_and_comment()),
            },
            CstKind::Call {
                receiver,
                arguments,
            } => CstKind::Call {
                receiver: Box::new(receiver.unwrap_whitespace_and_comment()),
                arguments: arguments.unwrap_whitespace_and_comment(),
            },
            CstKind::List {
                opening_parenthesis,
                items,
                closing_parenthesis,
            } => CstKind::List {
                opening_parenthesis: Box::new(opening_parenthesis.unwrap_whitespace_and_comment()),
                items: items.unwrap_whitespace_and_comment(),
                closing_parenthesis: Box::new(closing_parenthesis.unwrap_whitespace_and_comment()),
            },
            CstKind::ListItem { value, comma } => CstKind::ListItem {
                value: Box::new(value.unwrap_whitespace_and_comment()),
                comma: comma
                    .as_ref()
                    .map(|comma| Box::new(comma.unwrap_whitespace_and_comment())),
            },
            CstKind::Struct {
                opening_bracket,
                fields,
                closing_bracket,
            } => CstKind::Struct {
                opening_bracket: Box::new(opening_bracket.unwrap_whitespace_and_comment()),
                fields: fields.unwrap_whitespace_and_comment(),
                closing_bracket: Box::new(closing_bracket.unwrap_whitespace_and_comment()),
            },
            CstKind::StructField {
                key_and_colon,
                value,
                comma,
            } => CstKind::StructField {
                key_and_colon: key_and_colon.as_ref().map(|box (key, colon)| {
                    Box::new((
                        key.unwrap_whitespace_and_comment(),
                        colon.unwrap_whitespace_and_comment(),
                    ))
                }),
                value: Box::new(value.unwrap_whitespace_and_comment()),
                comma: comma
                    .as_ref()
                    .map(|comma| Box::new(comma.unwrap_whitespace_and_comment())),
            },
            CstKind::StructAccess { struct_, dot, key } => CstKind::StructAccess {
                struct_: Box::new(struct_.unwrap_whitespace_and_comment()),
                dot: Box::new(dot.unwrap_whitespace_and_comment()),
                key: Box::new(key.unwrap_whitespace_and_comment()),
            },
            CstKind::Lambda {
                opening_curly_brace,
                parameters_and_arrow,
                body,
                closing_curly_brace,
            } => CstKind::Lambda {
                opening_curly_brace: Box::new(opening_curly_brace.unwrap_whitespace_and_comment()),
                parameters_and_arrow: parameters_and_arrow.as_ref().map(|(parameters, arrow)| {
                    (
                        parameters.unwrap_whitespace_and_comment(),
                        Box::new(arrow.unwrap_whitespace_and_comment()),
                    )
                }),
                body: body.unwrap_whitespace_and_comment(),
                closing_curly_brace: Box::new(closing_curly_brace.unwrap_whitespace_and_comment()),
            },
            CstKind::Assignment {
                name_or_pattern,
                parameters,
                assignment_sign,
                body,
            } => CstKind::Assignment {
                name_or_pattern: Box::new(name_or_pattern.unwrap_whitespace_and_comment()),
                parameters: parameters.unwrap_whitespace_and_comment(),
                assignment_sign: Box::new(assignment_sign.unwrap_whitespace_and_comment()),
                body: body.unwrap_whitespace_and_comment(),
            },
            kind @ CstKind::Error { .. } => kind.clone(),
        };
        Cst {
            id: self.id,
            span: self.span.clone(),
            kind,
        }
    }
}
impl UnwrapWhitespaceAndComment for Vec<Cst> {
    fn unwrap_whitespace_and_comment(&self) -> Self {
        self.iter()
            .filter(|it| !it.is_whitespace())
            .map(|it| it.unwrap_whitespace_and_comment())
            .collect()
    }
}

trait TreeWithIds {
    fn first_id(&self) -> Option<Id>;
    fn find(&self, id: &Id) -> Option<&Cst>;

    fn first_offset(&self) -> Option<usize>;
    fn find_by_offset(&self, offset: &usize) -> Option<&Cst>;
}
impl TreeWithIds for Cst {
    fn first_id(&self) -> Option<Id> {
        Some(self.id)
    }
    fn find(&self, id: &Id) -> Option<&Cst> {
        if id == &self.id {
            return Some(self);
        };

        match &self.kind {
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
            | CstKind::Octothorpe
            | CstKind::Whitespace(_)
            | CstKind::Newline(_) => None,
            CstKind::Comment { octothorpe, .. } => octothorpe.find(id),
            CstKind::TrailingWhitespace { child, whitespace } => {
                child.find(id).or_else(|| whitespace.find(id))
            }
            CstKind::Identifier(_) | CstKind::Symbol(_) | CstKind::Int { .. } => None,
            CstKind::OpeningText {
                opening_single_quotes,
                opening_double_quote,
            } => opening_single_quotes
                .find(id)
                .or_else(|| opening_double_quote.find(id)),
            CstKind::ClosingText {
                closing_double_quote,
                closing_single_quotes,
            } => closing_double_quote
                .find(id)
                .or_else(|| closing_single_quotes.find(id)),
            CstKind::Text {
                opening,
                parts,
                closing,
            } => opening
                .find(id)
                .or_else(|| parts.find(id))
                .or_else(|| closing.find(id)),
            CstKind::TextPart(_) => None,
            CstKind::TextInterpolation {
                opening_curly_braces,
                expression,
                closing_curly_braces,
            } => opening_curly_braces
                .find(id)
                .or_else(|| expression.find(id))
                .or_else(|| closing_curly_braces.find(id)),
            CstKind::Pipe {
                receiver,
                bar,
                call,
            } => receiver
                .find(id)
                .or_else(|| bar.find(id))
                .or_else(|| call.find(id)),
            CstKind::Parenthesized {
                opening_parenthesis,
                inner,
                closing_parenthesis,
            } => opening_parenthesis
                .find(id)
                .or_else(|| inner.find(id))
                .or_else(|| closing_parenthesis.find(id)),
            CstKind::Call {
                receiver,
                arguments,
            } => receiver.find(id).or_else(|| arguments.find(id)),
            CstKind::List {
                opening_parenthesis,
                items,
                closing_parenthesis,
            } => opening_parenthesis
                .find(id)
                .or_else(|| items.find(id))
                .or_else(|| closing_parenthesis.find(id)),
            CstKind::ListItem { value, comma } => value
                .find(id)
                .or_else(|| comma.as_ref().and_then(|comma| comma.find(id))),
            CstKind::Struct {
                opening_bracket,
                fields,
                closing_bracket,
            } => opening_bracket
                .find(id)
                .or_else(|| fields.find(id))
                .or_else(|| closing_bracket.find(id)),
            CstKind::StructField {
                key_and_colon,
                value,
                comma,
            } => key_and_colon
                .as_ref()
                .and_then(|box (key, colon)| key.find(id).or_else(|| colon.find(id)))
                .or_else(|| value.find(id))
                .or_else(|| comma.as_ref().and_then(|comma| comma.find(id))),
            CstKind::StructAccess { struct_, dot, key } => struct_
                .find(id)
                .or_else(|| dot.find(id))
                .or_else(|| key.find(id)),
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
                name_or_pattern,
                parameters,
                assignment_sign,
                body,
            } => name_or_pattern
                .find(id)
                .or_else(|| parameters.find(id))
                .or_else(|| assignment_sign.find(id))
                .or_else(|| body.find(id)),
            CstKind::Error { .. } => None,
        }
    }

    fn first_offset(&self) -> Option<usize> {
        Some(self.span.start)
    }
    fn find_by_offset(&self, offset: &usize) -> Option<&Cst> {
        let (inner, is_end_inclusive) = match &self.kind {
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
            | CstKind::Octothorpe
            | CstKind::Whitespace(_)
            | CstKind::Newline(_) => (None, false),
            CstKind::Comment { octothorpe, .. } => (octothorpe.find_by_offset(offset), true),
            CstKind::TrailingWhitespace { child, .. } => (child.find_by_offset(offset), false),
            CstKind::Identifier { .. } | CstKind::Symbol { .. } | CstKind::Int { .. } => {
                (None, true)
            }
            CstKind::Text { .. }
            | CstKind::OpeningText { .. }
            | CstKind::ClosingText { .. }
            | CstKind::TextPart(_)
            | CstKind::TextInterpolation { .. } => (None, false),
            CstKind::Pipe { receiver, call, .. } => (
                receiver
                    .find_by_offset(offset)
                    .or_else(|| call.find_by_offset(offset)),
                false,
            ),
            CstKind::Parenthesized { inner, .. } => (inner.find_by_offset(offset), false),
            CstKind::Call {
                receiver,
                arguments,
            } => (
                receiver
                    .find_by_offset(offset)
                    .or_else(|| arguments.find_by_offset(offset)),
                false,
            ),
            CstKind::List {
                opening_parenthesis,
                items,
                closing_parenthesis,
            } => (
                opening_parenthesis
                    .find_by_offset(offset)
                    .or_else(|| items.find_by_offset(offset))
                    .or_else(|| closing_parenthesis.find_by_offset(offset)),
                false,
            ),
            CstKind::ListItem { value, comma } => (
                value
                    .find_by_offset(offset)
                    .or_else(|| comma.find_by_offset(offset)),
                false,
            ),
            CstKind::Struct {
                opening_bracket,
                fields,
                closing_bracket,
            } => (
                opening_bracket
                    .find_by_offset(offset)
                    .or_else(|| fields.find_by_offset(offset))
                    .or_else(|| closing_bracket.find_by_offset(offset)),
                false,
            ),
            CstKind::StructField {
                key_and_colon,
                value,
                comma,
            } => (
                key_and_colon
                    .as_ref()
                    .and_then(|box (key, colon)| {
                        key.find_by_offset(offset)
                            .or_else(|| colon.find_by_offset(offset))
                    })
                    .or_else(|| value.find_by_offset(offset))
                    .or_else(|| comma.find_by_offset(offset)),
                false,
            ),
            CstKind::StructAccess { struct_, dot, key } => (
                struct_
                    .find_by_offset(offset)
                    .or_else(|| dot.find_by_offset(offset))
                    .or_else(|| key.find_by_offset(offset)),
                false,
            ),
            CstKind::Lambda { body, .. } => (body.find_by_offset(offset), false),
            CstKind::Assignment {
                name_or_pattern,
                parameters,
                assignment_sign,
                body,
            } => (
                name_or_pattern
                    .find_by_offset(offset)
                    .or_else(|| parameters.find_by_offset(offset))
                    .or_else(|| assignment_sign.find_by_offset(offset))
                    .or_else(|| body.find_by_offset(offset)),
                false,
            ),
            CstKind::Error { .. } => (None, false),
        };

        inner.or_else(|| {
            if self.span.contains(offset) || (is_end_inclusive && &self.span.end == offset) {
                Some(self)
            } else {
                None
            }
        })
    }
}
impl<T: TreeWithIds> TreeWithIds for Option<T> {
    fn first_id(&self) -> Option<Id> {
        self.as_ref().and_then(|it| it.first_id())
    }
    fn find(&self, id: &Id) -> Option<&Cst> {
        self.as_ref().and_then(|it| it.find(id))
    }

    fn first_offset(&self) -> Option<usize> {
        self.as_ref().and_then(|it| it.first_offset())
    }
    fn find_by_offset(&self, offset: &usize) -> Option<&Cst> {
        self.as_ref().and_then(|it| it.find_by_offset(offset))
    }
}
impl<T: TreeWithIds> TreeWithIds for Box<T> {
    fn first_id(&self) -> Option<Id> {
        self.as_ref().first_id()
    }
    fn find(&self, id: &Id) -> Option<&Cst> {
        self.as_ref().find(id)
    }

    fn first_offset(&self) -> Option<usize> {
        self.as_ref().first_offset()
    }
    fn find_by_offset(&self, offset: &usize) -> Option<&Cst> {
        self.as_ref().find_by_offset(offset)
    }
}
impl<T: TreeWithIds> TreeWithIds for [T] {
    fn first_id(&self) -> Option<Id> {
        self.iter()
            .map(|it| it.first_id())
            .filter_map(Some)
            .next()
            .flatten()
    }
    fn find(&self, id: &Id) -> Option<&Cst> {
        let child_index = self
            .binary_search_by_key(id, |it| it.first_id().unwrap())
            .or_else(|err| if err == 0 { Err(()) } else { Ok(err - 1) })
            .ok()?;
        self[child_index].find(id)
    }

    fn first_offset(&self) -> Option<usize> {
        self.iter()
            .map(|it| it.first_offset())
            .filter_map(Some)
            .next()
            .flatten()
    }
    fn find_by_offset(&self, offset: &usize) -> Option<&Cst> {
        let child_index = self
            .binary_search_by_key(offset, |it| it.first_offset().unwrap())
            .or_else(|err| if err == 0 { Err(()) } else { Ok(err - 1) })
            .ok()?;
        self[child_index].find_by_offset(offset)
    }
}
