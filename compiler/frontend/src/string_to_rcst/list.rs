use super::{
    expression::{expression, ExpressionParsingOptions},
    literal::{closing_parenthesis, comma, opening_parenthesis},
    whitespace::whitespaces_and_newlines,
};
use crate::{
    cst::{CstError, CstKind, IsMultiline},
    rcst::Rcst,
};
use tracing::instrument;

#[instrument(level = "trace")]
pub fn list(input: &str, indentation: usize) -> Option<(&str, Rcst)> {
    let (mut input_before_closing, mut opening_parenthesis) = opening_parenthesis(input)?;

    let mut items: Vec<Rcst> = vec![];
    loop {
        // Whitespace before value.
        let (input, whitespace) =
            whitespaces_and_newlines(input_before_closing, indentation + 1, true);
        let item_indentation = if whitespace.is_multiline() {
            indentation + 1
        } else {
            indentation
        };
        if items.is_empty() {
            opening_parenthesis = opening_parenthesis.wrap_in_whitespace(whitespace);
        } else {
            let last = items.pop().unwrap();
            items.push(last.wrap_in_whitespace(whitespace));
        }

        // Value.
        let (input_after_expression, value) = expression(
            input,
            item_indentation,
            ExpressionParsingOptions {
                allow_assignment: false,
                allow_call: true,
                allow_bar: true,
                allow_function: true,
            },
        )
        .map_or((input, None), |(input, expression)| {
            (input, Some(expression))
        });

        // Whitespace between value and comma.
        let (input, whitespace) =
            whitespaces_and_newlines(input_after_expression, item_indentation + 1, true);

        // Comma.
        let (input, comma) = match comma(input) {
            // It is an empty list if there is no first expression but a comma
            Some((input, comma)) if items.is_empty() && value.is_none() => {
                let (input, trailing_whitespace) =
                    whitespaces_and_newlines(input, indentation + 1, true);
                let comma = comma.wrap_in_whitespace(trailing_whitespace);

                // Closing parenthesis.
                if let Some((input, closing_parenthesis)) = closing_parenthesis(input) {
                    return Some((
                        input,
                        CstKind::List {
                            opening_parenthesis: Box::new(opening_parenthesis),
                            items: vec![comma],
                            closing_parenthesis: Box::new(closing_parenthesis),
                        }
                        .into(),
                    ));
                };

                (input, Some(comma))
            }
            Some((input, comma)) => (input, Some(comma)),
            // It is a parenthesized expression if there is no comma after the first expression
            None if items.is_empty() => {
                let (input, whitespace) =
                    whitespaces_and_newlines(input_after_expression, indentation, true);
                let (input, closing_parenthesis) =
                    closing_parenthesis(input).unwrap_or_else(|| {
                        (
                            input,
                            CstKind::Error {
                                unparsable_input: String::new(),
                                error: CstError::ParenthesisNotClosed,
                            }
                            .into(),
                        )
                    });

                return Some((
                    input,
                    CstKind::Parenthesized {
                        opening_parenthesis: Box::new(opening_parenthesis),
                        inner: Box::new(
                            value
                                .unwrap_or_else(|| {
                                    CstKind::Error {
                                        unparsable_input: String::new(),
                                        error: CstError::OpeningParenthesisMissesExpression,
                                    }
                                    .into()
                                })
                                .wrap_in_whitespace(whitespace),
                        ),
                        closing_parenthesis: Box::new(closing_parenthesis),
                    }
                    .into(),
                ));
            }
            None => (input, None),
        };

        input_before_closing = input;
        if value.is_none() && comma.is_none() {
            break;
        }
        items.push(
            CstKind::ListItem {
                value: Box::new(
                    value
                        .unwrap_or_else(|| {
                            CstKind::Error {
                                unparsable_input: String::new(),
                                error: CstError::ListItemMissesValue,
                            }
                            .into()
                        })
                        .wrap_in_whitespace(whitespace),
                ),
                comma: comma.map(Box::new),
            }
            .into(),
        );
    }

    let (input, whitespace) = whitespaces_and_newlines(input_before_closing, indentation, true);

    let (input, closing_parenthesis) = match closing_parenthesis(input) {
        Some((input, closing_parenthesis)) => {
            if items.is_empty() {
                opening_parenthesis = opening_parenthesis.wrap_in_whitespace(whitespace);
            } else {
                let last = items.pop().unwrap();
                items.push(last.wrap_in_whitespace(whitespace));
            }
            (input, closing_parenthesis)
        }
        None => (
            input,
            CstKind::Error {
                unparsable_input: String::new(),
                error: CstError::ListNotClosed,
            }
            .into(),
        ),
    };

    Some((
        input,
        CstKind::List {
            opening_parenthesis: Box::new(opening_parenthesis),
            items,
            closing_parenthesis: Box::new(closing_parenthesis),
        }
        .into(),
    ))
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::string_to_rcst::utils::assert_rich_ir_snapshot;

    #[test]
    fn test_parenthesized() {
        assert_rich_ir_snapshot!(list("(foo)", 0), @r###"
        Remaining input: ""
        Parsed: Parenthesized:
          opening_parenthesis: OpeningParenthesis
          inner: Identifier "foo"
          closing_parenthesis: ClosingParenthesis
        "###);
        assert_rich_ir_snapshot!(list("foo", 0), @"Nothing was parsed");
        assert_rich_ir_snapshot!(list("(foo", 0), @r###"
        Remaining input: ""
        Parsed: Parenthesized:
          opening_parenthesis: OpeningParenthesis
          inner: Identifier "foo"
          closing_parenthesis: Error:
            unparsable_input: ""
            error: ParenthesisNotClosed
        "###);
        assert_rich_ir_snapshot!(list("()", 0), @r###"
        Remaining input: ""
        Parsed: Parenthesized:
          opening_parenthesis: OpeningParenthesis
          inner: Error:
            unparsable_input: ""
            error: OpeningParenthesisMissesExpression
          closing_parenthesis: ClosingParenthesis
        "###);
    }

    #[test]
    fn test_list() {
        assert_rich_ir_snapshot!(list("hello", 0), @"Nothing was parsed");
        assert_rich_ir_snapshot!(list("(,)", 0), @r###"
        Remaining input: ""
        Parsed: List:
          opening_parenthesis: OpeningParenthesis
          items:
            Comma
          closing_parenthesis: ClosingParenthesis
        "###);
        assert_rich_ir_snapshot!(list("(foo,)", 0), @r###"
        Remaining input: ""
        Parsed: List:
          opening_parenthesis: OpeningParenthesis
          items:
            ListItem:
              value: Identifier "foo"
              comma: Comma
          closing_parenthesis: ClosingParenthesis
        "###);
        assert_rich_ir_snapshot!(list("(foo, )", 0), @r###"
        Remaining input: ""
        Parsed: List:
          opening_parenthesis: OpeningParenthesis
          items:
            TrailingWhitespace:
              child: ListItem:
                value: Identifier "foo"
                comma: Comma
              whitespace:
                Whitespace " "
          closing_parenthesis: ClosingParenthesis
        "###);
        assert_rich_ir_snapshot!(list("(foo,bar)", 0), @r###"
        Remaining input: ""
        Parsed: List:
          opening_parenthesis: OpeningParenthesis
          items:
            ListItem:
              value: Identifier "foo"
              comma: Comma
            ListItem:
              value: Identifier "bar"
              comma: None
          closing_parenthesis: ClosingParenthesis
        "###);
        // (
        //   foo,
        //   4,
        //   "Hi",
        // )
        assert_rich_ir_snapshot!(list("(\n  foo,\n  4,\n  \"Hi\",\n)", 0), @r###"
        Remaining input: ""
        Parsed: List:
          opening_parenthesis: TrailingWhitespace:
            child: OpeningParenthesis
            whitespace:
              Newline "\n"
              Whitespace "  "
          items:
            TrailingWhitespace:
              child: ListItem:
                value: Identifier "foo"
                comma: Comma
              whitespace:
                Newline "\n"
                Whitespace "  "
            TrailingWhitespace:
              child: ListItem:
                value: Int:
                  radix_prefix: None
                  value: 4
                  string: "4"
                comma: Comma
              whitespace:
                Newline "\n"
                Whitespace "  "
            TrailingWhitespace:
              child: ListItem:
                value: Text:
                  opening: OpeningText:
                    opening_single_quotes:
                    opening_double_quote: DoubleQuote
                  parts:
                    TextPart "Hi"
                  closing: ClosingText:
                    closing_double_quote: DoubleQuote
                    closing_single_quotes:
                comma: Comma
              whitespace:
                Newline "\n"
          closing_parenthesis: ClosingParenthesis
        "###);
    }
}
