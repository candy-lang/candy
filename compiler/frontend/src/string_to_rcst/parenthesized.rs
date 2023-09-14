#[cfg(test)]
mod test {
    use crate::{
        cst::{CstError, CstKind},
        string_to_rcst::{list::list, utils::build_identifier},
    };

    #[test]
    fn test_parenthesized() {
        assert_eq!(
            list("(foo)", 0),
            Some((
                "",
                CstKind::Parenthesized {
                    opening_parenthesis: Box::new(CstKind::OpeningParenthesis.into()),
                    inner: Box::new(build_identifier("foo")),
                    closing_parenthesis: Box::new(CstKind::ClosingParenthesis.into()),
                }
                .into(),
            )),
        );
        assert_eq!(list("foo", 0), None);
        assert_eq!(
            list("(foo", 0),
            Some((
                "",
                CstKind::Parenthesized {
                    opening_parenthesis: Box::new(CstKind::OpeningParenthesis.into()),
                    inner: Box::new(build_identifier("foo")),
                    closing_parenthesis: Box::new(
                        CstKind::Error {
                            unparsable_input: String::new(),
                            error: CstError::ParenthesisNotClosed
                        }
                        .into()
                    ),
                }
                .into(),
            )),
        );
    }
}
