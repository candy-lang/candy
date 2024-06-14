use crate::{cst::CstKind, rcst::Rcst};
use tracing::instrument;

#[instrument(level = "trace")]
pub fn literal<'a>(input: &'a str, literal: &'static str) -> Option<&'a str> {
    input.strip_prefix(literal)
}

macro_rules! define_literal {
    ($name:ident, $string:expr, $kind:expr) => {
        #[instrument(level = "trace")]
        pub fn $name(input: &str) -> Option<(&str, Rcst)> {
            literal(input, $string).map(|it| (it, $kind.into()))
        }
    };
}

define_literal!(equals_sign, "=", CstKind::EqualsSign);
define_literal!(comma, ",", CstKind::Comma);
define_literal!(dot, ".", CstKind::Dot);
define_literal!(colon, ":", CstKind::Colon);
define_literal!(colon_equals_sign, ":=", CstKind::ColonEqualsSign);
define_literal!(opening_parenthesis, "(", CstKind::OpeningParenthesis);
define_literal!(closing_parenthesis, ")", CstKind::ClosingParenthesis);
define_literal!(opening_bracket, "[", CstKind::OpeningBracket);
define_literal!(closing_bracket, "]", CstKind::ClosingBracket);
define_literal!(opening_curly_brace, "{", CstKind::OpeningCurlyBrace);
define_literal!(closing_curly_brace, "}", CstKind::ClosingCurlyBrace);
define_literal!(arrow, "->", CstKind::Arrow);
define_literal!(double_quote, "\"", CstKind::DoubleQuote);
define_literal!(octothorpe, "#", CstKind::Octothorpe);
