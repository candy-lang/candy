use candy_frontend::{
    cst::{Cst, CstData, CstKind, Id, IsMultiline},
    id::{CountableId, IdGenerator},
    position::Offset,
};
use existing_whitespace::ExistingWhitespace;
use extension_trait::extension_trait;
use itertools::Itertools;
use last_line_width::LastLineWidth;
use std::ops::Range;
use traversal::dft_pre;

mod existing_whitespace;
mod last_line_width;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct TextEdit {
    pub range: Range<Offset>,
    pub new_text: String,
}

pub const MAX_LINE_LENGTH: usize = 100;

#[extension_trait]
pub impl Formatter for &[Cst] {
    fn format_to_string(&self) -> String {
        self.format().iter().join("")
    }
    fn format_to_edits(&self) -> Vec<TextEdit> {
        todo!()
    }
    fn format(&self) -> Vec<Cst> {
        let mut id_generator = IdGenerator::start_at(largest_id(self).to_usize() + 1);

        self.format_helper(&mut id_generator, 0)
        // TODO: fix spans
    }
}

fn largest_id(csts: &[Cst]) -> Id {
    csts.iter()
        .map(|it| {
            dft_pre(it, |it| it.kind.children().into_iter())
                .map(|(_, it)| it.data.id)
                .max()
                .unwrap()
        })
        .max()
        .unwrap()
}

#[extension_trait]
impl FormatCsts for [Cst] {
    fn format_helper(
        &self,
        id_generator: &mut IdGenerator<Id>,
        indentation_level: usize,
    ) -> Vec<Cst> {
        let mut is_after_expression = false;
        let mut result = vec![];
        for cst in self {
            match &cst.kind {
                CstKind::Whitespace(_) => {
                    if !is_after_expression {
                        continue;
                    }
                    // TOOD: indentation
                }
                CstKind::Newline(_) => {
                    is_after_expression = false;
                    continue;
                }
                _ => {
                    result.push(cst.format(id_generator, indentation_level));
                    is_after_expression = true;
                }
            }
        }
        result
    }
}
#[extension_trait]
impl FormatCst for Cst {
    fn format(&self, id_generator: &mut IdGenerator<Id>, indentation_level: usize) -> Cst {
        let new_kind = match &self.kind {
            CstKind::EqualsSign
            | CstKind::Comma
            | CstKind::Dot
            | CstKind::Colon
            | CstKind::ColonEqualsSign
            | CstKind::Bar
            | CstKind::OpeningParenthesis
            | CstKind::ClosingParenthesis
            | CstKind::OpeningBracket
            | CstKind::ClosingBracket
            | CstKind::OpeningCurlyBrace
            | CstKind::ClosingCurlyBrace
            | CstKind::Arrow
            | CstKind::SingleQuote
            | CstKind::DoubleQuote
            | CstKind::Percent
            | CstKind::Octothorpe
            | CstKind::Whitespace(_)
            | CstKind::Newline(_) => return self.to_owned(),
            CstKind::Comment {
                octothorpe,
                comment,
            } => todo!(),
            CstKind::TrailingWhitespace { child, whitespace } => todo!(),
            CstKind::Identifier(_) | CstKind::Symbol(_) => return self.to_owned(),
            CstKind::Int { value, string } => return self.to_owned(), // TODO
            CstKind::OpeningText {
                opening_single_quotes,
                opening_double_quote,
            } => todo!(),
            CstKind::ClosingText {
                closing_double_quote,
                closing_single_quotes,
            } => todo!(),
            CstKind::Text {
                opening,
                parts,
                closing,
            } => todo!(),
            CstKind::TextPart(_) => todo!(),
            CstKind::TextInterpolation {
                opening_curly_braces,
                expression,
                closing_curly_braces,
            } => todo!(),
            CstKind::BinaryBar { left, bar, right } => todo!(),
            CstKind::Parenthesized {
                opening_parenthesis,
                inner,
                closing_parenthesis,
            } => todo!(),
            CstKind::Call {
                receiver,
                arguments,
            } => {
                let (receiver, receiver_whitespace) = receiver.split_trailing_whitespace();
                let receiver = receiver.format(id_generator, indentation_level);

                let mut arguments = arguments
                    .iter()
                    .map(|argument| {
                        let (argument, argument_whitespace) = argument.split_trailing_whitespace();
                        let argument = argument.format(id_generator, indentation_level + 1);
                        (argument, argument_whitespace)
                    })
                    .collect_vec();

                let indentation_level = if arguments.iter().all(|(it, _)| it.is_singleline())
                    && arguments
                        .iter()
                        .map(|(it, _)| 1 + it.last_line_width())
                        .sum::<usize>()
                        + receiver.last_line_width()
                        <= MAX_LINE_LENGTH
                {
                    None
                } else {
                    Some(indentation_level + 1)
                };

                let receiver =
                    receiver_whitespace.into_trailing(id_generator, receiver, indentation_level);

                let last_argument = arguments.pop().unwrap().0;
                let mut arguments = arguments
                    .into_iter()
                    .map(|(argument, argument_whitespace)| {
                        argument_whitespace.into_trailing(id_generator, argument, indentation_level)
                    })
                    .collect_vec();
                arguments.push(last_argument);

                CstKind::Call {
                    receiver: Box::new(receiver),
                    arguments,
                }
            }
            CstKind::List {
                opening_parenthesis,
                items,
                closing_parenthesis,
            } => todo!(),
            CstKind::ListItem { value, comma } => todo!(),
            CstKind::Struct {
                opening_bracket,
                fields,
                closing_bracket,
            } => todo!(),
            CstKind::StructField {
                key_and_colon,
                value,
                comma,
            } => todo!(),
            CstKind::StructAccess { struct_, dot, key } => todo!(),
            CstKind::Match {
                expression,
                percent,
                cases,
            } => todo!(),
            CstKind::MatchCase {
                pattern,
                arrow,
                body,
            } => todo!(),
            CstKind::Lambda {
                opening_curly_brace,
                parameters_and_arrow,
                body,
                closing_curly_brace,
            } => todo!(),
            CstKind::Assignment {
                left,
                assignment_sign,
                body,
            } => todo!(),
            CstKind::Error {
                unparsable_input,
                error,
            } => todo!(),
        };
        Cst {
            data: self.data.clone(),
            kind: new_kind,
        }
    }

    fn split_trailing_whitespace(&self) -> (&Cst, ExistingWhitespace) {
        match &self.kind {
            CstKind::TrailingWhitespace { child, whitespace } => (
                child,
                ExistingWhitespace::Some {
                    id: self.data.id,
                    trailing_whitespace: whitespace,
                },
            ),
            _ => (self, ExistingWhitespace::None),
        }
    }
}

#[cfg(test)]
mod test {
    use crate::Formatter;
    use candy_frontend::{rcst_to_cst::RcstsToCstsExt, string_to_rcst::parse_rcst};
    use itertools::Itertools;

    #[test]
    fn test_int() {
        test("1", "1");
        test("123", "123");
    }
    #[test]
    fn test_call() {
        test("foo bar Baz", "foo bar Baz");
        test("foo   bar Baz ", "foo bar Baz");
        test("foo   bar Baz ", "foo bar Baz");
        test(
"foo firstVeryVeryVeryVeryVeryVeryVeryVeryLongArgument secondVeryVeryVeryVeryVeryVeryVeryVeryLongArgument",
            "foo\n  firstVeryVeryVeryVeryVeryVeryVeryVeryLongArgument\n  secondVeryVeryVeryVeryVeryVeryVeryVeryLongArgument",
        );
    }

    fn test(source: &str, expected: &str) {
        let csts = parse_rcst(source).to_csts();
        assert_eq!(source, csts.iter().join(""));

        dbg!(&csts);

        let formatted = csts.as_slice().format_to_string();
        assert_eq!(formatted, expected);
    }
}
