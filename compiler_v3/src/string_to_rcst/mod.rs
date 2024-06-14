use crate::{cst::CstKind, rcst::Rcst};
use lambda::body;

mod assignment;
mod expression;
mod lambda;
mod literal;
mod text;
mod whitespace;
mod word;

#[must_use]
pub fn string_to_rcst(source: &str) -> Vec<Rcst> {
    let (mut rest, mut rcsts) = body(source);

    if !rest.is_empty() {
        let trailing_newline = if rest.ends_with("\r\n") {
            let newline = CstKind::Whitespace(rest[rest.len() - 2..].to_string());
            rest = &rest[..rest.len() - 2];
            Some(newline)
        } else if rest.ends_with('\n') {
            let newline = CstKind::Whitespace(rest[rest.len() - 1..].to_string());
            rest = &rest[..rest.len() - 1];
            Some(newline)
        } else {
            None
        };
        rcsts.push(
            CstKind::Error {
                unparsable_input: rest.to_string(),
                error: "The parser couldn't parse this rest.".to_string(),
            }
            .into(),
        );
        if let Some(trailing_newline) = trailing_newline {
            rcsts.push(trailing_newline.into());
        }
    }
    rcsts
}
