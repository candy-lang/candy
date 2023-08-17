use super::word::word;
use crate::{
    cst::{CstError, CstKind},
    rcst::Rcst,
};
use tracing::instrument;

#[instrument(level = "trace")]
pub fn int(input: &str) -> Option<(&str, Rcst)> {
    let (input, w) = word(input)?;
    if !w.chars().next().unwrap().is_ascii_digit() {
        return None;
    }

    if w.chars().all(|c| c.is_ascii_digit()) {
        let value = str::parse(&w).expect("Couldn't parse int.");
        Some((input, CstKind::Int { value, string: w }.into()))
    } else {
        Some((
            input,
            CstKind::Error {
                unparsable_input: w,
                error: CstError::IntContainsNonDigits,
            }
            .into(),
        ))
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::string_to_rcst::utils::build_simple_int;

    #[test]
    fn test_int() {
        assert_eq!(int("42 "), Some((" ", build_simple_int(42))));
        assert_eq!(
            int("012"),
            Some((
                "",
                CstKind::Int {
                    value: 12u8.into(),
                    string: "012".to_string()
                }
                .into(),
            )),
        );
        assert_eq!(int("123 years"), Some((" years", build_simple_int(123))));
        assert_eq!(int("foo"), None);
        assert_eq!(
            int("3D"),
            Some((
                "",
                CstKind::Error {
                    unparsable_input: "3D".to_string(),
                    error: CstError::IntContainsNonDigits,
                }
                .into(),
            )),
        );
    }
}
