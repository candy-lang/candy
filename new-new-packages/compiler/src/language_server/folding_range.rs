use lsp_types::{FoldingRange, FoldingRangeKind};

use crate::compiler::{
    cst::{Cst, CstKind},
    string_to_cst::StringToCst,
};

use super::utils::Utf8ByteOffsetToLsp;

pub fn compute_folding_ranges(source: &str) -> Vec<FoldingRange> {
    folding_ranges_for_csts(source, source.parse_cst())
}
fn folding_ranges_for_csts(source: &str, csts: Vec<Cst>) -> Vec<FoldingRange> {
    csts.into_iter()
        .flat_map(|cst| folding_ranges(source, cst))
        .collect()
}
fn folding_ranges(source: &str, cst: Cst) -> Vec<FoldingRange> {
    match cst.kind {
        CstKind::EqualsSign { .. } => vec![],
        CstKind::OpeningParenthesis { .. } => vec![],
        CstKind::ClosingParenthesis { .. } => vec![],
        CstKind::OpeningCurlyBrace { .. } => vec![],
        CstKind::ClosingCurlyBrace { .. } => vec![],
        CstKind::Arrow { .. } => vec![],
        CstKind::Int { .. } => vec![],
        CstKind::Text { .. } => vec![],
        CstKind::Identifier { .. } => vec![],
        CstKind::Symbol { .. } => vec![],
        CstKind::LeadingWhitespace { child, .. } => folding_ranges(source, *child),
        // TODO: support folding ranges for comments
        CstKind::LeadingComment { child, .. } => folding_ranges(source, *child),
        CstKind::TrailingWhitespace { child, .. } => folding_ranges(source, *child),
        CstKind::TrailingComment { child, .. } => folding_ranges(source, *child),
        CstKind::Parenthesized { inner, .. } => folding_ranges(source, *inner),
        CstKind::Lambda {
            opening_curly_brace,
            parameters_and_arrow,
            body,
            closing_curly_brace,
        } => {
            let opening_curly_brace = opening_curly_brace.unwrap_whitespace_and_comment();
            assert!(matches!(
                opening_curly_brace.kind,
                CstKind::OpeningCurlyBrace { .. }
            ));
            let start = opening_curly_brace
                .span()
                .end
                .utf8_byte_offset_to_lsp(source);

            let closing_curly_brace = closing_curly_brace.unwrap_whitespace_and_comment();
            assert!(matches!(
                closing_curly_brace.kind,
                CstKind::ClosingCurlyBrace { .. }
            ));
            let end = closing_curly_brace
                .span()
                .start
                .utf8_byte_offset_to_lsp(source);

            let mut ranges = vec![FoldingRange {
                start_line: start.line,
                start_character: Some(start.character),
                end_line: end.line,
                end_character: Some(end.character),
                kind: Some(FoldingRangeKind::Region),
            }];
            if let Some((parameters, _)) = parameters_and_arrow {
                ranges.append(&mut folding_ranges_for_csts(source, parameters));
            }
            ranges.append(&mut folding_ranges_for_csts(source, body));
            ranges
        }
        CstKind::Call { name, arguments } => {
            let mut ranges = vec![];

            if !arguments.is_empty() {
                let name = name.unwrap_whitespace_and_comment();
                assert!(matches!(name.kind, CstKind::Identifier { .. }));
                let start = name.span().end.utf8_byte_offset_to_lsp(source);

                let last_argument = arguments.last().unwrap().unwrap_whitespace_and_comment();
                let end = last_argument.span().end.utf8_byte_offset_to_lsp(source);

                if start.line != end.line {
                    ranges.push(FoldingRange {
                        start_line: start.line,
                        start_character: Some(start.character),
                        end_line: end.line,
                        end_character: Some(end.character),
                        kind: Some(FoldingRangeKind::Region),
                    });
                }
            }

            ranges.append(&mut folding_ranges(source, *name));
            ranges.append(&mut folding_ranges_for_csts(source, arguments));
            ranges
        }
        CstKind::Assignment {
            name,
            equals_sign,
            parameters,
            body,
        } => {
            let mut ranges = vec![];

            if !body.is_empty() {
                let equals_sign = equals_sign.unwrap_whitespace_and_comment();
                assert!(matches!(equals_sign.kind, CstKind::EqualsSign { .. }));
                let start = equals_sign.span().end.utf8_byte_offset_to_lsp(source);

                let last_expression = body.last().unwrap().unwrap_whitespace_and_comment();
                let end = last_expression.span().end.utf8_byte_offset_to_lsp(source);

                if start.line != end.line {
                    ranges.push(FoldingRange {
                        start_line: start.line,
                        start_character: Some(start.character),
                        end_line: end.line,
                        end_character: Some(end.character),
                        kind: Some(FoldingRangeKind::Region),
                    });
                }
            }

            ranges.append(&mut folding_ranges(source, *name));
            ranges.append(&mut folding_ranges_for_csts(source, parameters));
            ranges.append(&mut folding_ranges_for_csts(source, body));
            ranges
        }
        CstKind::Error { .. } => vec![],
    }
}
