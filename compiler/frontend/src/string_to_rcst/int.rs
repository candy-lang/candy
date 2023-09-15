use super::word::word;
use crate::{
    cst::{CstError, CstKind, IntRadix},
    rcst::Rcst,
};
use num_bigint::BigUint;
use num_traits::Num;
use tracing::instrument;

#[instrument(level = "trace")]
pub fn int(input: &str) -> Option<(&str, Rcst)> {
    let (input, string) = word(input)?;
    if !string.chars().next().unwrap().is_ascii_digit() {
        return None;
    }

    let rcst = if (string.starts_with("0b") || string.starts_with("0B"))
        && string.len() >= 3
        && string.chars().skip(2).all(|c| c == '0' || c == '1')
    {
        // Binary
        let value = BigUint::from_str_radix(&string[2..], 2).expect("Couldn't parse binary int.");
        CstKind::Int {
            radix_prefix: Some((IntRadix::Binary, string[..2].to_string())),
            value,
            string: string[2..].to_string(),
        }
        .into()
    } else if (string.starts_with("0x") || string.starts_with("0X"))
        && string.len() >= 3
        && string.chars().skip(2).all(|c| c.is_ascii_hexdigit())
    {
        // Hexadecimal
        let value =
            BigUint::from_str_radix(&string[2..], 16).expect("Couldn't parse hexadecimal int.");
        CstKind::Int {
            radix_prefix: Some((IntRadix::Hexadecimal, string[..2].to_string())),
            value,
            string: string[2..].to_string(),
        }
        .into()
    } else if string.chars().all(|c| c.is_ascii_digit()) {
        // Decimal
        let value = str::parse(&string).expect("Couldn't parse decimal int.");
        CstKind::Int {
            radix_prefix: None,
            value,
            string,
        }
        .into()
    } else {
        CstKind::Error {
            unparsable_input: string,
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
        // Binary
        assert_eq!(
            int("0b10"),
            Some((
                "",
                CstKind::Int {
                    radix_prefix: Some((IntRadix::Binary, "0b".to_string())),
                    value: 0b10u8.into(),
                    string: "10".to_string()
                }
                .into(),
            )),
        );
        assert_eq!(
            int("0B101"),
            Some((
                "",
                CstKind::Int {
                    radix_prefix: Some((IntRadix::Binary, "0B".to_string())),
                    value: 0b101u8.into(),
                    string: "101".to_string()
                }
                .into(),
            )),
        );
        assert_eq!(
            int("0b10100101"),
            Some((
                "",
                CstKind::Int {
                    radix_prefix: Some((IntRadix::Binary, "0b".to_string())),
                    value: 0b1010_0101u32.into(),
                    string: "10100101".to_string()
                }
                .into(),
            )),
        );
        // Decimal
        assert_eq!(int("42 "), Some((" ", build_simple_int(42))));
        assert_eq!(
            int("012"),
            Some((
                "",
                CstKind::Int {
                    radix_prefix: None,
                    value: 12u8.into(),
                    string: "012".to_string()
                }
                .into(),
            )),
        );
        // Hexadecimal
        assert_eq!(
            int("0x12"),
            Some((
                "",
                CstKind::Int {
                    radix_prefix: Some((IntRadix::Hexadecimal, "0x".to_string())),
                    value: 0x12u8.into(),
                    string: "12".to_string()
                }
                .into(),
            )),
        );
        assert_eq!(
            int("0X012"),
            Some((
                "",
                CstKind::Int {
                    radix_prefix: Some((IntRadix::Hexadecimal, "0X".to_string())),
                    value: 0x12u8.into(),
                    string: "012".to_string()
                }
                .into(),
            )),
        );
        assert_eq!(
            int("0xDEADc0de"),
            Some((
                "",
                CstKind::Int {
                    radix_prefix: Some((IntRadix::Hexadecimal, "0x".to_string())),
                    value: 0xDEAD_C0DEu32.into(),
                    string: "DEADc0de".to_string()
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
