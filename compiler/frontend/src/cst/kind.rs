use super::{Cst, CstData, CstError};
use num_bigint::BigUint;
use std::fmt::{self, Display, Formatter};
use strum_macros::EnumIs;

#[derive(Clone, Debug, EnumIs, Eq, Hash, PartialEq)]
pub enum CstKind<D = CstData> {
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
        radix_prefix: Option<(IntRadix, String)>,
        value: BigUint,
        string: String,
    },
    OpeningText {
        opening_single_quotes: Vec<Cst<D>>,
        opening_double_quote: Box<Cst<D>>,
    },
    ClosingText {
        closing_double_quote: Box<Cst<D>>,
        closing_single_quotes: Vec<Cst<D>>,
    },
    Text {
        opening: Box<Cst<D>>,
        parts: Vec<Cst<D>>,
        closing: Box<Cst<D>>,
    },
    TextNewline(String), // special newline for text because line breaks have semantic meaning there
    TextPart(String),
    TextInterpolation {
        opening_curly_braces: Vec<Cst<D>>,
        expression: Box<Cst<D>>,
        closing_curly_braces: Vec<Cst<D>>,
    },
    BinaryBar {
        left: Box<Cst<D>>,
        bar: Box<Cst<D>>,
        right: Box<Cst<D>>,
    },
    Parenthesized {
        opening_parenthesis: Box<Cst<D>>,
        inner: Box<Cst<D>>,
        closing_parenthesis: Box<Cst<D>>,
    },
    Call {
        receiver: Box<Cst<D>>,
        arguments: Vec<Cst<D>>,
    },
    List {
        opening_parenthesis: Box<Cst<D>>,
        items: Vec<Cst<D>>,
        closing_parenthesis: Box<Cst<D>>,
    },
    ListItem {
        value: Box<Cst<D>>,
        comma: Option<Box<Cst<D>>>,
    },
    Struct {
        opening_bracket: Box<Cst<D>>,
        fields: Vec<Cst<D>>,
        closing_bracket: Box<Cst<D>>,
    },
    StructField {
        key_and_colon: Option<Box<(Cst<D>, Cst<D>)>>,
        value: Box<Cst<D>>,
        comma: Option<Box<Cst<D>>>,
    },
    StructAccess {
        struct_: Box<Cst<D>>,
        dot: Box<Cst<D>>,
        key: Box<Cst<D>>,
    },
    Match {
        expression: Box<Cst<D>>,
        percent: Box<Cst<D>>,
        cases: Vec<Cst<D>>,
    },
    MatchCase {
        pattern: Box<Cst<D>>,
        arrow: Box<Cst<D>>,
        body: Vec<Cst<D>>,
    },
    Function {
        opening_curly_brace: Box<Cst<D>>,
        parameters_and_arrow: Option<FunctionParametersAndArrow<D>>,
        body: Vec<Cst<D>>,
        closing_curly_brace: Box<Cst<D>>,
    },
    Assignment {
        left: Box<Cst<D>>,
        assignment_sign: Box<Cst<D>>,
        body: Vec<Cst<D>>,
    },
    Error {
        unparsable_input: String,
        error: CstError,
    },
}
#[derive(Clone, Debug, EnumIs, Eq, Hash, PartialEq)]
pub enum IntRadix {
    Binary,
    Hexadecimal,
}
pub type FunctionParametersAndArrow<D> = (Vec<Cst<D>>, Box<Cst<D>>);

impl<D> CstKind<D> {
    #[must_use]
    pub fn is_whitespace_or_comment(&self) -> bool {
        match self {
            Self::Whitespace(_) | Self::Newline(_) | Self::Comment { .. } => true,
            Self::TrailingWhitespace { child, .. } => (**child).is_whitespace_or_comment(),
            _ => false,
        }
    }

    #[must_use]
    pub fn children(&self) -> Vec<&Cst<D>> {
        match self {
            Self::EqualsSign
            | Self::Comma
            | Self::Dot
            | Self::Colon
            | Self::ColonEqualsSign
            | Self::Bar
            | Self::OpeningParenthesis
            | Self::ClosingParenthesis
            | Self::OpeningBracket
            | Self::ClosingBracket
            | Self::OpeningCurlyBrace
            | Self::ClosingCurlyBrace
            | Self::Arrow
            | Self::SingleQuote
            | Self::DoubleQuote
            | Self::Percent
            | Self::Octothorpe
            | Self::Whitespace(_)
            | Self::Newline(_) => vec![],
            Self::Comment { octothorpe, .. } => vec![octothorpe],
            Self::TrailingWhitespace { child, whitespace } => {
                let mut children = vec![child.as_ref()];
                children.extend(whitespace);
                children
            }
            Self::Identifier(_) | Self::Symbol(_) | Self::Int { .. } => vec![],
            Self::OpeningText {
                opening_single_quotes,
                opening_double_quote,
            } => {
                let mut children = vec![];
                children.extend(opening_single_quotes);
                children.push(opening_double_quote);
                children
            }
            Self::ClosingText {
                closing_double_quote,
                closing_single_quotes,
            } => {
                let mut children = vec![closing_double_quote.as_ref()];
                children.extend(closing_single_quotes);
                children
            }
            Self::Text {
                opening,
                parts,
                closing,
            } => {
                let mut children = vec![opening.as_ref()];
                children.extend(parts);
                children.push(closing);
                children
            }
            Self::TextNewline(_) | Self::TextPart(_) => vec![],
            Self::TextInterpolation {
                opening_curly_braces,
                expression,
                closing_curly_braces,
            } => {
                let mut children = vec![];
                children.extend(opening_curly_braces);
                children.push(expression);
                children.extend(closing_curly_braces);
                children
            }
            Self::BinaryBar { left, bar, right } => {
                let mut children = vec![left.as_ref()];
                children.push(bar);
                children.push(right);
                children
            }
            Self::Parenthesized {
                opening_parenthesis,
                inner,
                closing_parenthesis,
            } => {
                let mut children = vec![opening_parenthesis.as_ref()];
                children.push(inner);
                children.push(closing_parenthesis);
                children
            }
            Self::Call {
                receiver,
                arguments,
            } => {
                let mut children = vec![receiver.as_ref()];
                children.extend(arguments);
                children
            }
            Self::List {
                opening_parenthesis,
                items,
                closing_parenthesis,
            } => {
                let mut children = vec![opening_parenthesis.as_ref()];
                children.extend(items);
                children.push(closing_parenthesis);
                children
            }
            Self::ListItem { value, comma } => {
                let mut children = vec![value.as_ref()];
                if let Some(comma) = comma {
                    children.push(comma);
                }
                children
            }
            Self::Struct {
                opening_bracket,
                fields,
                closing_bracket,
            } => {
                let mut children = vec![opening_bracket.as_ref()];
                children.extend(fields);
                children.push(closing_bracket);
                children
            }
            Self::StructField {
                key_and_colon,
                value,
                comma,
            } => {
                let mut children = vec![];
                if let Some(box (key, colon)) = key_and_colon {
                    children.push(key);
                    children.push(colon);
                }
                children.push(value);
                if let Some(box comma) = comma {
                    children.push(comma);
                }
                children
            }
            Self::StructAccess { struct_, dot, key } => {
                vec![struct_.as_ref(), dot.as_ref(), key.as_ref()]
            }
            Self::Match {
                expression,
                percent,
                cases,
            } => {
                let mut children = vec![expression.as_ref(), percent.as_ref()];
                children.extend(cases);
                children
            }
            Self::MatchCase {
                pattern,
                arrow,
                body,
            } => {
                let mut children = vec![pattern.as_ref(), arrow.as_ref()];
                children.extend(body);
                children
            }
            Self::Function {
                opening_curly_brace,
                parameters_and_arrow,
                body,
                closing_curly_brace,
            } => {
                let mut children = vec![opening_curly_brace.as_ref()];
                if let Some((parameters, arrow)) = parameters_and_arrow {
                    children.extend(parameters);
                    children.push(arrow);
                }
                children.extend(body);
                children.push(closing_curly_brace);
                children
            }
            Self::Assignment {
                left,
                assignment_sign,
                body,
            } => {
                let mut children = vec![left.as_ref()];
                children.push(assignment_sign);
                children.extend(body);
                children
            }
            Self::Error { .. } => vec![],
        }
    }
}

impl<D> Display for CstKind<D> {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match &self {
            Self::EqualsSign => '='.fmt(f),
            Self::Comma => ','.fmt(f),
            Self::Dot => '.'.fmt(f),
            Self::Colon => ':'.fmt(f),
            Self::ColonEqualsSign => ":=".fmt(f),
            Self::Bar => '|'.fmt(f),
            Self::OpeningParenthesis => '('.fmt(f),
            Self::ClosingParenthesis => ')'.fmt(f),
            Self::OpeningBracket => '['.fmt(f),
            Self::ClosingBracket => ']'.fmt(f),
            Self::OpeningCurlyBrace => '{'.fmt(f),
            Self::ClosingCurlyBrace => '}'.fmt(f),
            Self::Arrow => "->".fmt(f),
            Self::SingleQuote => '\''.fmt(f),
            Self::DoubleQuote => '"'.fmt(f),
            Self::Percent => '%'.fmt(f),
            Self::Octothorpe => '#'.fmt(f),
            Self::Whitespace(whitespace) => whitespace.fmt(f),
            Self::Newline(newline) => newline.fmt(f),
            Self::Comment {
                octothorpe,
                comment,
            } => {
                octothorpe.fmt(f)?;
                comment.fmt(f)
            }
            Self::TrailingWhitespace { child, whitespace } => {
                child.fmt(f)?;
                for w in whitespace {
                    w.fmt(f)?;
                }
                Ok(())
            }
            Self::Identifier(identifier) => identifier.fmt(f),
            Self::Symbol(symbol) => symbol.fmt(f),
            Self::Int {
                radix_prefix,
                value: _,
                string,
            } => {
                if let Some((_, radix_string)) = radix_prefix {
                    radix_string.fmt(f)?;
                }
                string.fmt(f)
            }
            Self::OpeningText {
                opening_single_quotes,
                opening_double_quote,
            } => {
                for opening_single_quote in opening_single_quotes {
                    opening_single_quote.fmt(f)?;
                }
                opening_double_quote.fmt(f)
            }
            Self::ClosingText {
                closing_double_quote,
                closing_single_quotes,
            } => {
                closing_double_quote.fmt(f)?;
                for closing_single_quote in closing_single_quotes {
                    closing_single_quote.fmt(f)?;
                }
                Ok(())
            }
            Self::Text {
                opening,
                parts,
                closing,
            } => {
                opening.fmt(f)?;
                for line in parts {
                    line.fmt(f)?;
                }
                closing.fmt(f)
            }
            Self::TextNewline(newline) => newline.fmt(f),
            Self::TextPart(literal) => literal.fmt(f),
            Self::TextInterpolation {
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
            Self::BinaryBar { left, bar, right } => {
                write!(f, "{}{}{}", left.kind, bar.kind, right.kind)
            }
            Self::Parenthesized {
                opening_parenthesis,
                inner,
                closing_parenthesis,
            } => write!(
                f,
                "{}{}{}",
                opening_parenthesis.kind, inner.kind, closing_parenthesis.kind,
            ),
            Self::Call {
                receiver,
                arguments,
            } => {
                receiver.fmt(f)?;
                for argument in arguments {
                    argument.fmt(f)?;
                }
                Ok(())
            }
            Self::List {
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
            Self::ListItem { value, comma } => {
                value.fmt(f)?;
                if let Some(comma) = comma {
                    comma.fmt(f)?;
                }
                Ok(())
            }
            Self::Struct {
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
            Self::StructField {
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
            Self::StructAccess { struct_, dot, key } => {
                struct_.fmt(f)?;
                dot.fmt(f)?;
                key.fmt(f)
            }
            Self::Match {
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
            Self::MatchCase {
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
            Self::Function {
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
            Self::Assignment {
                left,
                assignment_sign,
                body,
            } => {
                left.fmt(f)?;
                assignment_sign.fmt(f)?;
                for expression in body {
                    expression.fmt(f)?;
                }
                Ok(())
            }
            Self::Error {
                unparsable_input, ..
            } => unparsable_input.fmt(f),
        }
    }
}
