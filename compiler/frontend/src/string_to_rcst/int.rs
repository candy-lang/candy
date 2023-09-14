use super::word::word;
use crate::{
    cst::{CstError, CstKind},
    rcst::Rcst,
};
use num_bigint::BigUint;
use num_traits::Num;
use tracing::instrument;

#[instrument(level = "trace")]
pub fn int(input: &str) -> Option<(&str, Rcst)> {
    let (input, w) = word(input)?;
    if !w.chars().next().unwrap().is_ascii_digit() {
        return None;
    }

    let rcst = if (w.starts_with("0x") || w.starts_with("0X"))
        && w.len() >= 3
        && w.chars().skip(2).all(|c| c.is_ascii_hexdigit())
    {
        let value = BigUint::from_str_radix(&w[2..], 16).expect("Couldn't parse hexadecimal int.");
        CstKind::Int { value, string: w }.into()
    } else if w.chars().all(|c| c.is_ascii_digit()) {
        let value = str::parse(&w).expect("Couldn't parse decimal int.");
        CstKind::Int { value, string: w }.into()
    } else {
        CstKind::Error {
            unparsable_input: w,
            error: CstError::IntContainsNonDigits,
        }
        .into()
    };
    Some((input, rcst))
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
        assert_eq!(
            int("0x12"),
            Some((
                "",
                CstKind::Int {
                    value: 0x12u8.into(),
                    string: "0x12".to_string()
                }
                .into(),
            )),
        );
        assert_eq!(
            int("0X012"),
            Some((
                "",
                CstKind::Int {
                    value: 0x12u8.into(),
                    string: "0X012".to_string()
                }
                .into(),
            )),
        );
        assert_eq!(
            int("0xDEADc0de"),
            Some((
                "",
                CstKind::Int {
                    value: 0xDEAD_C0DEu32.into(),
                    string: "0xDEADc0de".to_string()
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
