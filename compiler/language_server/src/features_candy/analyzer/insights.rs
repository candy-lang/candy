use super::utils::IdToEndOfLine;
use crate::{database::Database, utils::LspPositionConversion};
use candy_frontend::{
    ast::{Assignment, AssignmentBody, AstDb, AstKind},
    ast_to_hir::AstToHir,
    hir::{Expression, HirDb, Id},
    module::Module,
};
use candy_fuzzer::{Fuzzer, Status};
use candy_vm::{fiber::Panic, heap::InlineObject};
use extension_trait::extension_trait;
use itertools::Itertools;
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
        let Some(hir) = db.find_expression(id.clone()) else { return None; };
        match hir {
            Expression::Reference(_) => {
                // Could be an assignment.
                let Some(ast_id) = db.hir_to_ast_id(id.clone()) else { return None; };
                let Some(ast) = db.find_ast(ast_id) else { return None; };
                let AstKind::Assignment(Assignment { body, .. }) = &ast.kind else { return None; };
                let creates_hint = match body {
                    AssignmentBody::Function { .. } => true,
                    AssignmentBody::Body { pattern, .. } => {
                        matches!(pattern.kind, AstKind::Identifier(_))
                    }
                };
                if !creates_hint {
                    return None;
                }

                Some(Insight::Hint(Hint {
                    kind: HintKind::Value,
                    position: db.id_to_end_of_line(id).unwrap(),
                    text: value.to_string(),
                }))
            }
            Expression::PatternIdentifierReference { .. } => {
                let body = db.containing_body_of(id.clone());
                let name = body.identifiers.get(&id).unwrap();
                Some(Insight::Hint(Hint {
                    kind: HintKind::Value,
                    position: db.id_to_end_of_line(id.clone()).unwrap(),
                    text: format!("{name} = {value}"),
                }))
            }
            _ => None,
        }
    }

    pub fn for_fuzzer_status(db: &Database, fuzzer: &Fuzzer) -> Vec<Self> {
        let end_of_line = db.id_to_end_of_line(fuzzer.function_id.clone()).unwrap();

        // The fuzzer status message consists of an optional fuzzing status and
        // interesting inputs. Here are some examples:
        // - 0 % fuzzed
        // - 12 % fuzzed · True · False
        // - 100 % fuzzed · Abc · 42
        // - fuzzed · Abc · 42

        let id = fuzzer.function_id.clone();
        let front_message = match fuzzer.status() {
            Status::StillFuzzing { total_coverage, .. } => {
                let function_range = fuzzer.lir.range_of_function(&id);
                let function_coverage = total_coverage.in_range(&function_range);
                format!(
                    "{:.0} % fuzzed",
                    100. * function_coverage.relative_coverage()
                )
            }
            Status::FoundPanic { input, .. } => format!("fuzzed · {input}"),
            Status::TotalCoverageButNoPanic => "100 % fuzzed".to_string(),
        };

        let function_name = fuzzer.function_id.function_name();
        let interesting_inputs = fuzzer.pool.interesting_inputs();

        [Insight::Hint(Hint {
            kind: HintKind::FuzzingStatus,
            position: end_of_line,
            text: front_message,
        })]
        .into_iter()
        .chain(interesting_inputs.into_iter().map(|input| {
            Insight::Hint(Hint {
                kind: HintKind::SampleInputReturningNormally,
                position: end_of_line,
                text: format!("{function_name} {input}"),
            })
        }))
        .collect_vec()
    }

    pub fn for_static_panic(db: &Database, module: Module, panic: &Panic) -> Self {
        let call_span = db
            .hir_id_to_display_span(panic.responsible.clone())
            .unwrap();
        let call_span = db.range_to_lsp_range(module, call_span);

        Insight::Diagnostic(Diagnostic::error(call_span, panic.reason.to_string()))
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
