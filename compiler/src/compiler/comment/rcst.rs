use std::fmt::{self, Display, Formatter};
use url::Url;

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub enum Rcst {
    Whitespace(String),
    Newline,
    TrailingWhitespace {
        child: Box<Rcst>,
        whitespace: Vec<Rcst>,
    },

    // Inline Elements
    TextPart(String),
    EscapedChar(Option<char>),
    Emphasized {
        has_opening_underscore: bool,
        text: Vec<Rcst>,
        has_closing_underscore: bool,
    },
    Link {
        has_opening_bracket: bool,
        text: Vec<Rcst>,
        has_closing_bracket: bool,
    },
    InlineCode {
        has_opening_backtick: bool,
        code: Vec<Rcst>,
        has_closing_backtick: bool,
    },

    // Block Elements
    Title(Vec<Rcst>),
    TitleLine {
        octothorpe_count: usize,
        text: Vec<Rcst>,
    },

    Paragraph(Vec<Rcst>),

    Urls(Vec<Rcst>),
    UrlLine(Url),

    CodeBlock {
        code: Vec<Rcst>,
        has_closing_backticks: bool,
    },

    List(Vec<Rcst>),
    ListItem {
        marker: RcstListItemMarker,
        content: Vec<Rcst>,
    },

    Error {
        child: Option<Box<Rcst>>,
        error: RcstError,
    },
}

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub enum RcstListItemMarker {
    Unordered {
        has_trailing_space: bool,
    },
    Ordered {
        number: Box<Rcst>,
        has_trailing_space: bool,
    },
}

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub enum RcstError {
    EscapeWithoutChar,
    EscapeWithInvalidChar,
    UrlInvalid,
    WeirdWhitespace,
    WeirdWhitespaceInIndentation,
}

impl Display for Rcst {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Rcst::Whitespace(whitespace) => whitespace.fmt(f),
            Rcst::Newline => '\n'.fmt(f),
            Rcst::TrailingWhitespace { child, whitespace } => {
                child.fmt(f)?;
                for w in whitespace {
                    w.fmt(f)?;
                }
                Ok(())
            }
            Rcst::TextPart(text) => text.fmt(f),
            Rcst::EscapedChar(character) => {
                '\\'.fmt(f)?;
                if let Some(character) = character {
                    character.fmt(f)?;
                }
                Ok(())
            }
            Rcst::Emphasized {
                has_opening_underscore,
                text,
                has_closing_underscore,
            } => {
                if *has_opening_underscore {
                    '_'.fmt(f)?;
                }
                for rcst in text {
                    rcst.fmt(f)?;
                }
                if *has_closing_underscore {
                    '_'.fmt(f)?;
                }
                Ok(())
            }
            Rcst::Link {
                has_opening_bracket,
                text,
                has_closing_bracket,
            } => {
                if *has_opening_bracket {
                    '['.fmt(f)?;
                }
                for rcst in text {
                    rcst.fmt(f)?;
                }
                if *has_closing_bracket {
                    ']'.fmt(f)?;
                }
                Ok(())
            }
            Rcst::InlineCode {
                has_opening_backtick,
                code,
                has_closing_backtick,
            } => {
                if *has_opening_backtick {
                    '`'.fmt(f)?;
                }
                for rcst in code {
                    rcst.fmt(f)?;
                }
                if *has_closing_backtick {
                    '`'.fmt(f)?;
                }
                Ok(())
            }
            Rcst::Title(lines) => {
                for line in lines {
                    line.fmt(f)?;
                }
                Ok(())
            }
            Rcst::TitleLine {
                octothorpe_count,
                text,
            } => {
                for _ in 0..*octothorpe_count {
                    '#'.fmt(f)?;
                }
                for rcst in text {
                    rcst.fmt(f)?;
                }
                Ok(())
            }
            Rcst::Paragraph(text) => {
                for rcst in text {
                    rcst.fmt(f)?;
                }
                Ok(())
            }
            Rcst::Urls(urls) => {
                for url in urls {
                    url.fmt(f)?;
                }
                Ok(())
            }
            Rcst::UrlLine(url) => url.fmt(f),
            Rcst::CodeBlock {
                code,
                has_closing_backticks,
            } => {
                "```".fmt(f)?;
                for rcst in code {
                    rcst.fmt(f)?;
                }
                if *has_closing_backticks {
                    "```".fmt(f)?;
                }
                Ok(())
            }
            Rcst::List(items) => {
                for item in items {
                    item.fmt(f)?;
                }
                Ok(())
            }
            Rcst::ListItem { marker, content } => {
                marker.fmt(f)?;
                for rcst in content {
                    rcst.fmt(f)?;
                }
                Ok(())
            }
            Rcst::Error { child, .. } => {
                if let Some(child) = child {
                    child.fmt(f)?;
                }
                Ok(())
            }
        }
    }
}

impl Display for RcstListItemMarker {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            RcstListItemMarker::Unordered { has_trailing_space } => {
                '-'.fmt(f)?;
                if *has_trailing_space {
                    ' '.fmt(f)?;
                }
                Ok(())
            }
            RcstListItemMarker::Ordered {
                number,
                has_trailing_space,
            } => {
                number.fmt(f)?;
                '.'.fmt(f)?;
                if *has_trailing_space {
                    ' '.fmt(f)?;
                }
                Ok(())
            }
        }
    }
}
