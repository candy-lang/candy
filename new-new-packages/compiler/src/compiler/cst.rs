use std::{
    fmt::{self, Display, Formatter},
    ops::Range,
};

use itertools::Itertools;

use crate::input::Input;

use super::string_to_cst::StringToCst;

#[salsa::query_group(CstDbStorage)]
pub trait CstDb: StringToCst {
    fn find_cst(&self, input: Input, id: Id) -> Cst;
}

fn find_cst(db: &dyn CstDb, input: Input, id: Id) -> Cst {
    db.cst(input).unwrap().find(&id).unwrap().to_owned()
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Hash)]
pub struct Id(pub usize);

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct Cst {
    pub id: Id,
    pub kind: CstKind,
}

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub enum CstKind {
    // Simple characters.
    EqualsSign {
        offset: usize,
    },
    Colon {
        offset: usize,
    },
    Comma {
        offset: usize,
    },
    OpeningParenthesis {
        offset: usize,
    },
    ClosingParenthesis {
        offset: usize,
    },
    OpeningBracket {
        offset: usize,
    },
    ClosingBracket {
        offset: usize,
    },
    OpeningCurlyBrace {
        offset: usize,
    },
    ClosingCurlyBrace {
        offset: usize,
    },
    Arrow {
        offset: usize,
    },

    // Self-contained atoms of the language.
    Int {
        offset: usize,
        value: u64,
        source: String,
    },
    Text {
        offset: usize,
        value: String,
    },
    Identifier {
        offset: usize,
        value: String,
    },
    Symbol {
        offset: usize,
        value: String,
    },

    // Decorators.
    LeadingWhitespace {
        value: String,
        child: Box<Cst>,
    },
    LeadingComment {
        value: String, // without #
        child: Box<Cst>,
    },
    TrailingWhitespace {
        child: Box<Cst>,
        value: String,
    },
    TrailingComment {
        child: Box<Cst>,
        value: String, // without #
    },

    // Compound expressions.
    Parenthesized {
        opening_parenthesis: Box<Cst>,
        inner: Box<Cst>,
        closing_parenthesis: Box<Cst>,
    },
    Struct {
        opening_bracket: Box<Cst>,
        entries: Vec<Cst>,
        closing_bracket: Option<Box<Cst>>,
    },
    StructEntry {
        key: Option<Box<Cst>>,
        colon: Option<Box<Cst>>,
        value: Option<Box<Cst>>,
        comma: Option<Box<Cst>>,
    },
    Lambda {
        opening_curly_brace: Box<Cst>,
        parameters_and_arrow: Option<(Vec<Cst>, Box<Cst>)>,
        body: Vec<Cst>,
        closing_curly_brace: Box<Cst>,
    },
    Call {
        name: Box<Cst>,
        arguments: Vec<Cst>,
    },
    Assignment {
        name: Box<Cst>,
        parameters: Vec<Cst>,
        equals_sign: Box<Cst>,
        body: Vec<Cst>,
    },

    /// Indicates a parsing of some subtree did not succeed.
    Error {
        offset: usize,
        unparsable_input: String,
        message: String,
    },
}

impl Display for Cst {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match &self.kind {
            CstKind::EqualsSign { .. } => write!(f, "="),
            CstKind::Colon { .. } => write!(f, ":"),
            CstKind::Comma { .. } => write!(f, ","),
            CstKind::OpeningParenthesis { .. } => write!(f, "("),
            CstKind::ClosingParenthesis { .. } => write!(f, ")"),
            CstKind::OpeningBracket { .. } => write!(f, "["),
            CstKind::ClosingBracket { .. } => write!(f, "]"),
            CstKind::OpeningCurlyBrace { .. } => write!(f, "{{"),
            CstKind::ClosingCurlyBrace { .. } => write!(f, "}}"),
            CstKind::Arrow { .. } => write!(f, "->"),
            CstKind::Int { source, .. } => write!(f, "{}", source),
            CstKind::Text { value, .. } => write!(f, "\"{}\"", value),
            CstKind::Identifier { value, .. } => write!(f, "{}", value),
            CstKind::Symbol { value, .. } => write!(f, "{}", value),
            CstKind::LeadingWhitespace { value, child } => write!(f, "{}{}", value, child),
            CstKind::LeadingComment { value, child } => write!(f, "{}{}", value, child),
            CstKind::TrailingWhitespace { child, value } => write!(f, "{}{}", child, value),
            CstKind::TrailingComment { child, value } => write!(f, "{}#{}", child, value),
            CstKind::Parenthesized {
                opening_parenthesis,
                inner,
                closing_parenthesis,
            } => write!(f, "{}{}{}", opening_parenthesis, inner, closing_parenthesis),
            CstKind::Struct {
                opening_bracket,
                entries,
                closing_bracket,
            } => {
                write!(f, "{}", opening_bracket)?;
                for entry in entries {
                    write!(f, "{}", entry)?;
                }
                if let Some(bracket) = closing_bracket {
                    write!(f, "{}", bracket)?;
                }
                Ok(())
            }
            CstKind::StructEntry {
                key,
                colon,
                value,
                comma,
            } => {
                if let Some(key) = key {
                    write!(f, "{}", key)?;
                }
                if let Some(colon) = colon {
                    write!(f, "{}", colon)?;
                }
                if let Some(value) = value {
                    write!(f, "{}", value)?;
                }
                if let Some(comma) = comma {
                    write!(f, "{}", comma)?;
                }
                Ok(())
            }
            CstKind::Lambda {
                opening_curly_brace,
                parameters_and_arrow,
                body,
                closing_curly_brace,
            } => write!(
                f,
                "{}{}{}{}",
                opening_curly_brace,
                parameters_and_arrow
                    .as_ref()
                    .map(|(parameters, arrow)| format!(
                        "{}{}",
                        parameters.iter().map(|it| format!("{}", it)).join(""),
                        arrow
                    ))
                    .unwrap_or("".into()),
                body.iter().map(|it| format!("{}", it)).join(""),
                closing_curly_brace,
            ),
            CstKind::Call { name, arguments } => {
                write!(
                    f,
                    "{}{}",
                    name,
                    arguments.iter().map(|it| format!("{}", it)).join("")
                )
            }
            CstKind::Assignment {
                name,
                parameters,
                equals_sign,
                body,
            } => write!(
                f,
                "{}{}{}{}",
                name,
                parameters.iter().map(|it| format!("{}", it)).join(""),
                equals_sign,
                body.iter().map(|it| format!("{}", it)).join(""),
            ),
            CstKind::Error {
                unparsable_input, ..
            } => write!(f, "{}", unparsable_input),
        }
    }
}

impl Cst {
    pub fn span(&self) -> Range<usize> {
        match &self.kind {
            CstKind::EqualsSign { offset } => *offset..(*offset + 1),
            CstKind::Colon { offset } => *offset..(*offset + 1),
            CstKind::Comma { offset } => *offset..(*offset + 1),
            CstKind::OpeningParenthesis { offset } => *offset..(*offset + 1),
            CstKind::ClosingParenthesis { offset } => *offset..(*offset + 1),
            CstKind::OpeningBracket { offset } => *offset..(*offset + 1),
            CstKind::ClosingBracket { offset } => *offset..(*offset + 1),
            CstKind::OpeningCurlyBrace { offset } => *offset..(*offset + 1),
            CstKind::ClosingCurlyBrace { offset } => *offset..(*offset + 1),
            CstKind::Arrow { offset } => *offset..(*offset + 2),
            CstKind::Int { offset, source, .. } => *offset..(*offset + source.len()),
            CstKind::Text { offset, value } => *offset..(*offset + value.len() + 2),
            CstKind::Identifier { offset, value } => *offset..(*offset + value.len()),
            CstKind::Symbol { offset, value } => *offset..(*offset + value.len()),
            CstKind::LeadingWhitespace { value, child } => {
                let child_span = child.span();
                (child_span.start - value.len())..child_span.end
            }
            CstKind::LeadingComment { value, child } => {
                let child_span = child.span();
                (child_span.start - value.len() - 1)..child_span.end
            }
            CstKind::TrailingWhitespace { child, value } => {
                let child_span = child.span();
                child_span.start..(child_span.end + value.len())
            }
            CstKind::TrailingComment { child, value } => {
                let child_span = child.span();
                child_span.start..(child_span.end + value.len() + 1)
            }
            CstKind::Parenthesized {
                opening_parenthesis,
                closing_parenthesis,
                ..
            } => opening_parenthesis.span().start..closing_parenthesis.span().end,
            CstKind::Struct {
                opening_bracket,
                entries,
                closing_bracket,
            } => {
                let start = opening_bracket.span().start;
                let end = closing_bracket
                    .clone()
                    .map(|it| *it)
                    .or_else(|| entries.last().cloned())
                    .unwrap_or(*opening_bracket.clone())
                    .span()
                    .end;
                start..end
            }
            CstKind::StructEntry {
                key,
                colon,
                value,
                comma,
            } => {
                let start = key
                    .to_owned()
                    .or(colon.to_owned())
                    .or(value.to_owned())
                    .or(comma.to_owned())
                    .unwrap()
                    .span()
                    .start;
                let end = comma
                    .to_owned()
                    .or(value.to_owned())
                    .or(colon.to_owned())
                    .or(key.to_owned())
                    .unwrap()
                    .span()
                    .end;
                start..end
            }
            CstKind::Lambda {
                opening_curly_brace,
                closing_curly_brace,
                ..
            } => opening_curly_brace.span().start..closing_curly_brace.span().end,
            CstKind::Call { name, arguments } => {
                if arguments.is_empty() {
                    name.span()
                } else {
                    name.span().start..arguments.last().unwrap().span().end
                }
            }
            CstKind::Assignment {
                name,
                equals_sign,
                body,
                ..
            } => {
                let last_cst = body.last().unwrap_or(&*equals_sign);
                name.span().start..last_cst.span().end
            }
            CstKind::Error {
                offset,
                unparsable_input,
                ..
            } => *offset..(*offset + unparsable_input.len()),
        }
    }

    /// Returns a span that makes sense to display in the editor.
    ///
    /// For example, if a call contains errors, we want to only underline the
    /// name of the called function itself, not everything including arguments.
    pub fn display_span(&self) -> Range<usize> {
        match &self.kind {
            CstKind::LeadingWhitespace { child, .. } => child.display_span(),
            CstKind::LeadingComment { child, .. } => child.display_span(),
            CstKind::TrailingWhitespace { child, .. } => child.display_span(),
            CstKind::TrailingComment { child, .. } => child.display_span(),
            CstKind::Call { name, .. } => name.display_span(),
            CstKind::Assignment { name, .. } => name.display_span(),
            _ => self.span(),
        }
    }

    pub fn unwrap_whitespace_and_comment(&self) -> &Self {
        match &self.kind {
            CstKind::LeadingWhitespace { child, .. } => child.unwrap_whitespace_and_comment(),
            CstKind::LeadingComment { child, .. } => child.unwrap_whitespace_and_comment(),
            CstKind::TrailingWhitespace { child, .. } => child.unwrap_whitespace_and_comment(),
            CstKind::TrailingComment { child, .. } => child.unwrap_whitespace_and_comment(),
            _ => self,
        }
    }
}

trait TreeWithIds {
    fn first_id(&self) -> Option<Id>;
    fn find(&self, id: &Id) -> Option<&Cst>;
}
impl TreeWithIds for Cst {
    fn first_id(&self) -> Option<Id> {
        Some(self.id)
    }
    fn find(&self, id: &Id) -> Option<&Cst> {
        if id == &self.id {
            return Some(self);
        };

        match &self.kind {
            CstKind::EqualsSign { .. } => None,
            CstKind::Colon { .. } => None,
            CstKind::Comma { .. } => None,
            CstKind::OpeningParenthesis { .. } => None,
            CstKind::ClosingParenthesis { .. } => None,
            CstKind::OpeningBracket { .. } => None,
            CstKind::ClosingBracket { .. } => None,
            CstKind::OpeningCurlyBrace { .. } => None,
            CstKind::ClosingCurlyBrace { .. } => None,
            CstKind::Arrow { .. } => None,
            CstKind::Int { .. } => None,
            CstKind::Text { .. } => None,
            CstKind::Identifier { .. } => None,
            CstKind::Symbol { .. } => None,
            CstKind::LeadingWhitespace { child, .. } => child.find(id),
            CstKind::LeadingComment { child, .. } => child.find(id),
            CstKind::TrailingWhitespace { child, .. } => child.find(id),
            CstKind::TrailingComment { child, .. } => child.find(id),
            CstKind::Parenthesized { inner, .. } => inner.find(id),
            CstKind::Struct {
                opening_bracket,
                entries,
                closing_bracket,
            } => opening_bracket
                .find(id)
                .or_else(|| entries.find(id))
                .or_else(|| closing_bracket.find(id)),
            CstKind::StructEntry {
                key,
                colon,
                value,
                comma,
            } => key
                .find(id)
                .or_else(|| colon.find(id))
                .or_else(|| value.find(id))
                .or_else(|| comma.find(id)),
            CstKind::Lambda { body, .. } => body.find(id),
            CstKind::Call { name, arguments } => name.find(id).or_else(|| arguments.find(id)),
            CstKind::Assignment {
                name,
                parameters,
                equals_sign,
                body,
            } => name
                .find(id)
                .or_else(|| parameters.find(id))
                .or_else(|| equals_sign.find(id))
                .or_else(|| body.find(id)),
            CstKind::Error { .. } => None,
        }
    }
}
impl<T: TreeWithIds> TreeWithIds for Option<T> {
    fn first_id(&self) -> Option<Id> {
        self.as_ref().and_then(|it| it.first_id())
    }
    fn find(&self, id: &Id) -> Option<&Cst> {
        self.as_ref().and_then(|it| it.find(id))
    }
}
impl<T: TreeWithIds> TreeWithIds for Box<T> {
    fn first_id(&self) -> Option<Id> {
        self.as_ref().first_id()
    }
    fn find(&self, id: &Id) -> Option<&Cst> {
        self.as_ref().find(id)
    }
}
impl<T: TreeWithIds> TreeWithIds for [T] {
    fn first_id(&self) -> Option<Id> {
        self.iter()
            .map(|it| it.first_id())
            .filter_map(Some)
            .nth(0)
            .flatten()
    }
    fn find(&self, id: &Id) -> Option<&Cst> {
        let slice = self.as_ref();
        let child_index = slice
            .binary_search_by_key(id, |it| it.first_id().unwrap())
            .or_else(|err| if err == 0 { Err(()) } else { Ok(err - 1) })
            .ok()?;
        slice[child_index].find(id)
    }
}
