use candy_frontend::cst::{Cst, CstKind};
use extension_trait::extension_trait;
use itertools::{FoldWhile, Itertools};
use unicode_width::UnicodeWidthStr;

#[extension_trait]
pub impl<D> LastLineWidth for Cst<D> {
    fn last_line_width(&self) -> usize {
        self.last_line_width_info().last_line_width
    }
}

struct LastLineWidthInfo {
    is_multiline: bool,
    last_line_width: usize,
}
impl LastLineWidthInfo {
    fn singleline(width: usize) -> Self {
        Self {
            is_multiline: false,
            last_line_width: width,
        }
    }
    fn multiline(width: usize) -> Self {
        Self {
            is_multiline: true,
            last_line_width: width,
        }
    }

    fn preceded_by(self, get_other: impl FnOnce() -> LastLineWidthInfo) -> Self {
        if self.is_multiline {
            return self;
        }

        let other = get_other();
        Self {
            is_multiline: other.is_multiline,
            last_line_width: if other.is_multiline {
                other.last_line_width
            } else {
                self.last_line_width + other.last_line_width
            },
        }
    }
}
impl Default for LastLineWidthInfo {
    fn default() -> Self {
        LastLineWidthInfo::singleline(0)
    }
}

trait HasLastLineWidthInfo {
    fn last_line_width_info(&self) -> LastLineWidthInfo;
}
impl HasLastLineWidthInfo for String {
    fn last_line_width_info(&self) -> LastLineWidthInfo {
        // Our CST doesn't contain any multiline strings.
        LastLineWidthInfo::singleline(self.width())
    }
}
impl<D> HasLastLineWidthInfo for Cst<D> {
    fn last_line_width_info(&self) -> LastLineWidthInfo {
        match &self.kind {
            CstKind::EqualsSign | CstKind::Comma | CstKind::Dot | CstKind::Colon => {
                LastLineWidthInfo::singleline(1)
            }
            CstKind::ColonEqualsSign => LastLineWidthInfo::singleline(2),
            CstKind::Bar
            | CstKind::OpeningParenthesis
            | CstKind::ClosingParenthesis
            | CstKind::OpeningBracket
            | CstKind::ClosingBracket
            | CstKind::OpeningCurlyBrace
            | CstKind::ClosingCurlyBrace => LastLineWidthInfo::singleline(1),
            CstKind::Arrow => LastLineWidthInfo::singleline(2),
            CstKind::SingleQuote
            | CstKind::DoubleQuote
            | CstKind::Percent
            | CstKind::Octothorpe => LastLineWidthInfo::singleline(1),
            CstKind::Whitespace(whitespace) => whitespace.last_line_width_info(),
            CstKind::Newline(_) => LastLineWidthInfo::multiline(0),
            CstKind::Comment {
                octothorpe,
                comment,
            } => comment
                .last_line_width_info()
                .preceded_by(|| octothorpe.last_line_width_info()),
            CstKind::TrailingWhitespace { child, whitespace } => whitespace
                .last_line_width_info()
                .preceded_by(|| child.last_line_width_info()),
            CstKind::Identifier(string) | CstKind::Symbol(string) | CstKind::Int { string, .. } => {
                string.last_line_width_info()
            }
            CstKind::OpeningText {
                opening_single_quotes,
                opening_double_quote,
            } => opening_double_quote
                .last_line_width_info()
                .preceded_by(|| opening_single_quotes.last_line_width_info()),
            CstKind::ClosingText {
                closing_double_quote,
                closing_single_quotes,
            } => closing_double_quote
                .last_line_width_info()
                .preceded_by(|| closing_single_quotes.last_line_width_info()),
            CstKind::Text {
                opening,
                parts,
                closing,
            } => closing
                .last_line_width_info()
                .preceded_by(|| parts.last_line_width_info())
                .preceded_by(|| opening.last_line_width_info()),
            CstKind::TextPart(part) => part.last_line_width_info(),
            CstKind::TextInterpolation {
                opening_curly_braces,
                expression,
                closing_curly_braces,
            } => closing_curly_braces
                .last_line_width_info()
                .preceded_by(|| expression.last_line_width_info())
                .preceded_by(|| opening_curly_braces.last_line_width_info()),
            CstKind::BinaryBar { left, bar, right } => right
                .last_line_width_info()
                .preceded_by(|| bar.last_line_width_info())
                .preceded_by(|| left.last_line_width_info()),
            CstKind::Parenthesized {
                opening_parenthesis,
                inner,
                closing_parenthesis,
            } => closing_parenthesis
                .last_line_width_info()
                .preceded_by(|| inner.last_line_width_info())
                .preceded_by(|| opening_parenthesis.last_line_width_info()),
            CstKind::Call {
                receiver,
                arguments,
            } => arguments
                .last_line_width_info()
                .preceded_by(|| receiver.last_line_width_info()),
            CstKind::List {
                opening_parenthesis,
                items,
                closing_parenthesis,
            } => closing_parenthesis
                .last_line_width_info()
                .preceded_by(|| items.last_line_width_info())
                .preceded_by(|| opening_parenthesis.last_line_width_info()),
            CstKind::ListItem { value, comma } => comma
                .last_line_width_info()
                .preceded_by(|| value.last_line_width_info()),
            CstKind::Struct {
                opening_bracket,
                fields,
                closing_bracket,
            } => closing_bracket
                .last_line_width_info()
                .preceded_by(|| fields.last_line_width_info())
                .preceded_by(|| opening_bracket.last_line_width_info()),
            CstKind::StructField {
                key_and_colon,
                value,
                comma,
            } => comma
                .last_line_width_info()
                .preceded_by(|| value.last_line_width_info())
                .preceded_by(|| key_and_colon.last_line_width_info()),
            CstKind::StructAccess { struct_, dot, key } => key
                .last_line_width_info()
                .preceded_by(|| dot.last_line_width_info())
                .preceded_by(|| struct_.last_line_width_info()),
            CstKind::Match {
                expression,
                percent,
                cases,
            } => cases
                .last_line_width_info()
                .preceded_by(|| percent.last_line_width_info())
                .preceded_by(|| expression.last_line_width_info()),
            CstKind::MatchCase {
                pattern,
                arrow,
                body,
            } => body
                .last_line_width_info()
                .preceded_by(|| arrow.last_line_width_info())
                .preceded_by(|| pattern.last_line_width_info()),
            CstKind::Lambda {
                opening_curly_brace,
                parameters_and_arrow,
                body,
                closing_curly_brace,
            } => closing_curly_brace
                .last_line_width_info()
                .preceded_by(|| body.last_line_width_info())
                .preceded_by(|| parameters_and_arrow.last_line_width_info())
                .preceded_by(|| opening_curly_brace.last_line_width_info()),
            CstKind::Assignment {
                left,
                assignment_sign,
                body,
            } => body
                .last_line_width_info()
                .preceded_by(|| assignment_sign.last_line_width_info())
                .preceded_by(|| left.last_line_width_info()),
            CstKind::Error {
                unparsable_input, ..
            } => unparsable_input.last_line_width_info(),
        }
    }
}
impl<C: HasLastLineWidthInfo> HasLastLineWidthInfo for Box<C> {
    fn last_line_width_info(&self) -> LastLineWidthInfo {
        self.as_ref().last_line_width_info()
    }
}
impl<C: HasLastLineWidthInfo> HasLastLineWidthInfo for Option<C> {
    fn last_line_width_info(&self) -> LastLineWidthInfo {
        self.as_ref()
            .map(|it| it.last_line_width_info())
            .unwrap_or_default()
    }
}
impl<C0: HasLastLineWidthInfo, C1: HasLastLineWidthInfo> HasLastLineWidthInfo for (C0, C1) {
    fn last_line_width_info(&self) -> LastLineWidthInfo {
        self.1
            .last_line_width_info()
            .preceded_by(|| self.0.last_line_width_info())
    }
}

impl<C: HasLastLineWidthInfo> HasLastLineWidthInfo for Vec<C> {
    fn last_line_width_info(&self) -> LastLineWidthInfo {
        let result = self.iter().rev().fold_while(0, |acc, child| {
            let child_width = child.last_line_width_info();
            if child_width.is_multiline {
                FoldWhile::Done(acc + child_width.last_line_width)
            } else {
                FoldWhile::Continue(acc + child_width.last_line_width)
            }
        });
        match result {
            FoldWhile::Done(width) => LastLineWidthInfo::multiline(width),
            FoldWhile::Continue(width) => LastLineWidthInfo::singleline(width),
        }
    }
}
