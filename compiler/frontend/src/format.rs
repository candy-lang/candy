// Builder for printing Candy values.

use itertools::{EitherOrBoth, Itertools};
use num_bigint::BigInt;
use std::{borrow::Cow, ops::Sub};

pub enum FormatValue<'a, T: Copy> {
    Int(Cow<'a, BigInt>),
    Tag { symbol: &'a str, value: Option<T> },
    Text(&'a str),
    List(&'a [T]),
    Struct(Cow<'a, Vec<(T, T)>>),
    Function,
    SendPort,
    ReceivePort,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Precedence {
    /// No spaces allowed.
    High,

    /// Spaces allowed.
    Low,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum MaxLength {
    Unlimited,
    Limited(usize),
}
impl MaxLength {
    const fn fits(self, len: usize) -> bool {
        match self {
            Self::Unlimited => true,
            Self::Limited(max) => len <= max,
        }
    }
}
impl Sub<usize> for MaxLength {
    type Output = Self;

    fn sub(self, rhs: usize) -> Self::Output {
        match self {
            Self::Unlimited => Self::Unlimited,
            Self::Limited(n) => {
                assert!(n >= rhs);
                Self::Limited(n - rhs)
            }
        }
    }
}

/// Formats the value, using the visitor to match across possible values.
pub fn format_value<'a, T: 'a + Copy>(
    value: T,
    precedence: Precedence,
    max_length: MaxLength,
    visitor: &impl Fn(T) -> Option<FormatValue<'a, T>>,
) -> Option<String> {
    // For each case, the different alternatives of printing are listed.
    // Depending on the available space, the best is chosen.
    Some(match visitor(value)? {
        FormatValue::Int(int) => {
            // - int
            // - `…`

            let string = int.to_string();
            if max_length.fits(string.len()) {
                string
            } else {
                "…".to_string()
            }
        }
        FormatValue::Tag { symbol, value } => {
            // - full: `Tag Value` or `(Tag Value)` or `Tag`
            // - only symbol: `Tag …` or `(Tag …)` or `Tag`
            // - only structure: `… …` or `(… …)` or `…`
            // - `…`

            let needs_parentheses = value.is_some() && precedence == Precedence::High;

            let length_needed_for_structure = match (needs_parentheses, value.is_some()) {
                (false, false) => 1, // `…`
                (false, true) => 3,  // `… …`
                (true, false) => unreachable!(),
                (true, true) => 5, // `(… …)`
            };
            if !max_length.fits(length_needed_for_structure) {
                return Some("…".to_string());
            }

            let mut string = String::new();
            if needs_parentheses {
                string.push('(');
            }

            let symbol_fits = max_length.fits(length_needed_for_structure - 1 + symbol.len());
            if symbol_fits {
                string.push_str(symbol);
            } else {
                string.push('…');
            }

            if let Some(value) = value {
                string.push(' ');
                if symbol_fits {
                    string.push_str(&format_value(
                        value,
                        Precedence::High,
                        max_length - (length_needed_for_structure - 2 + symbol.len()),
                        visitor,
                    )?);
                } else {
                    string.push('…');
                }
            }
            if needs_parentheses {
                string.push(')');
            }
            string
        }
        FormatValue::Text(text) => {
            // - full text
            // - `…`

            if max_length.fits(1 + text.len() + 1) {
                format!("\"{text}\"")
            } else {
                "…".to_string()
            }
        }
        FormatValue::Function => {
            // - `{ … }`
            // - `…`

            if max_length.fits(5) { "{ … }" } else { "…" }.to_string()
        }
        FormatValue::List(list) => {
            // - all items: `(Foo, Bar, Baz)`
            // - some items: `(Foo, Bar, + 2 more)`
            // - no items shown: `(list of 2 items)`
            // - `…`

            if !max_length.fits(3) {
                return Some("…".to_string());
            }

            if list.is_empty() {
                return Some("(,)".to_string());
            }

            if !max_length.fits(4) {
                return Some("…".to_string());
            }

            let list_len = list.len();
            if list_len == 1 {
                let item = list[0];
                let item = format_value(item, Precedence::Low, MaxLength::Unlimited, visitor)?;
                return if max_length.fits(item.len() + 3) {
                    Some(format!("({item},)"))
                } else {
                    Some("(…,)".to_string())
                };
            }

            let mut items = Vec::with_capacity(list_len);
            let mut total_item_length = 0;
            for item in list {
                // Would an additional item fit?
                // surrounding parentheses, items, and for each item comma + space, new item
                if !max_length.fits(2 + total_item_length + items.len() * 2 + 1) {
                    break;
                }

                let item = format_value(*item, Precedence::Low, MaxLength::Unlimited, visitor)?;
                total_item_length += item.len();
                items.push(item);
            }
            if items.len() == list_len && max_length.fits(total_item_length + items.len() * 2) {
                return Some(format!("({})", items.into_iter().join(", ")));
            }

            // Not all items fit. Try to remove the back ones, showing "+ X more" instead.
            while let Some(popped) = items.pop() {
                total_item_length -= popped.len();
                let extra_text = format!("+ {} more", list_len - items.len());
                if max_length.fits(total_item_length + items.len() * 2 + extra_text.len()) {
                    return Some(format!(
                        "({}, {})",
                        items.into_iter().join(", "),
                        extra_text,
                    ));
                }
            }

            let summary = format!("(list of {list_len} items)");
            if max_length.fits(summary.len()) {
                summary
            } else {
                "…".to_string()
            }
        }
        FormatValue::Struct(entries) => {
            // - all entries: `[Baz: 2, Foo: Bar]`
            // - all keys, some values: `[Baz: …, Foo: Bar, Quz: …]`
            // - some keys: `[Bar: …, Foo: …, + 2 more]`
            // - no items shown: `[struct with 2 entries]`
            // - `…`

            if !max_length.fits(2) {
                return Some("…".to_string());
            }

            if entries.is_empty() {
                return Some("[]".to_string());
            }

            let num_entries = entries.len();

            let mut entries = entries
                .iter()
                .map(|(key, value)| {
                    format_value(*key, Precedence::Low, MaxLength::Unlimited, visitor)
                        .map(|key| (key, value))
                })
                .collect::<Option<Vec<_>>>()?;
            entries.sort_by(|(key_a, _), (key_b, _)| key_a.cmp(key_b));
            let mut total_keys_length: usize = entries.iter().map(|(key, _)| key.len()).sum();

            // surrounding brackets, keys, and for each key colon + space + dots + comma + space
            if entries.len() < num_entries
                || !max_length.fits(2 + total_keys_length + entries.len() * 5)
            {
                // Not all keys fit. Try to remove the back ones, showing "+ X more" instead.
                while let Some(popped) = entries.pop() {
                    total_keys_length -= popped.0.len();
                    let extra_text = format!("+ {} more", num_entries - entries.len());
                    if max_length.fits(2 + total_keys_length + entries.len() * 5 + extra_text.len())
                    {
                        return Some(format!(
                            "[{}, {}]",
                            entries
                                .into_iter()
                                .map(|(key, _)| format!("{key}: …"))
                                .join(", "),
                            extra_text,
                        ));
                    }
                }

                let summary = format!("[struct with {num_entries} entries]");
                return Some(if max_length.fits(summary.len()) {
                    summary
                } else {
                    "…".to_string()
                });
            }

            let mut values = Vec::with_capacity(num_entries);
            let mut total_values_length = num_entries; // dots for every value
            for (_, value) in &entries {
                let value = format_value(**value, Precedence::Low, MaxLength::Unlimited, visitor)?;
                total_values_length += value.len() - 1; // remove the dots, add the value
                values.push(value);

                if !max_length.fits(total_keys_length + entries.len() * 4 + total_values_length) {
                    break;
                }
            }

            if values.len() == num_entries
                && max_length.fits(total_keys_length + entries.len() * 4 + total_values_length)
            {
                // Everything fits!
                return Some(format!(
                    "[{}]",
                    entries
                        .into_iter()
                        .map(|(key, _)| key)
                        .zip(values)
                        .map(|(key, value)| format!("{key}: {value}"))
                        .join(", "),
                ));
            }

            // Not all values fit. Try to remove the back ones.
            while let Some(popped) = values.pop() {
                total_values_length -= popped.len() - 1; // replace with dots
                if max_length.fits(total_keys_length + total_values_length + num_entries * 4) {
                    break;
                }
            }

            format!(
                "[{}]",
                entries
                    .into_iter()
                    .map(|(key, _)| key)
                    .zip_longest(values)
                    .map(|zipped| match zipped {
                        EitherOrBoth::Both(key, value) => format!("{key}: {value}"),
                        EitherOrBoth::Left(key) => format!("{key}: …"),
                        EitherOrBoth::Right(_) => unreachable!(),
                    })
                    .join(", "),
            )
        }
        FormatValue::SendPort => match precedence {
            Precedence::High => "(send port)",
            Precedence::Low => "send port",
        }
        .to_string(),
        FormatValue::ReceivePort => match precedence {
            Precedence::High => "(receive port)",
            Precedence::Low => "receive port",
        }
        .to_string(),
    })
}
