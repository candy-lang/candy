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
    EscapeMissesChar,
    EscapeWithInvalidChar,
    UrlInvalid,
    WeirdWhitespace,
    WeirdWhitespaceInIndentation,
}

impl Display for Rcst {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Whitespace(whitespace) => whitespace.fmt(f),
            Self::Newline => '\n'.fmt(f),
            Self::TrailingWhitespace { child, whitespace } => {
                child.fmt(f)?;
                for w in whitespace {
                    w.fmt(f)?;
                }
                Ok(())
            }
            Self::TextPart(text) => text.fmt(f),
            Self::EscapedChar(character) => {
                '\\'.fmt(f)?;
                if let Some(character) = character {
                    character.fmt(f)?;
                }
                Ok(())
            }
            Self::Emphasized {
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
            Self::Link {
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
            Self::InlineCode {
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
            Self::Title(lines) => {
                for line in lines {
                    line.fmt(f)?;
                }
                Ok(())
            }
            Self::TitleLine {
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
            Self::Paragraph(text) => {
                for rcst in text {
                    rcst.fmt(f)?;
                }
                Ok(())
            }
            Self::Urls(urls) => {
                for url in urls {
                    url.fmt(f)?;
                }
                Ok(())
            }
            Self::UrlLine(url) => url.fmt(f),
            Self::CodeBlock {
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
            Self::List(items) => {
                for item in items {
                    item.fmt(f)?;
                }
                Ok(())
            }
            Self::ListItem { marker, content } => {
                marker.fmt(f)?;
                for rcst in content {
                    rcst.fmt(f)?;
                }
                Ok(())
            }
            Self::Error { child, .. } => {
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
            Self::Unordered { has_trailing_space } => {
                '-'.fmt(f)?;
                if *has_trailing_space {
                    ' '.fmt(f)?;
                }
                Ok(())
            }
            Self::Ordered {
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
