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
    use crate::string_to_rcst::utils::assert_rich_ir_snapshot;

    #[test]
    fn test_int() {
        // Binary
        assert_rich_ir_snapshot!(int("0b10"), @r###"
        Remaining input: ""
        Parsed: Int:
          radix_prefix:
            radix: Binary
            prefix: "0b"
          value: 2
          string: "10"
        "###);
        assert_rich_ir_snapshot!(int("0B101"), @r###"
        Remaining input: ""
        Parsed: Int:
          radix_prefix:
            radix: Binary
            prefix: "0B"
          value: 5
          string: "101"
        "###);
        assert_rich_ir_snapshot!(int("0b10100101"), @r###"
        Remaining input: ""
        Parsed: Int:
          radix_prefix:
            radix: Binary
            prefix: "0b"
          value: 165
          string: "10100101"
        "###);
        // Decimal
        assert_rich_ir_snapshot!(int("42 "), @r###"
        Remaining input: " "
        Parsed: Int:
          radix_prefix: None
          value: 42
          string: "42"
        "###);
        assert_rich_ir_snapshot!(int("012"), @r###"
        Remaining input: ""
        Parsed: Int:
          radix_prefix: None
          value: 12
          string: "012"
        "###);
        // Hexadecimal
        assert_rich_ir_snapshot!(int("0x12"), @r###"
        Remaining input: ""
        Parsed: Int:
          radix_prefix:
            radix: Hexadecimal
            prefix: "0x"
          value: 18
          string: "12"
        "###);
        assert_rich_ir_snapshot!(int("0X012"), @r###"
        Remaining input: ""
        Parsed: Int:
          radix_prefix:
            radix: Hexadecimal
            prefix: "0X"
          value: 18
          string: "012"
        "###);
        assert_rich_ir_snapshot!(int("0xDEADc0de"), @r###"
        Remaining input: ""
        Parsed: Int:
          radix_prefix:
            radix: Hexadecimal
            prefix: "0x"
          value: 3735929054
          string: "DEADc0de"
        "###);

        assert_rich_ir_snapshot!(int("123 years"), @r###"
        Remaining input: " years"
        Parsed: Int:
          radix_prefix: None
          value: 123
          string: "123"
        "###);
        assert_rich_ir_snapshot!(int("foo"), @"Nothing was parsed");
        assert_rich_ir_snapshot!(int("3D"), @r###"
        Remaining input: ""
        Parsed: Error:
          unparsable_input: "3D"
          error: IntContainsNonDigits
        "###);
    }
}
