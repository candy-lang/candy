use super::literal::octothorpe;
use crate::{cst::CstKind, rcst::Rcst};
use extension_trait::extension_trait;
use itertools::Itertools;
use tracing::instrument;

#[instrument(level = "trace")]
pub fn whitespace(mut input: &str) -> (&str, Vec<Rcst>) {
    let mut parts = vec![];
    loop {
        let input_from_iteration_start = input;

        // Whitespace
        let mut chars = vec![];
        let mut contains_tab = false;
        while let Some(c) = input.chars().next() {
            match c {
                ' ' | '\r' | '\n' => {}
                '\t' => contains_tab = true,
                _ => break,
            }
            chars.push(c);
            input = &input[c.len_utf8()..];
        }
        let whitespace = chars.into_iter().join("");
        if contains_tab {
            parts.push(
                CstKind::Error {
                    unparsable_input: whitespace,
                    error: "We use spaces, not tabs.".to_string(),
                }
                .into(),
            );
        } else if !whitespace.is_empty() {
            parts.push(CstKind::Whitespace(whitespace).into());
        };

        // Comment
        if let Some((new_input, comment)) = comment(input) {
            input = new_input;
            parts.push(comment);
        }

        if input == input_from_iteration_start {
            break;
        }
    }
    (input, parts)
}

#[instrument(level = "trace")]
fn comment(input: &str) -> Option<(&str, Rcst)> {
    let (mut input, octothorpe) = octothorpe(input)?;
    let mut comment = vec![];
    loop {
        match input.chars().next() {
            Some('\n' | '\r') | None => {
                break;
            }
            Some(c) => {
                comment.push(c);
                input = &input[c.len_utf8()..];
            }
        }
    }
    Some((
        input,
        CstKind::Comment {
            octothorpe: Box::new(octothorpe),
            comment: comment.into_iter().join(""),
        }
        .into(),
    ))
}

#[extension_trait]
pub impl<'a> AndTrailingWhitespace<'a> for (&'a str, Rcst) {
    #[must_use]
    fn and_trailing_whitespace(self) -> (&'a str, Rcst) {
        let (input, whitespace) = whitespace(self.0);
        (input, self.1.wrap_in_whitespace(whitespace))
    }
}
#[extension_trait]
pub impl<'a> OptionAndTrailingWhitespace<'a> for Option<(&'a str, Rcst)> {
    #[must_use]
    fn and_trailing_whitespace(self) -> Self {
        self.map(AndTrailingWhitespace::and_trailing_whitespace)
    }
}

impl Rcst {
    #[must_use]
    pub fn wrap_in_whitespace(mut self, mut whitespace: Vec<Self>) -> Self {
        if whitespace.is_empty() {
            return self;
        }

        if let CstKind::TrailingWhitespace {
            whitespace: self_whitespace,
            ..
        } = &mut self.kind
        {
            self_whitespace.append(&mut whitespace);
            self
        } else {
            CstKind::TrailingWhitespace {
                child: Box::new(self),
                whitespace,
            }
            .into()
        }
    }
}
