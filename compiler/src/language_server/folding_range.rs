use super::utils::{LspPositionConversion, TupleToPosition};
use crate::{
    compiler::{
        cst::{Cst, CstKind, UnwrapWhitespaceAndComment},
        rcst_to_cst::RcstToCst,
    },
    input::Input,
};
use lsp_types::{FoldingRange, FoldingRangeKind};

#[salsa::query_group(FoldingRangeDbStorage)]
pub trait FoldingRangeDb: LspPositionConversion + RcstToCst {
    fn folding_ranges(&self, input: Input) -> Vec<FoldingRange>;
}

fn folding_ranges(db: &dyn FoldingRangeDb, input: Input) -> Vec<FoldingRange> {
    let mut context = Context::new(db, input.clone());
    let cst = db.cst(input).unwrap();
    context.visit_csts(&cst);
    context.ranges
}

struct Context<'a> {
    db: &'a dyn FoldingRangeDb,
    input: Input,
    ranges: Vec<FoldingRange>,
}
impl<'a> Context<'a> {
    fn new(db: &'a dyn FoldingRangeDb, input: Input) -> Self {
        Context {
            db,
            input,
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
            CstKind::EqualsSign => {}
            CstKind::Comma => {}
            CstKind::Colon => {}
            CstKind::OpeningParenthesis => {}
            CstKind::ClosingParenthesis => {}
            CstKind::OpeningBracket => {}
            CstKind::ClosingBracket => {}
            CstKind::OpeningCurlyBrace => {}
            CstKind::ClosingCurlyBrace => {}
            CstKind::Arrow => {}
            CstKind::DoubleQuote => {}
            CstKind::Octothorpe => {}
            CstKind::Whitespace(_) => {}
            CstKind::Newline(_) => {}
            // TODO: support folding ranges for comments
            CstKind::Comment { .. } => {}
            CstKind::TrailingWhitespace { child, .. } => self.visit_cst(child),
            CstKind::Identifier(_) => {}
            CstKind::Symbol(_) => {}
            CstKind::Int { .. } => {}
            // TODO: support folding ranges for multiline texts
            CstKind::Text { .. } => {}
            CstKind::TextPart(_) => {}
            CstKind::Parenthesized { inner, .. } => self.visit_cst(inner),
            CstKind::Call { name, arguments } => {
                if !arguments.is_empty() {
                    let name = name.unwrap_whitespace_and_comment();
                    assert!(matches!(name.kind, CstKind::Identifier { .. }));

                    let last_argument = arguments.last().unwrap().unwrap_whitespace_and_comment();

                    self.push(
                        name.span.end,
                        last_argument.span.end,
                        FoldingRangeKind::Region,
                    );
                }

                self.visit_cst(name);
                self.visit_csts(&arguments);
            }
            // TODO: support folding ranges for structs
            CstKind::Struct { fields, .. } => self.visit_csts(fields),
            CstKind::StructField { key, value, .. } => {
                self.visit_cst(key);
                self.visit_cst(value);
            }
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
                    opening_curly_brace.span.end,
                    closing_curly_brace.span.start,
                    FoldingRangeKind::Region,
                );
                if let Some((parameters, _)) = parameters_and_arrow {
                    self.visit_csts(&parameters);
                }
                self.visit_csts(&body);
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
                        equals_sign.span.end,
                        last_expression.span.end,
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
        let start = self
            .db
            .offset_to_lsp(self.input.clone(), start)
            .to_position();
        let end = self.db.offset_to_lsp(self.input.clone(), end).to_position();

        self.ranges.push(FoldingRange {
            start_line: start.line,
            start_character: Some(start.character),
            end_line: end.line,
            end_character: Some(end.character),
            kind: Some(kind),
        });
    }
}
