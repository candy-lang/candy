use candy_frontend::{
    cst::{Cst, CstKind},
    module::{Module, ModuleDb},
    position::PositionConversionDb,
    rcst_to_cst::RcstToCst,
};
use lazy_static::lazy_static;
use lsp_types::{self, SemanticToken, SemanticTokensLegend};
use strum::IntoEnumIterator;

use crate::semantic_tokens::{SemanticTokenType, SemanticTokensBuilder};

pub fn semantic_tokens<DB: ModuleDb + PositionConversionDb + RcstToCst>(
    db: &DB,
    module: Module,
) -> Vec<SemanticToken> {
    let text = db.get_module_content_as_string(module.clone()).unwrap();
    let line_start_offsets = db.line_start_offsets(module.clone());
    let mut builder = SemanticTokensBuilder::new(&*text, &*line_start_offsets);
    let cst = db.cst(module).unwrap();
    visit_csts(&mut builder, &cst, None);
    builder.finish()
}

lazy_static! {
    pub static ref LEGEND: SemanticTokensLegend = SemanticTokensLegend {
        token_types: SemanticTokenType::iter().map(|it| it.as_lsp()).collect(),
        token_modifiers: vec![
            lsp_types::SemanticTokenModifier::DEFINITION,
            lsp_types::SemanticTokenModifier::READONLY,
        ],
    };
}

fn visit_csts(
    builder: &mut SemanticTokensBuilder<'_>,
    csts: &[Cst],
    token_type_for_identifier: Option<SemanticTokenType>,
) {
    for cst in csts {
        visit_cst(builder, cst, token_type_for_identifier)
    }
}
fn visit_cst(
    builder: &mut SemanticTokensBuilder<'_>,
    cst: &Cst,
    token_type_for_identifier: Option<SemanticTokenType>,
) {
    match &cst.kind {
        CstKind::EqualsSign => builder.add(cst.span.clone(), SemanticTokenType::Operator),
        CstKind::Comma
        | CstKind::Dot
        | CstKind::Colon
        | CstKind::ColonEqualsSign
        | CstKind::Bar
        | CstKind::OpeningParenthesis
        | CstKind::ClosingParenthesis
        | CstKind::OpeningBracket
        | CstKind::ClosingBracket
        | CstKind::OpeningCurlyBrace
        | CstKind::ClosingCurlyBrace => {}
        CstKind::Arrow => builder.add(cst.span.clone(), SemanticTokenType::Operator),
        CstKind::SingleQuote => {} // handled by parent
        CstKind::DoubleQuote => {} // handled by parent
        CstKind::Percent => builder.add(cst.span.clone(), SemanticTokenType::Operator),
        CstKind::Octothorpe => {} // handled by parent
        CstKind::Whitespace(_) | CstKind::Newline(_) => {}
        CstKind::Comment { octothorpe, .. } => {
            visit_cst(builder, octothorpe, None);
            builder.add(cst.span.clone(), SemanticTokenType::Comment);
        }
        CstKind::TrailingWhitespace { child, whitespace } => {
            visit_cst(builder, child, token_type_for_identifier);
            visit_csts(builder, whitespace, token_type_for_identifier);
        }
        CstKind::Identifier { .. } => builder.add(
            cst.span.clone(),
            token_type_for_identifier.unwrap_or(SemanticTokenType::Variable),
        ),
        CstKind::Symbol { .. } => builder.add(cst.span.clone(), SemanticTokenType::Symbol),
        CstKind::Int { .. } => builder.add(cst.span.clone(), SemanticTokenType::Int),
        CstKind::OpeningText {
            opening_single_quotes,
            opening_double_quote,
        } => {
            for opening_single_quote in opening_single_quotes {
                builder.add(opening_single_quote.span.clone(), SemanticTokenType::Text);
            }
            builder.add(opening_double_quote.span.clone(), SemanticTokenType::Text);
        }
        CstKind::ClosingText {
            closing_double_quote,
            closing_single_quotes,
        } => {
            builder.add(closing_double_quote.span.clone(), SemanticTokenType::Text);
            for closing_single_quote in closing_single_quotes {
                builder.add(closing_single_quote.span.clone(), SemanticTokenType::Text);
            }
        }
        CstKind::Text {
            opening,
            parts,
            closing,
        } => {
            visit_cst(builder, opening, None);
            for part in parts {
                visit_cst(builder, part, None);
            }
            visit_cst(builder, closing, None);
        }
        CstKind::TextPart(_) => builder.add(cst.span.clone(), SemanticTokenType::Text),
        CstKind::TextInterpolation {
            opening_curly_braces,
            expression,
            closing_curly_braces,
        } => {
            for opening_curly_brace in opening_curly_braces {
                visit_cst(builder, opening_curly_brace, None);
            }
            visit_cst(builder, expression, None);
            for closing_curly_brace in closing_curly_braces {
                visit_cst(builder, closing_curly_brace, None);
            }
        }
        CstKind::Pipe {
            receiver,
            bar,
            call,
        } => {
            visit_cst(builder, receiver, None);
            visit_cst(builder, bar, None);
            visit_cst(builder, call, None);
        }
        CstKind::Parenthesized {
            opening_parenthesis,
            inner,
            closing_parenthesis,
        } => {
            visit_cst(builder, opening_parenthesis, None);
            visit_cst(builder, inner, None);
            visit_cst(builder, closing_parenthesis, None);
        }
        CstKind::Call {
            receiver,
            arguments,
        } => {
            visit_cst(builder, receiver, Some(SemanticTokenType::Function));
            visit_csts(builder, arguments, None);
        }
        CstKind::List {
            opening_parenthesis,
            items,
            closing_parenthesis,
        } => {
            visit_cst(builder, opening_parenthesis, None);
            visit_csts(builder, items, token_type_for_identifier);
            visit_cst(builder, closing_parenthesis, None);
        }
        CstKind::ListItem { value, comma } => {
            visit_cst(builder, value, token_type_for_identifier);
            if let Some(comma) = comma {
                visit_cst(builder, comma, None);
            }
        }
        CstKind::Struct {
            opening_bracket,
            fields,
            closing_bracket,
        } => {
            visit_cst(builder, opening_bracket, None);
            visit_csts(builder, fields, token_type_for_identifier);
            visit_cst(builder, closing_bracket, None);
        }
        CstKind::StructField {
            key_and_colon,
            value,
            comma,
        } => {
            if let Some(box (key, colon)) = key_and_colon {
                visit_cst(builder, key, token_type_for_identifier);
                visit_cst(builder, colon, None);
            }
            visit_cst(builder, value, token_type_for_identifier);
            if let Some(comma) = comma {
                visit_cst(builder, comma, None);
            }
        }
        CstKind::StructAccess { struct_, dot, key } => {
            visit_cst(builder, struct_, None);
            visit_cst(builder, dot, None);
            visit_cst(
                builder,
                key,
                Some(token_type_for_identifier.unwrap_or(SemanticTokenType::Symbol)),
            );
        }
        CstKind::Match {
            expression,
            percent,
            cases,
        } => {
            visit_cst(builder, expression, None);
            visit_cst(builder, percent, None);
            visit_csts(builder, cases, None);
        }
        CstKind::MatchCase {
            pattern,
            arrow,
            body,
        } => {
            visit_cst(builder, pattern, None);
            visit_cst(builder, arrow, None);
            visit_csts(builder, body, None);
        }
        CstKind::OrPattern { left, right } => {
            visit_cst(builder, left, None);
            for (bar, pattern) in right {
                visit_cst(builder, bar, None);
                visit_cst(builder, pattern, None);
            }
        }
        CstKind::Lambda {
            opening_curly_brace,
            parameters_and_arrow,
            body,
            closing_curly_brace,
        } => {
            visit_cst(builder, opening_curly_brace, None);
            if let Some((parameters, arrow)) = parameters_and_arrow {
                visit_csts(builder, parameters, Some(SemanticTokenType::Parameter));
                visit_cst(builder, arrow, None);
            }
            visit_csts(builder, body, None);
            visit_cst(builder, closing_curly_brace, None);
        }
        CstKind::Assignment {
            name_or_pattern,
            parameters,
            assignment_sign,
            body,
        } => {
            visit_cst(builder, name_or_pattern, Some(SemanticTokenType::Variable));
            visit_csts(builder, &parameters[..], Some(SemanticTokenType::Parameter));
            visit_cst(builder, assignment_sign, None);
            visit_csts(builder, body, None);
        }
        CstKind::Error { .. } => {}
    }
}
