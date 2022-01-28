use lsp_types::{FoldingRange, FoldingRangeKind};

use crate::{
    compiler::{
        cst::{Cst, CstKind},
        string_to_cst::StringToCst,
    },
    input::InputReference,
};

use super::utils::Utf8ByteOffsetToLsp;

#[salsa::query_group(FoldingRangeDbStorage)]
pub trait FoldingRangeDb: StringToCst {
    fn folding_ranges(&self, input_reference: InputReference) -> Vec<FoldingRange>;
}

fn folding_ranges(db: &dyn FoldingRangeDb, input_reference: InputReference) -> Vec<FoldingRange> {
    let source = db.get_input(input_reference.clone()).unwrap();
    let mut context = Context::new(&source);
    context.visit_csts(&db.cst(input_reference).unwrap());
    context.ranges
}

struct Context<'a> {
    source: &'a str,
    ranges: Vec<FoldingRange>,
}
impl<'a> Context<'a> {
    fn new(source: &'a str) -> Self {
        Context {
            source,
            ranges: vec![],
        }
    }

    fn visit_csts(&mut self, csts: &[Cst]) {
        for cst in csts {
            self.visit_cst(cst);
        }
    }
    fn visit_cst(&mut self, cst: &Cst) {
        match &cst.kind {
            CstKind::EqualsSign { .. } => {}
            CstKind::OpeningParenthesis { .. } => {}
            CstKind::ClosingParenthesis { .. } => {}
            CstKind::OpeningCurlyBrace { .. } => {}
            CstKind::ClosingCurlyBrace { .. } => {}
            CstKind::Arrow { .. } => {}
            CstKind::Int { .. } => {}
            CstKind::Text { .. } => {}
            CstKind::Identifier { .. } => {}
            CstKind::Symbol { .. } => {}
            CstKind::LeadingWhitespace { child, .. } => self.visit_cst(child),
            // TODO: support folding ranges for comments
            CstKind::LeadingComment { child, .. } => self.visit_cst(child),
            CstKind::TrailingWhitespace { child, .. } => self.visit_cst(child),
            CstKind::TrailingComment { child, .. } => self.visit_cst(child),
            CstKind::Parenthesized { inner, .. } => self.visit_cst(inner),
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

                let closing_curly_brace = closing_curly_brace.unwrap_whitespace_and_comment();
                assert!(matches!(
                    closing_curly_brace.kind,
                    CstKind::ClosingCurlyBrace { .. }
                ));

                self.push(
                    opening_curly_brace.span().end,
                    closing_curly_brace.span().start,
                    FoldingRangeKind::Region,
                );
                if let Some((parameters, _)) = parameters_and_arrow {
                    self.visit_csts(&parameters);
                }
                self.visit_csts(&body);
            }
            CstKind::Call { name, arguments } => {
                if !arguments.is_empty() {
                    let name = name.unwrap_whitespace_and_comment();
                    assert!(matches!(name.kind, CstKind::Identifier { .. }));

                    let last_argument = arguments.last().unwrap().unwrap_whitespace_and_comment();

                    self.push(
                        name.span().end,
                        last_argument.span().end,
                        FoldingRangeKind::Region,
                    );
                }

                self.visit_cst(name);
                self.visit_csts(&arguments);
            }
            CstKind::Assignment {
                name,
                equals_sign,
                parameters,
                body,
            } => {
                if !body.is_empty() {
                    let equals_sign = equals_sign.unwrap_whitespace_and_comment();
                    assert!(matches!(equals_sign.kind, CstKind::EqualsSign { .. }));

                    let last_expression = body.last().unwrap().unwrap_whitespace_and_comment();

                    self.push(
                        equals_sign.span().end,
                        last_expression.span().end,
                        FoldingRangeKind::Region,
                    );
                }

                self.visit_cst(name);
                self.visit_csts(&parameters);
                self.visit_csts(&body);
            }
            CstKind::Error { .. } => {}
        }
    }

    fn push(&mut self, start: usize, end: usize, kind: FoldingRangeKind) {
        let start = start.utf8_byte_offset_to_lsp(self.source);
        let end = end.utf8_byte_offset_to_lsp(self.source);

        self.ranges.push(FoldingRange {
            start_line: start.line,
            start_character: Some(start.character),
            end_line: end.line,
            end_character: Some(end.character),
            kind: Some(kind),
        });
    }
}
