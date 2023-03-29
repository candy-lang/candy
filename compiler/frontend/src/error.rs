use enumset::EnumSet;

use super::{ast::AstError, cst, cst::CstError, hir::HirError};
use crate::{
    module::Module,
    position::Offset,
    rich_ir::{ReferenceKey, RichIrBuilder, ToRichIr},
};
use std::{fmt::Display, hash::Hash, ops::Range};

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct CompilerError {
    pub module: Module,
    pub span: Range<Offset>,
    pub payload: CompilerErrorPayload,
}

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub enum CompilerErrorPayload {
    InvalidUtf8,
    Cst(CstError),
    Ast(AstError),
    Hir(HirError),
}
impl Display for CompilerErrorPayload {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let message = match self {
            CompilerErrorPayload::InvalidUtf8 => "The module contains invalid UTF-8.".to_string(),
            CompilerErrorPayload::Cst(error) => match error {
                CstError::BinaryBarMissesRight => "There should be a right side after this bar.",
                CstError::CurlyBraceNotClosed => "The curly brace is not closed.",
                CstError::IdentifierContainsNonAlphanumericAscii => {
                    "This identifier contains non-alphanumeric ASCII characters."
                }
                CstError::IntContainsNonDigits => {
                    "This integer contains characters that are not digits."
                }
                CstError::ListItemMissesValue => "This list item is missing a value.",
                CstError::ListNotClosed => "The list is not closed.",
                CstError::MatchMissesCases => "This match misses cases to match against.",
                CstError::MatchCaseMissesArrow => "This match case misses an arrow.",
                CstError::MatchCaseMissesBody => "This match case misses a body to run.",
                CstError::OpeningParenthesisMissesExpression => {
                    "Here's an opening parenthesis without an expression after it."
                }
                CstError::OrPatternMissesRight => "This or-pattern misses a right-hand side.",
                CstError::ParenthesisNotClosed => "This parenthesis isn't closed.",
                CstError::StructFieldMissesColon => "This struct field misses a colon.",
                CstError::StructFieldMissesKey => "This struct field misses a key.",
                CstError::StructFieldMissesValue => "This struct field misses a value.",
                CstError::StructNotClosed => "This struct is not closed.",
                CstError::SymbolContainsNonAlphanumericAscii => {
                    "This symbol contains non-alphanumeric ASCII characters."
                }
                CstError::TextNotClosed => "This text isn't closed.",
                CstError::TextNotSufficientlyIndented => "This text isn't sufficiently indented.",
                CstError::TextInterpolationNotClosed => "This text interpolation isn't closed.",
                CstError::TextInterpolationMissesExpression => {
                    "Here's a start of a text interpolation without an expression after it."
                }
                CstError::TooMuchWhitespace => "There is too much whitespace here.",
                CstError::UnexpectedCharacters => "This is an unexpected character.",
                CstError::UnparsedRest => "The parser couldn't parse this rest.",
                CstError::WeirdWhitespace => "This is weird whitespace.",
                CstError::WeirdWhitespaceInIndentation => {
                    "This is weird whitespace. Make sure to use indent using two spaces."
                }
            }
            .to_string(),
            CompilerErrorPayload::Ast(error) => match error {
                AstError::CallInPattern => "Calls in patterns are not allowed.".to_string(),
                AstError::ExpectedNameOrPatternInAssignment => {
                    "An assignment should have a name or pattern on the left side.".to_string()
                }
                AstError::ExpectedParameter => "A parameter should come here.".to_string(),
                AstError::LambdaMissesClosingCurlyBrace => {
                    "This lambda doesn't have a closing curly brace.".to_string()
                }
                AstError::ListItemMissesComma => {
                    "This list item should be followed by a comma.".to_string()
                }
                AstError::ListMissesClosingParenthesis => {
                    "This list doesn't have a closing parenthesis.".to_string()
                }
                AstError::ListWithNonListItem => "This is not a list item.".to_string(),
                AstError::OrPatternIsMissingIdentifiers {
                    identifier,
                    number_of_missing_captures,
                    ..
                } => {
                    format!(
                        "`{identifier}` is missing in {number_of_missing_captures} {} of this or-pattern.",
                        if number_of_missing_captures.get() == 1 { "sub-pattern" } else { "sub-patterns" },
                    )
                }
                AstError::ParenthesizedInPattern => {
                    "Parentheses are not allowed in patterns.".to_string()
                }
                AstError::ParenthesizedMissesClosingParenthesis => {
                    "This expression is parenthesized, but the closing parenthesis is missing."
                        .to_string()
                }
                AstError::PatternContainsInvalidExpression => {
                    "This type of expression is not allowed in patterns.".to_string()
                }
                AstError::PatternLiteralPartContainsInvalidExpression => {
                    "This type of expression is not allowed in this part of a pattern.".to_string()
                }
                AstError::PipeInPattern => "Pipes are not allowed in patterns.".to_string(),
                AstError::StructKeyMissesColon => {
                    "This struct key should be followed by a colon.".to_string()
                }
                AstError::StructMissesClosingBrace => {
                    "This struct doesn't have a closing bracket.".to_string()
                }
                AstError::StructShorthandWithNotIdentifier => {
                    "Shorthand syntax in structs only supports identifiers.".to_string()
                }
                AstError::StructValueMissesComma => {
                    "This struct value should be followed by a comma.".to_string()
                }
                AstError::StructWithNonStructField => {
                    "Structs should only contain struct key.".to_string()
                }
                AstError::TextInterpolationMissesClosingCurlyBraces => {
                    "This text interpolation never ends.".to_string()
                }
                AstError::TextMissesClosingQuote => "This text never ends.".to_string(),
                AstError::UnexpectedPunctuation => "This punctuation was unexpected.".to_string(),
            },
            CompilerErrorPayload::Hir(error) => match error {
                HirError::NeedsWithWrongNumberOfArguments { num_args } => {
                    format!("`needs` accepts one or two arguments, but was called with {num_args} arguments. Its parameters are the `condition` and an optional `message`.")
                }
                HirError::PublicAssignmentInNotTopLevel => {
                    "Public assignments (:=) can only be used in top-level code.".to_string()
                }
                HirError::PublicAssignmentWithSameName { name } => {
                    format!("There already exists a public assignment (:=) named `{name}`.")
                }
                HirError::UnknownReference { name } => format!("`{name}` is not in scope."),
            },
        };
        write!(f, "{message}")
    }
}
impl CompilerError {
    pub fn to_related_information(&self) -> Vec<(Module, cst::Id, String)> {
        match &self.payload {
            CompilerErrorPayload::Ast(AstError::OrPatternIsMissingIdentifiers {
                all_captures,
                ..
            }) => all_captures
                .iter()
                .map(|capture| {
                    (
                        self.module.clone(),
                        capture.to_owned(),
                        "The identifier is bound here.".to_string(),
                    )
                })
                .collect(),
            _ => vec![],
        }
    }
}

impl ToRichIr for CompilerError {
    fn build_rich_ir(&self, builder: &mut RichIrBuilder) {
        let range = builder.push(
            format!(
                "{} (span: {} – {})",
                self.module.to_rich_ir(),
                *self.span.start,
                *self.span.end,
            ),
            None,
            EnumSet::empty(),
        );
        builder.push_reference(
            ReferenceKey::ModuleWithSpan(self.module.clone(), self.span.to_owned()),
            range,
        );
        builder.push(format!(": {}", self.payload), None, EnumSet::empty());
    }
}
