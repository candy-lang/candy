// Builder for printing Candy values.

use itertools::Itertools;
use num_bigint::BigInt;

pub enum PrintValue<T> {
    Int(BigInt),
    Tag { symbol: String, value: Option<T> },
    Text(String),
    List(Vec<T>),
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

/// Prints the value, using the visitor to match across possible values.
///
pub fn print<T>(
    value: T,
    precedence: Precedence,
    max_length: MaxLength,
    visitor: &impl Fn(T) -> Option<PrintValue<T>>,
) -> Option<String> {
    // For each case, the different alternatives of printing are listed.
    // Depending on the available space, the best is chosen.
    Some(match visitor(value)? {
        PrintValue::Int(int) => {
            // - int
            // - `…`

            let string = int.to_string();
            if max_length.fits(string.len()) {
                string
            } else {
                "…".to_string()
            }
        }
        PrintValue::Tag { symbol, value } => {
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
                    let value = print(value, Precedence::High, max_length, visitor)?;
                    if max_length.fits(length_needed_for_structure - 2 + symbol.len() + value.len())
                    {
                        string.push_str(&value);
                    } else {
                        string.push('…');
                    }
                } else {
                    string.push('…');
                }
            }
            if needs_parentheses {
                string.push(')');
            }
            string
        }
        PrintValue::Text(text) => {
            // - full text
            // - `…`

            let text = format!("\"{text}\"");
            if max_length.fits(text.len()) {
                text
            } else {
                "…".to_string()
            }
        }
        PrintValue::Function => {
            // - `{ … }`
            // - `…`

            if max_length.fits(5) { "{ … }" } else { "…" }.to_string()
        }
        PrintValue::List(list) => {
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

            let list_len = list.len();
            let mut items = vec![];
            let mut total_item_length = 0;
            for item in list {
                let item = print(item, Precedence::Low, MaxLength::Unlimited, visitor)?;
                total_item_length += item.len();
                items.push(item);

                // surrounding parentheses, items, and for each item comma + space
                if !max_length.fits(2 + total_item_length + items.len() * 2) {
                    break;
                }
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
                        extra_text
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
        // TODO: structs ignore the max_length for now.
        PrintValue::Struct(entries) => {
            // - all entries: `[Foo: Bar, Baz: 2]`
            // - all keys, some values: `[Foo: Bar, Baz: …, Quz: …]`
            // - some keys: `[Foo: …, Bar: …, + 2 more]`
            // - no items shown: `[struct with 2 entries]`
            // - `…`

            let mut entries = entries
                .into_iter()
                .map(|(key, value)| {
                    print(key, Precedence::Low, max_length, visitor)
                        .map(|stringified_key| (stringified_key, value))
                })
                .collect::<Option<Vec<_>>>()?;
            entries.sort_by(|(a, _), (b, _)| a.cmp(b));
            let len = entries.len();

            let mut string = "[".to_string();
            for (i, (key, value)) in entries.into_iter().enumerate() {
                string.push_str(&key);
                string.push_str(": ");
                string.push_str(&print(value, Precedence::Low, max_length, visitor)?);
                if i < len - 1 {
                    string.push_str(", ");
                }
            }
            string.push(']');
            string
        }
        PrintValue::SendPort => match precedence {
            Precedence::High => "(send port)",
            Precedence::Low => "send port",
        }
        .to_string(),
        PrintValue::ReceivePort => match precedence {
            Precedence::High => "(receive port)",
            Precedence::Low => "receive port",
        }
        .to_string(),
    })
}
