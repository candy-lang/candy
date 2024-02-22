use super::utils::IdToEndOfLine;
use crate::{database::Database, utils::LspPositionConversion};
use candy_frontend::{
    ast::{Assignment, AssignmentBody, AstDb, AstKind},
    ast_to_hir::AstToHir,
    format::{MaxLength, Precedence},
    hir::{Expression, HirDb, Id},
    module::Module,
};
use candy_fuzzer::{Fuzzer, RunResult, Status};
use candy_vm::{
    heap::{InlineObject, ToDebugText},
    Panic,
};
use extension_trait::extension_trait;
use lsp_types::{Diagnostic, DiagnosticSeverity, Position, Range};
use serde::{Deserialize, Serialize};

#[derive(Debug)]
pub enum Insight {
    Diagnostic(Diagnostic),
    Hint(Hint),
}

#[derive(PartialEq, Eq, Debug, Clone, Serialize, Deserialize)]
pub struct Hint {
    pub kind: HintKind,
    pub text: String,
    pub position: Position,
}
#[derive(PartialEq, Eq, Debug, Clone, Serialize, Deserialize, PartialOrd, Ord, Copy)]
#[serde(rename_all = "camelCase")]
pub enum HintKind {
    Value,
    Panic,
    FuzzingStatus,
    SampleInputReturningNormally,
    SampleInputPanickingWithCallerResponsible,
    SampleInputPanickingWithInternalCodeResponsible,
}

impl Insight {
    pub fn for_value(db: &Database, id: Id, value: InlineObject) -> Option<Self> {
        let hir = db.find_expression(id.clone())?;
        let text = match hir {
            Expression::Reference(_) => {
                // Could be an assignment.
                let ast_id = db.hir_to_ast_id(&id)?;
                let ast = db.find_ast(ast_id)?;
                let AstKind::Assignment(Assignment { body, .. }) = &ast.kind else {
                    return None;
                };
                let creates_hint = match body {
                    AssignmentBody::Function { .. } => true,
                    AssignmentBody::Body { pattern, .. } => {
                        matches!(pattern.kind, AstKind::Identifier(_))
                    }
                };
                if !creates_hint {
                    return None;
                }

                value.to_debug_text(Precedence::Low, MaxLength::Limited(60))
            }
            Expression::PatternIdentifierReference { .. } => {
                let body = db.containing_body_of(id.clone());
                let name = body.identifiers.get(&id).unwrap();
                format!(
                    "{name} = {}",
                    value.to_debug_text(Precedence::Low, MaxLength::Limited(60)),
                )
            }
            _ => return None,
        };
        Some(Self::Hint(Hint {
            kind: HintKind::Value,
            position: db.id_to_end_of_line(id).unwrap(),
            text: if let Some(i) = text.find('\n') {
                // TODO: Show all lines when hovering the hint
                format!("{}...", &text[0..i])
            } else {
                text
            },
        }))
    }

    pub fn for_fuzzer_status(db: &Database, fuzzer: &Fuzzer) -> Vec<Self> {
        let mut insights = vec![];

        let id = fuzzer.function_id.clone();
        let end_of_line = db
            .id_to_end_of_line(id.clone())
            .unwrap_or_else(|| panic!("Can't resolve end of line for {id}"));

        let coverage = match fuzzer.status() {
            Status::StillFuzzing { total_coverage, .. } => {
                let function_range = fuzzer.byte_code().range_of_function(&id);
                let function_coverage = total_coverage.in_range(&function_range);
                function_coverage.relative_coverage()
            }
            Status::FoundPanic { .. } => 1., // TODO: not correct
        };
        let function_name = id.function_name();
        let interesting_inputs = fuzzer.input_pool().interesting_inputs();
        insights.push(Self::Hint(Hint {
            kind: HintKind::FuzzingStatus,
            position: end_of_line,
            text: format!("{:.0} % fuzzed", 100. * coverage),
        }));

        if let Status::FoundPanic { input, .. } = fuzzer.status() {
            insights.push(Self::Hint(Hint {
                kind: HintKind::SampleInputPanickingWithInternalCodeResponsible,
                position: end_of_line,
                text: format!("{function_name} {input}"),
            }));
        }

        insights.extend(interesting_inputs.into_iter().map(|input| {
            Self::Hint(match fuzzer.input_pool().result_of(&input) {
                RunResult::Timeout => unreachable!(),
                RunResult::Done { return_value, .. } => Hint {
                    kind: HintKind::SampleInputReturningNormally,
                    position: end_of_line,
                    text: format!("{function_name} {input} = {return_value}"),
                },
                RunResult::NeedsUnfulfilled { .. } => Hint {
                    kind: HintKind::SampleInputPanickingWithCallerResponsible,
                    position: end_of_line,
                    text: format!("{function_name} {input}"),
                },
                RunResult::Panicked { .. } => Hint {
                    kind: HintKind::SampleInputPanickingWithInternalCodeResponsible,
                    position: end_of_line,
                    text: format!("{function_name} {input}"),
                },
            })
        }));

        insights
    }

    pub fn for_static_panic(db: &Database, module: Module, panic: &Panic) -> Self {
        let call_span = db
            .hir_id_to_display_span(&panic.responsible)
            .unwrap_or_else(|| panic!("Can't resolve responsible ID for panic: {panic:?}"));
        let call_span = db.range_to_lsp_range(module, call_span);

        Self::Diagnostic(Diagnostic::error(
            call_span,
            ToString::to_string(&panic.reason),
        ))
    }
}

#[extension_trait]
pub impl ErrorDiagnostic for Diagnostic {
    fn error(range: Range, message: String) -> Self {
        Self {
            range,
            severity: Some(DiagnosticSeverity::ERROR),
            code: None,
            code_description: None,
            source: Some("🍭 Candy".to_owned()),
            message,
            related_information: None,
            tags: None,
            data: None,
        }
    }
}
