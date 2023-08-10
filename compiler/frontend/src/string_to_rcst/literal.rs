use crate::{cst::CstKind, rcst::Rcst};
use tracing::instrument;

#[instrument(level = "trace")]
fn literal<'a>(input: &'a str, literal: &'static str) -> Option<&'a str> {
    input.strip_prefix(literal)
}

#[instrument(level = "trace")]
pub fn equals_sign(input: &str) -> Option<(&str, Rcst)> {
    literal(input, "=").map(|it| (it, CstKind::EqualsSign.into()))
}
#[instrument(level = "trace")]
pub fn comma(input: &str) -> Option<(&str, Rcst)> {
    literal(input, ",").map(|it| (it, CstKind::Comma.into()))
}
#[instrument(level = "trace")]
pub fn dot(input: &str) -> Option<(&str, Rcst)> {
    literal(input, ".").map(|it| (it, CstKind::Dot.into()))
}
#[instrument(level = "trace")]
pub fn colon(input: &str) -> Option<(&str, Rcst)> {
    literal(input, ":").map(|it| (it, CstKind::Colon.into()))
}
#[instrument(level = "trace")]
pub fn colon_equals_sign(input: &str) -> Option<(&str, Rcst)> {
    literal(input, ":=").map(|it| (it, CstKind::ColonEqualsSign.into()))
}
#[instrument(level = "trace")]
pub fn bar(input: &str) -> Option<(&str, Rcst)> {
    literal(input, "|").map(|it| (it, CstKind::Bar.into()))
}
#[instrument(level = "trace")]
pub fn opening_bracket(input: &str) -> Option<(&str, Rcst)> {
    literal(input, "[").map(|it| (it, CstKind::OpeningBracket.into()))
}
#[instrument(level = "trace")]
pub fn closing_bracket(input: &str) -> Option<(&str, Rcst)> {
    literal(input, "]").map(|it| (it, CstKind::ClosingBracket.into()))
}
#[instrument(level = "trace")]
pub fn opening_parenthesis(input: &str) -> Option<(&str, Rcst)> {
    literal(input, "(").map(|it| (it, CstKind::OpeningParenthesis.into()))
}
#[instrument(level = "trace")]
pub fn closing_parenthesis(input: &str) -> Option<(&str, Rcst)> {
    literal(input, ")").map(|it| (it, CstKind::ClosingParenthesis.into()))
}
#[instrument(level = "trace")]
pub fn opening_curly_brace(input: &str) -> Option<(&str, Rcst)> {
    literal(input, "{").map(|it| (it, CstKind::OpeningCurlyBrace.into()))
}
#[instrument(level = "trace")]
pub fn closing_curly_brace(input: &str) -> Option<(&str, Rcst)> {
    literal(input, "}").map(|it| (it, CstKind::ClosingCurlyBrace.into()))
}
#[instrument(level = "trace")]
pub fn arrow(input: &str) -> Option<(&str, Rcst)> {
    literal(input, "->").map(|it| (it, CstKind::Arrow.into()))
}
#[instrument(level = "trace")]
pub fn single_quote(input: &str) -> Option<(&str, Rcst)> {
    literal(input, "'").map(|it| (it, CstKind::SingleQuote.into()))
}
#[instrument(level = "trace")]
pub fn double_quote(input: &str) -> Option<(&str, Rcst)> {
    literal(input, "\"").map(|it| (it, CstKind::DoubleQuote.into()))
}
#[instrument(level = "trace")]
pub fn percent(input: &str) -> Option<(&str, Rcst)> {
    literal(input, "%").map(|it| (it, CstKind::Percent.into()))
}
#[instrument(level = "trace")]
pub fn octothorpe(input: &str) -> Option<(&str, Rcst)> {
    literal(input, "#").map(|it| (it, CstKind::Octothorpe.into()))
}
#[instrument(level = "trace")]
pub fn newline(input: &str) -> Option<(&str, Rcst)> {
    let newlines = vec!["\n", "\r\n"];
    for newline in newlines {
        if let Some(input) = literal(input, newline) {
            return Some((input, CstKind::Newline(newline.to_string()).into()));
        }
    }
    None
}

#[test]
fn test_literal() {
    assert_eq!(literal("hello, world", "hello"), Some(", world"));
    assert_eq!(literal("hello, world", "hi"), None);
}
