use crate::{error::CompilerError, impl_countable_id, position::Offset};
use derive_more::Deref;
use std::{
    fmt::{self, Debug, Formatter},
    ops::Range,
    path::Path,
};

#[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Id(pub usize);
impl_countable_id!(Id);

impl Debug for Id {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "${}", self.0)
    }
}

#[derive(Clone, Debug, Deref, Eq, Hash, PartialEq)]
pub struct Cst<D = CstData> {
    pub data: D,
    #[deref]
    pub kind: CstKind<D>,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct CstData {
    pub id: Id,
    pub span: Range<Offset>,
}

impl Cst {
    /// Returns a span that makes sense to display in the editor.
    ///
    /// For example, if a call contains errors, we want to only underline the
    /// name of the called function itself, not everything including arguments.
    #[must_use]
    pub fn display_span(&self) -> Range<Offset> {
        match &self.kind {
            CstKind::TrailingWhitespace { child, .. } => child.display_span(),
            CstKind::Call { receiver, .. } => receiver.display_span(),
            CstKind::Assignment { name, .. } => name.display_span(),
            _ => self.data.span.clone(),
        }
    }
}

// TODO: Make the CST more typed? E.g., separate enums for expressions, patterns,
// and a wrapper that stores trailing whitespace and optionally an error.

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum CstKind<D = CstData> {
    EqualsSign,         // =
    Comma,              // ,
    Dot,                // .
    Colon,              // :
    ColonEqualsSign,    // :=
    OpeningParenthesis, // (
    ClosingParenthesis, // )
    OpeningBracket,     // [
    ClosingBracket,     // ]
    OpeningCurlyBrace,  // {
    ClosingCurlyBrace,  // }
    Arrow,              // ->
    DoubleQuote,        // "
    Octothorpe,         // #
    Let,                // let
    Whitespace(String),
    Comment {
        octothorpe: Box<Cst<D>>,
        comment: String,
    },
    TrailingWhitespace {
        child: Box<Cst<D>>,
        whitespace: Vec<Cst<D>>,
    },
    Identifier(String),
    Symbol(String),
    Int {
        value: i64,
        string: String,
    },
    Text {
        opening_double_quote: Box<Cst<D>>,
        parts: Vec<Cst<D>>,
        closing_double_quote: Box<Cst<D>>,
    },
    TextPart(String),
    TextInterpolation {
        opening_curly_brace: Box<Cst<D>>,
        expression: Box<Cst<D>>,
        closing_curly_brace: Box<Cst<D>>,
    },
    Parenthesized {
        opening_parenthesis: Box<Cst<D>>,
        inner: Box<Cst<D>>,
        closing_parenthesis: Box<Cst<D>>,
    },
    Call {
        receiver: Box<Cst<D>>,
        opening_parenthesis: Box<Cst<D>>,
        arguments: Vec<Cst<D>>,
        closing_parenthesis: Box<Cst<D>>,
    },
    CallArgument {
        value: Box<Cst<D>>,
        comma: Option<Box<Cst<D>>>,
    },
    Struct {
        opening_bracket: Box<Cst<D>>,
        fields: Vec<Cst<D>>,
        closing_bracket: Box<Cst<D>>,
    },
    StructField {
        key: Box<Cst<D>>,
        colon: Box<Cst<D>>,
        value: Box<Cst<D>>,
        comma: Option<Box<Cst<D>>>,
    },
    StructAccess {
        struct_: Box<Cst<D>>,
        dot: Box<Cst<D>>,
        key: Box<Cst<D>>,
    },
    Lambda {
        opening_curly_brace: Box<Cst<D>>,
        parameters_and_arrow: Option<CstLambdaParametersAndArrow<D>>,
        body: Vec<Cst<D>>,
        closing_curly_brace: Box<Cst<D>>,
    },
    Assignment {
        let_keyword: Box<Cst<D>>,
        name: Box<Cst<D>>,
        kind: Box<Cst<D>>,
        assignment_sign: Box<Cst<D>>,
        body: Box<Cst<D>>,
    },
    AssignmentValue {
        colon_and_type: Option<Box<(Cst<D>, Cst<D>)>>,
    },
    AssignmentFunction {
        opening_parenthesis: Box<Cst<D>>,
        parameters: Vec<Cst<D>>,
        closing_parenthesis: Box<Cst<D>>,
        arrow: Box<Cst<D>>,
        return_type: Box<Cst<D>>,
    },
    Parameter {
        name: Box<Cst<D>>,
        colon: Box<Cst<D>>,
        type_: Box<Cst<D>>,
        comma: Option<Box<Cst<D>>>,
    },
    Error {
        unparsable_input: String,
        error: String,
    },
}
pub type CstLambdaParametersAndArrow<D = CstData> = (Vec<Cst<D>>, Box<Cst<D>>);

pub trait CollectCstErrors {
    fn collect_errors(&self, file: &Path) -> Vec<CompilerError> {
        let mut errors = vec![];
        self.collect_errors_to(file, &mut errors);
        errors
    }
    fn collect_errors_to(&self, file: &Path, errors: &mut Vec<CompilerError>);
}
impl<C: CollectCstErrors> CollectCstErrors for Vec<C> {
    fn collect_errors_to(&self, file: &Path, errors: &mut Vec<CompilerError>) {
        for cst in self {
            cst.collect_errors_to(file, errors);
        }
    }
}
impl<C: CollectCstErrors> CollectCstErrors for Option<C> {
    fn collect_errors_to(&self, file: &Path, errors: &mut Vec<CompilerError>) {
        if let Some(cst) = self {
            cst.collect_errors_to(file, errors);
        }
    }
}
impl<C: CollectCstErrors> CollectCstErrors for Box<C> {
    fn collect_errors_to(&self, file: &Path, errors: &mut Vec<CompilerError>) {
        self.as_ref().collect_errors_to(file, errors);
    }
}
impl<C0: CollectCstErrors, C1: CollectCstErrors> CollectCstErrors for (C0, C1) {
    fn collect_errors_to(&self, file: &Path, errors: &mut Vec<CompilerError>) {
        self.0.collect_errors_to(file, errors);
        self.1.collect_errors_to(file, errors);
    }
}
impl CollectCstErrors for Cst {
    fn collect_errors_to(&self, file: &Path, errors: &mut Vec<CompilerError>) {
        match &self.kind {
            CstKind::EqualsSign
            | CstKind::Comma
            | CstKind::Dot
            | CstKind::Colon
            | CstKind::ColonEqualsSign
            | CstKind::OpeningParenthesis
            | CstKind::ClosingParenthesis
            | CstKind::OpeningBracket
            | CstKind::ClosingBracket
            | CstKind::OpeningCurlyBrace
            | CstKind::ClosingCurlyBrace
            | CstKind::Arrow
            | CstKind::DoubleQuote
            | CstKind::Octothorpe
            | CstKind::Let
            | CstKind::Whitespace(_) => {}
            CstKind::Comment {
                octothorpe,
                comment: _,
            } => {
                octothorpe.collect_errors_to(file, errors);
            }
            CstKind::TrailingWhitespace { child, whitespace } => {
                child.collect_errors_to(file, errors);
                whitespace.collect_errors_to(file, errors);
            }
            CstKind::Identifier(_)
            | CstKind::Symbol(_)
            | CstKind::Int {
                value: _,
                string: _,
            } => {}
            CstKind::Text {
                opening_double_quote,
                parts,
                closing_double_quote,
            } => {
                opening_double_quote.collect_errors_to(file, errors);
                parts.collect_errors_to(file, errors);
                closing_double_quote.collect_errors_to(file, errors);
            }
            CstKind::TextPart(_) => {}
            CstKind::TextInterpolation {
                opening_curly_brace,
                expression,
                closing_curly_brace,
            } => {
                opening_curly_brace.collect_errors_to(file, errors);
                expression.collect_errors_to(file, errors);
                closing_curly_brace.collect_errors_to(file, errors);
            }
            CstKind::Parenthesized {
                opening_parenthesis,
                inner,
                closing_parenthesis,
            } => {
                opening_parenthesis.collect_errors_to(file, errors);
                inner.collect_errors_to(file, errors);
                closing_parenthesis.collect_errors_to(file, errors);
            }
            CstKind::Call {
                receiver,
                opening_parenthesis,
                arguments,
                closing_parenthesis,
            } => {
                receiver.collect_errors_to(file, errors);
                opening_parenthesis.collect_errors_to(file, errors);
                arguments.collect_errors_to(file, errors);
                closing_parenthesis.collect_errors_to(file, errors);
            }
            CstKind::CallArgument { value, comma } => {
                value.collect_errors_to(file, errors);
                comma.collect_errors_to(file, errors);
            }
            CstKind::Struct {
                opening_bracket,
                fields,
                closing_bracket,
            } => {
                opening_bracket.collect_errors_to(file, errors);
                fields.collect_errors_to(file, errors);
                closing_bracket.collect_errors_to(file, errors);
            }
            CstKind::StructField {
                key,
                colon,
                value,
                comma,
            } => {
                key.collect_errors_to(file, errors);
                colon.collect_errors_to(file, errors);
                value.collect_errors_to(file, errors);
                comma.collect_errors_to(file, errors);
            }
            CstKind::StructAccess { struct_, dot, key } => {
                struct_.collect_errors_to(file, errors);
                dot.collect_errors_to(file, errors);
                key.collect_errors_to(file, errors);
            }
            CstKind::Lambda {
                opening_curly_brace,
                parameters_and_arrow,
                body,
                closing_curly_brace,
            } => {
                opening_curly_brace.collect_errors_to(file, errors);
                parameters_and_arrow.collect_errors_to(file, errors);
                body.collect_errors_to(file, errors);
                closing_curly_brace.collect_errors_to(file, errors);
            }
            CstKind::Assignment {
                let_keyword,
                name,
                kind,
                assignment_sign,
                body,
            } => {
                let_keyword.collect_errors_to(file, errors);
                name.collect_errors_to(file, errors);
                kind.collect_errors_to(file, errors);
                assignment_sign.collect_errors_to(file, errors);
                body.collect_errors_to(file, errors);
            }
            CstKind::AssignmentValue { colon_and_type } => {
                colon_and_type.collect_errors_to(file, errors);
            }
            CstKind::AssignmentFunction {
                opening_parenthesis,
                parameters,
                closing_parenthesis,
                arrow,
                return_type,
            } => {
                opening_parenthesis.collect_errors_to(file, errors);
                parameters.collect_errors_to(file, errors);
                closing_parenthesis.collect_errors_to(file, errors);
                arrow.collect_errors_to(file, errors);
                return_type.collect_errors_to(file, errors);
            }
            CstKind::Parameter {
                name,
                colon,
                type_,
                comma,
            } => {
                name.collect_errors_to(file, errors);
                colon.collect_errors_to(file, errors);
                type_.collect_errors_to(file, errors);
                comma.collect_errors_to(file, errors);
            }
            CstKind::Error {
                unparsable_input,
                error,
            } => {
                errors.push(CompilerError {
                    path: file.to_owned(),
                    span: self.data.span.clone(),
                    message: format!("{}: {}", unparsable_input, error),
                });
            }
        }
    }
}
