// Builder for printing Candy values.

use itertools::{EitherOrBoth, Itertools};
use num_bigint::BigInt;
use std::{borrow::Cow, ops::Sub};

pub enum FormatValue<'a, T> {
    Int(Cow<'a, BigInt>),
    Tag { symbol: &'a str, value: Option<T> },
    Text(&'a str),
    List(&'a [T]),
    Struct(Vec<(T, T)>),
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
    fn fits(self, len: usize) -> bool {
        match self {
            MaxLength::Unlimited => true,
            MaxLength::Limited(max) => len <= max,
        }
    }
}
impl Sub<usize> for MaxLength {
    type Output = MaxLength;

    fn sub(self, rhs: usize) -> Self::Output {
        match self {
            MaxLength::Unlimited => MaxLength::Unlimited,
            MaxLength::Limited(n) => {
                assert!(n >= rhs);
                MaxLength::Limited(n - rhs)
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

            let mut string = "".to_string();
            if needs_parentheses {
                string.push('(');
            }

            let symbol_fits = max_length.fits(length_needed_for_structure - 1 + symbol.len());
            if symbol_fits {
                string.push_str(&symbol);
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

            let summary = format!("(list of {} items)", list_len);
            if max_length.fits(summary.len()) {
                summary
            } else {
                "…".to_string()
            }
        }
        FormatValue::Struct(entries) => {
            // - all entries: `[Foo: Bar, Baz: 2]`
            // - all keys, some values: `[Foo: Bar, Baz: …, Quz: …]`
            // - some keys: `[Foo: …, Bar: …, + 2 more]`
            // - no items shown: `[struct with 2 entries]`
            // - `…`

            if !max_length.fits(2) {
                return Some("…".to_string());
            }

            if entries.is_empty() {
                return Some("[]".to_string());
            }

            let num_entries = entries.len();
            let mut keys = vec![];
            let mut values = vec![];
            for (key, value) in entries {
                keys.push(key);
                values.push(value);
            }

            let mut texted_keys = Vec::with_capacity(num_entries);
            let mut total_keys_length = 0;
            for key in keys {
                // surrounding brackets, keys, and for each key colon + space + dots + comma + space
                if !max_length.fits(total_keys_length + texted_keys.len() * 5) {
                    break;
                }

                let key = format_value(key, Precedence::Low, MaxLength::Unlimited, visitor)?;
                total_keys_length += key.len();
                texted_keys.push(key);
            }

            if texted_keys.len() < num_entries
                || !max_length.fits(total_keys_length + texted_keys.len() * 5)
            {
                // Not all keys fit. Try to remove the back ones, showing "+ X more" instead.
                while let Some(popped) = texted_keys.pop() {
                    total_keys_length -= popped.len();
                    let extra_text = format!("+ {} more", num_entries - texted_keys.len());
                    if max_length.fits(total_keys_length + texted_keys.len() * 5 + extra_text.len())
                    {
                        return Some(format!(
                            "[{}, {}]",
                            texted_keys
                                .into_iter()
                                .map(|key| format!("{key}: …"))
                                .join(", "),
                            extra_text,
                        ));
                    }
                }

                let summary = format!("[struct with {} entries]", num_entries);
                return Some(if max_length.fits(summary.len()) {
                    summary
                } else {
                    "…".to_string()
                });
            }

            let mut texted_values = Vec::with_capacity(num_entries);
            let mut total_values_length = num_entries; // dots for every value
            for value in values {
                let value = format_value(value, Precedence::Low, MaxLength::Unlimited, visitor)?;
                total_values_length += value.len() - 1; // remove the dots, add the value
                texted_values.push(value);

                if !max_length.fits(total_keys_length + texted_keys.len() * 4 + total_values_length)
                {
                    break;
                }
            }

            if texted_values.len() == num_entries
                && max_length.fits(total_keys_length + texted_keys.len() * 4 + total_values_length)
            {
                // Everything fits!
                return Some(format!(
                    "[{}]",
                    texted_keys
                        .into_iter()
                        .zip(texted_values)
                        .map(|(key, value)| format!("{key}: {value}"))
                        .join(", "),
                ));
            }

            // Not all values fit. Try to remove the back ones.
            while let Some(popped) = texted_values.pop() {
                total_values_length -= popped.len() - 1; // replace with dots
                if max_length.fits(total_keys_length + total_values_length + num_entries * 4) {
                    break;
                }
            }

            format!(
                "[{}]",
                texted_keys
                    .into_iter()
                    .zip_longest(texted_values)
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
