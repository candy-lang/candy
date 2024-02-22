use crate::{cst::CstKind, rcst::Rcst};

pub static MEANINGFUL_PUNCTUATION: &str = r#"=,.:|()[]{}->'"%#"#;
pub static SUPPORTED_WHITESPACE: &str = " \r\n\t";

impl CstKind<()> {
    #[must_use]
    pub fn wrap_in_whitespace(self, whitespace: Vec<Rcst>) -> Rcst {
        Rcst::from(self).wrap_in_whitespace(whitespace)
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

pub fn whitespace_indentation_score(whitespace: &str) -> usize {
    whitespace
        .chars()
        .map(|c| match c {
            '\t' => 2,
            c if c.is_whitespace() => 1,
            _ => panic!("whitespace_indentation_score called with something non-whitespace"),
        })
        .sum()
}

pub fn parse_multiple<F>(
    mut input: &str,
    parse_single: F,
    count: Option<(usize, bool)>,
) -> Option<(&str, Vec<Rcst>)>
where
    F: Fn(&str) -> Option<(&str, Rcst)>,
{
    let mut rcsts = vec![];
    while let Some((input_after_single, rcst)) = parse_single(input)
        && count.map_or(true, |(count, exact)| exact || rcsts.len() < count)
    {
        input = input_after_single;
        rcsts.push(rcst);
    }
    match count {
        Some((count, _)) if count != rcsts.len() => None,
        _ => Some((input, rcsts)),
    }
}

#[cfg(test)]
pub fn build_comment(value: impl AsRef<str>) -> Rcst {
    CstKind::Comment {
        octothorpe: Box::new(CstKind::Octothorpe.into()),
        comment: value.as_ref().to_string(),
    }
    .into()
}
#[cfg(test)]
pub fn build_identifier(value: impl AsRef<str>) -> Rcst {
    CstKind::Identifier(value.as_ref().to_string()).into()
}
#[cfg(test)]
pub fn build_symbol(value: impl AsRef<str>) -> Rcst {
    CstKind::Symbol(value.as_ref().to_string()).into()
}
#[cfg(test)]
pub fn build_simple_int(value: usize) -> Rcst {
    CstKind::Int {
        radix_prefix: None,
        value: value.into(),
        string: value.to_string(),
    }
    .into()
}
#[cfg(test)]
pub fn build_simple_text(value: impl AsRef<str>) -> Rcst {
    CstKind::Text {
        opening: Box::new(
            CstKind::OpeningText {
                opening_single_quotes: vec![],
                opening_double_quote: Box::new(CstKind::DoubleQuote.into()),
            }
            .into(),
        ),
        parts: vec![CstKind::TextPart(value.as_ref().to_string()).into()],
        closing: Box::new(
            CstKind::ClosingText {
                closing_double_quote: Box::new(CstKind::DoubleQuote.into()),
                closing_single_quotes: vec![],
            }
            .into(),
        ),
    }
    .into()
}
#[cfg(test)]
pub fn build_space() -> Rcst {
    CstKind::Whitespace(" ".to_string()).into()
}
#[cfg(test)]
pub fn build_newline() -> Rcst {
    CstKind::Newline("\n".to_string()).into()
}

#[cfg(test)]
impl Rcst {
    #[must_use]
    pub fn with_trailing_space(self) -> Self {
        self.with_trailing_whitespace(vec![CstKind::Whitespace(" ".to_string())])
    }
    #[must_use]
    pub fn with_trailing_whitespace(self, trailing_whitespace: Vec<CstKind<()>>) -> Self {
        CstKind::TrailingWhitespace {
            child: Box::new(self),
            whitespace: trailing_whitespace.into_iter().map(Into::into).collect(),
        }
        .into()
    }
}
#[cfg(test)]
impl CstKind<()> {
    #[must_use]
    pub fn with_trailing_space(self) -> Rcst {
        Rcst::from(self).with_trailing_space()
    }
    #[must_use]
    pub fn with_trailing_whitespace(self, trailing_whitespace: Vec<Self>) -> Rcst {
        Rcst::from(self).with_trailing_whitespace(trailing_whitespace)
    }
}
