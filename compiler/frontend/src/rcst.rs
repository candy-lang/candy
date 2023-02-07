use std::fmt::{self, Display, Formatter};

use num_bigint::BigUint;

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub enum Rcst {
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
    Percent,            // %
    Octothorpe,         // #
    Whitespace(String), // contains only non-multiline whitespace
    Newline(String), // the associated `String` because some systems (such as Windows) have weird newlines
    Comment {
        octothorpe: Box<Rcst>,
        comment: String,
    },
    TrailingWhitespace {
        child: Box<Rcst>,
        whitespace: Vec<Rcst>,
    },
    Identifier(String),
    Symbol(String),
    Int {
        value: BigUint,
        string: String,
    },
    OpeningText {
        opening_single_quotes: Vec<Rcst>,
        opening_double_quote: Box<Rcst>,
    },
    ClosingText {
        closing_double_quote: Box<Rcst>,
        closing_single_quotes: Vec<Rcst>,
    },
    Text {
        opening: Box<Rcst>,
        parts: Vec<Rcst>,
        closing: Box<Rcst>,
    },
    TextPart(String),
    TextInterpolation {
        opening_curly_braces: Vec<Rcst>,
        expression: Box<Rcst>,
        closing_curly_braces: Vec<Rcst>,
    },
    Pipe {
        receiver: Box<Rcst>,
        bar: Box<Rcst>,
        call: Box<Rcst>,
    },
    Parenthesized {
        opening_parenthesis: Box<Rcst>,
        inner: Box<Rcst>,
        closing_parenthesis: Box<Rcst>,
    },
    Call {
        receiver: Box<Rcst>,
        arguments: Vec<Rcst>,
    },
    List {
        opening_parenthesis: Box<Rcst>,
        items: Vec<Rcst>,
        closing_parenthesis: Box<Rcst>,
    },
    ListItem {
        value: Box<Rcst>,
        comma: Option<Box<Rcst>>,
    },
    Struct {
        opening_bracket: Box<Rcst>,
        fields: Vec<Rcst>,
        closing_bracket: Box<Rcst>,
    },
    StructField {
        key_and_colon: Option<Box<(Rcst, Rcst)>>,
        value: Box<Rcst>,
        comma: Option<Box<Rcst>>,
    },
    StructAccess {
        struct_: Box<Rcst>,
        dot: Box<Rcst>,
        key: Box<Rcst>,
    },
    Match {
        expression: Box<Rcst>,
        percent: Box<Rcst>,
        cases: Vec<Rcst>,
    },
    MatchCase {
        pattern: Box<Rcst>,
        arrow: Box<Rcst>,
        body: Vec<Rcst>,
    },
    OrPattern {
        left: Box<Rcst>,
        right: Vec<(Rcst, Rcst)>,
    },
    Lambda {
        opening_curly_brace: Box<Rcst>,
        parameters_and_arrow: Option<(Vec<Rcst>, Box<Rcst>)>,
        body: Vec<Rcst>,
        closing_curly_brace: Box<Rcst>,
    },
    Assignment {
        name_or_pattern: Box<Rcst>,
        parameters: Vec<Rcst>,
        assignment_sign: Box<Rcst>,
        body: Vec<Rcst>,
    },

    Error {
        unparsable_input: String,
        error: RcstError,
    },
}

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub enum RcstError {
    CurlyBraceNotClosed,
    IdentifierContainsNonAlphanumericAscii,
    IntContainsNonDigits,
    ListItemMissesValue,
    ListNotClosed,
    MatchMissesCases,
    MatchCaseMissesArrow,
    MatchCaseMissesBody,
    OpeningParenthesisWithoutExpression,
    OrPatternMissesRight,
    ParenthesisNotClosed,
    PipeMissesCall,
    StructFieldMissesColon,
    StructFieldMissesKey,
    StructFieldMissesValue,
    StructNotClosed,
    SymbolContainsNonAlphanumericAscii,
    TextNotClosed,
    TextNotSufficientlyIndented,
    TextInterpolationNotClosed,
    TextInterpolationWithoutExpression,
    TooMuchWhitespace,
    UnexpectedCharacters,
    UnparsedRest,
    WeirdWhitespace,
    WeirdWhitespaceInIndentation,
}

impl Display for Rcst {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Rcst::EqualsSign => "=".fmt(f),
            Rcst::Comma => ",".fmt(f),
            Rcst::Dot => ".".fmt(f),
            Rcst::Colon => ":".fmt(f),
            Rcst::ColonEqualsSign => ":=".fmt(f),
            Rcst::Bar => "|".fmt(f),
            Rcst::OpeningParenthesis => "(".fmt(f),
            Rcst::ClosingParenthesis => ")".fmt(f),
            Rcst::OpeningBracket => "[".fmt(f),
            Rcst::ClosingBracket => "]".fmt(f),
            Rcst::OpeningCurlyBrace => "{".fmt(f),
            Rcst::ClosingCurlyBrace => "}".fmt(f),
            Rcst::Arrow => "->".fmt(f),
            Rcst::SingleQuote => "'".fmt(f),
            Rcst::DoubleQuote => '"'.fmt(f),
            Rcst::Percent => '%'.fmt(f),
            Rcst::Octothorpe => "#".fmt(f),
            Rcst::Whitespace(whitespace) => whitespace.fmt(f),
            Rcst::Newline(newline) => newline.fmt(f),
            Rcst::Comment {
                octothorpe,
                comment,
            } => {
                octothorpe.fmt(f)?;
                comment.fmt(f)
            }
            Rcst::TrailingWhitespace { child, whitespace } => {
                child.fmt(f)?;
                for w in whitespace {
                    w.fmt(f)?;
                }
                Ok(())
            }
            Rcst::Identifier(identifier) => identifier.fmt(f),
            Rcst::Symbol(symbol) => symbol.fmt(f),
            Rcst::Int { string, .. } => string.fmt(f),
            Rcst::OpeningText {
                opening_single_quotes,
                opening_double_quote,
            } => {
                for opening_single_quote in opening_single_quotes {
                    opening_single_quote.fmt(f)?;
                }
                opening_double_quote.fmt(f)
            }
            Rcst::ClosingText {
                closing_double_quote,
                closing_single_quotes,
            } => {
                closing_double_quote.fmt(f)?;
                for closing_single_quote in closing_single_quotes {
                    closing_single_quote.fmt(f)?;
                }
                Ok(())
            }
            Rcst::Text {
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
            Rcst::TextPart(literal) => literal.fmt(f),
            Rcst::TextInterpolation {
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
            Rcst::Pipe {
                receiver,
                bar,
                call,
            } => {
                receiver.fmt(f)?;
                bar.fmt(f)?;
                call.fmt(f)
            }
            Rcst::Parenthesized {
                opening_parenthesis,
                inner,
                closing_parenthesis,
            } => {
                opening_parenthesis.fmt(f)?;
                inner.fmt(f)?;
                closing_parenthesis.fmt(f)
            }
            Rcst::Call {
                receiver,
                arguments,
            } => {
                receiver.fmt(f)?;
                for argument in arguments {
                    argument.fmt(f)?;
                }
                Ok(())
            }
            Rcst::List {
                opening_parenthesis,
                items,
                closing_parenthesis,
            } => {
                opening_parenthesis.fmt(f)?;
                for item in items {
                    item.fmt(f)?;
                }
                closing_parenthesis.fmt(f)
            }
            Rcst::ListItem { value, comma } => {
                value.fmt(f)?;
                if let Some(comma) = comma {
                    comma.fmt(f)?;
                }
                Ok(())
            }
            Rcst::Struct {
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
            Rcst::StructField {
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
            Rcst::StructAccess { struct_, dot, key } => {
                struct_.fmt(f)?;
                dot.fmt(f)?;
                key.fmt(f)
            }
            Rcst::Match {
                expression,
                percent,
                cases,
            } => {
                expression.fmt(f)?;
                percent.fmt(f)?;
                for case in cases {
                    case.fmt(f)?;
                }
                Ok(())
            }
            Rcst::MatchCase {
                pattern,
                arrow,
                body,
            } => {
                pattern.fmt(f)?;
                arrow.fmt(f)?;
                for expression in body {
                    expression.fmt(f)?;
                }
                Ok(())
            }
            Rcst::OrPattern { left, right } => {
                left.fmt(f)?;
                for (bar, right) in right {
                    bar.fmt(f)?;
                    right.fmt(f)?;
                }
                Ok(())
            }
            Rcst::Lambda {
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
            Rcst::Assignment {
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
            Rcst::Error {
                unparsable_input, ..
            } => unparsable_input.fmt(f),
        }
    }
}

pub trait IsMultiline {
    fn is_multiline(&self) -> bool;
}

impl IsMultiline for Rcst {
    fn is_multiline(&self) -> bool {
        match self {
            Rcst::EqualsSign => false,
            Rcst::Comma => false,
            Rcst::Dot => false,
            Rcst::Colon => false,
            Rcst::ColonEqualsSign => false,
            Rcst::Bar => false,
            Rcst::OpeningParenthesis => false,
            Rcst::ClosingParenthesis => false,
            Rcst::OpeningBracket => false,
            Rcst::ClosingBracket => false,
            Rcst::OpeningCurlyBrace => false,
            Rcst::ClosingCurlyBrace => false,
            Rcst::Arrow => false,
            Rcst::SingleQuote => false,
            Rcst::DoubleQuote => false,
            Rcst::Percent => false,
            Rcst::Octothorpe => false,
            Rcst::Whitespace(_) => false,
            Rcst::Newline(_) => true,
            Rcst::Comment { .. } => false,
            Rcst::TrailingWhitespace { child, whitespace } => {
                child.is_multiline() || whitespace.is_multiline()
            }
            Rcst::Identifier(_) => false,
            Rcst::Symbol(_) => false,
            Rcst::Int { .. } => false,
            Rcst::OpeningText { .. } => false,
            Rcst::ClosingText { .. } => false,
            Rcst::Text {
                opening,
                parts,
                closing,
            } => opening.is_multiline() || parts.is_multiline() || closing.is_multiline(),
            Rcst::TextPart(_) => false,
            Rcst::TextInterpolation { .. } => false,
            Rcst::Pipe {
                receiver,
                bar,
                call,
            } => receiver.is_multiline() || bar.is_multiline() || call.is_multiline(),
            Rcst::Parenthesized {
                opening_parenthesis,
                inner,
                closing_parenthesis,
            } => {
                opening_parenthesis.is_multiline()
                    || inner.is_multiline()
                    || closing_parenthesis.is_multiline()
            }
            Rcst::Call {
                receiver,
                arguments,
            } => receiver.is_multiline() || arguments.is_multiline(),
            Rcst::List {
                opening_parenthesis,
                items,
                closing_parenthesis,
            } => {
                opening_parenthesis.is_multiline()
                    || items.is_multiline()
                    || closing_parenthesis.is_multiline()
            }
            Rcst::ListItem { value, comma } => {
                value.is_multiline()
                    || comma
                        .as_ref()
                        .map(|comma| comma.is_multiline())
                        .unwrap_or(false)
            }
            Rcst::Struct {
                opening_bracket,
                fields,
                closing_bracket,
            } => {
                opening_bracket.is_multiline()
                    || fields.is_multiline()
                    || closing_bracket.is_multiline()
            }
            Rcst::StructField {
                key_and_colon,
                value,
                comma,
            } => {
                key_and_colon
                    .as_ref()
                    .map(|box (key, colon)| key.is_multiline() || colon.is_multiline())
                    .unwrap_or(false)
                    || value.is_multiline()
                    || comma
                        .as_ref()
                        .map(|comma| comma.is_multiline())
                        .unwrap_or(false)
            }
            Rcst::StructAccess { struct_, dot, key } => {
                struct_.is_multiline() || dot.is_multiline() || key.is_multiline()
            }
            Rcst::Match {
                expression,
                percent,
                cases,
            } => expression.is_multiline() || percent.is_multiline() || cases.is_multiline(),
            Rcst::MatchCase {
                pattern,
                arrow,
                body,
            } => pattern.is_multiline() || arrow.is_multiline() || body.is_multiline(),
            Rcst::OrPattern { left, right } => {
                left.is_multiline()
                    || right
                        .iter()
                        .any(|(bar, right)| bar.is_multiline() || right.is_multiline())
            }
            Rcst::Lambda {
                opening_curly_brace,
                parameters_and_arrow,
                body,
                closing_curly_brace,
            } => {
                opening_curly_brace.is_multiline()
                    || parameters_and_arrow
                        .as_ref()
                        .map(|(parameters, arrow)| {
                            parameters.is_multiline() || arrow.is_multiline()
                        })
                        .unwrap_or(false)
                    || body.is_multiline()
                    || closing_curly_brace.is_multiline()
            }
            Rcst::Assignment {
                name_or_pattern,
                parameters,
                assignment_sign,
                body,
            } => {
                name_or_pattern.is_multiline()
                    || parameters.is_multiline()
                    || assignment_sign.is_multiline()
                    || body.is_multiline()
            }
            Rcst::Error {
                unparsable_input, ..
            } => unparsable_input.is_multiline(),
        }
    }
}

impl IsMultiline for str {
    fn is_multiline(&self) -> bool {
        self.contains('\n')
    }
}

impl IsMultiline for Vec<Rcst> {
    fn is_multiline(&self) -> bool {
        self.iter().any(|cst| cst.is_multiline())
    }
}

impl<T: IsMultiline> IsMultiline for Option<T> {
    fn is_multiline(&self) -> bool {
        match self {
            Some(it) => it.is_multiline(),
            None => false,
        }
    }
}

impl<A: IsMultiline, B: IsMultiline> IsMultiline for (A, B) {
    fn is_multiline(&self) -> bool {
        self.0.is_multiline() || self.1.is_multiline()
    }
}

pub trait SplitOuterTrailingWhitespace {
    fn split_outer_trailing_whitespace(self) -> (Vec<Rcst>, Self);
}
impl SplitOuterTrailingWhitespace for Rcst {
    fn split_outer_trailing_whitespace(self) -> (Vec<Rcst>, Self) {
        match self {
            Rcst::TrailingWhitespace { child, whitespace } => (whitespace, *child),
            _ => (vec![], self),
        }
    }
}

impl<A: SplitOuterTrailingWhitespace> SplitOuterTrailingWhitespace for Vec<A> {
    fn split_outer_trailing_whitespace(mut self) -> (Vec<Rcst>, Self) {
        match self.pop() {
            Some(last) => {
                let (whitespace, last) = last.split_outer_trailing_whitespace();
                self.push(last);
                (whitespace, self)
            }
            None => (vec![], vec![]),
        }
    }
}

impl<T: SplitOuterTrailingWhitespace> SplitOuterTrailingWhitespace for Option<T> {
    fn split_outer_trailing_whitespace(self) -> (Vec<Rcst>, Self) {
        match self {
            Some(it) => {
                let (whitespace, it) = it.split_outer_trailing_whitespace();
                (whitespace, Some(it))
            }
            None => (vec![], None),
        }
    }
}

impl<A: SplitOuterTrailingWhitespace, B: SplitOuterTrailingWhitespace> SplitOuterTrailingWhitespace
    for (A, Vec<B>)
{
    fn split_outer_trailing_whitespace(self) -> (Vec<Rcst>, Self) {
        let (left, right) = self;
        if right.is_empty() {
            let (whitespace, first) = left.split_outer_trailing_whitespace();
            (whitespace, (first, right))
        } else {
            let (whitespace, second) = right.split_outer_trailing_whitespace();
            (whitespace, (left, second))
        }
    }
}
