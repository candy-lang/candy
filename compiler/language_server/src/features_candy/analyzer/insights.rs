use super::{utils::IdToEndOfLine, Hint, HintKind};
use crate::{database::Database, utils::LspPositionConversion};
use candy_frontend::{
    ast::{Assignment, AssignmentBody, AstDb, AstKind},
    ast_to_hir::AstToHir,
    hir::{Expression, HirDb, Id},
    module::Module,
};
use candy_fuzzer::{Fuzzer, Status};
use candy_vm::{fiber::Panic, heap::InlineObject};
use itertools::Itertools;
use lsp_types::{Diagnostic, DiagnosticSeverity, Position, Range};

#[derive(PartialEq, Eq, Debug, Clone)]
pub enum Insight {
    Value {
        position: Position, // End of line.
        text: String,
    },
    FuzzingStatus {
        position: Position, // End of line.
        message: String,
    },
    Panic {
        call_site: Range, // The callsite that will panic.
        message: String,
    },
}

impl Insight {
    // pub fn like_comment(kind: HintKind, comment: String, end_of_line: Position) -> Self {
    //     Self {
    //         kind,
    //         text: format!("  # {}", comment.replace('\n', r#"\n"#)),
    //         position: end_of_line,
    //     }
    // }

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

                Some(Insight::Value {
                    position: db.id_to_end_of_line(id).unwrap(),
                    text: value.to_string(),
                })
            }
            Expression::PatternIdentifierReference { .. } => {
                let body = db.containing_body_of(id.clone());
                let name = body.identifiers.get(&id).unwrap();
                Some(Insight::Value {
                    position: db.id_to_end_of_line(id.clone()).unwrap(),
                    text: format!("{name} = {value}"),
                })
            }
            _ => None,
        }
    }

    pub fn for_fuzzer_status(db: &Database, fuzzer: &Fuzzer) -> Self {
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

        Insight::FuzzingStatus {
            position: end_of_line,
            message: format!(
                "{front_message}{}",
                interesting_inputs
                    .into_iter()
                    .map(|input| format!(" · `{function_name} {input}`"))
                    .join("")
            ),
        }
    }

    pub fn for_static_panic(db: &Database, module: Module, panic: &Panic) -> Self {
        let call_span = db
            .hir_id_to_display_span(panic.responsible.clone())
            .unwrap();
        let call_span = db.range_to_lsp_range(module, call_span);

        Insight::Panic {
            call_site: call_span,
            message: panic.reason.to_string(),
        }
    }
}

impl Insight {
    pub fn to_lsp_type(&self) -> LspType {
        match self {
            Insight::Value { position, text } => LspType::Hint(Hint {
                kind: HintKind::Value,
                text: format!("  # {text}"),
                position: *position,
            }),
            Insight::FuzzingStatus { position, message } => LspType::Hint(Hint {
                kind: HintKind::FuzzingStatus,
                text: format!("  # {}", message),
                position: *position,
            }),
            Insight::Panic { call_site, message } => LspType::Diagnostic(Diagnostic {
                range: *call_site,
                severity: Some(DiagnosticSeverity::ERROR),
                code: None,
                code_description: None,
                source: Some("🍭 Candy".to_owned()),
                message: message.to_string(),
                related_information: None,
                tags: None,
                data: None,
            }),
        }
    }
}
pub enum LspType {
    Diagnostic(Diagnostic),
    Hint(Hint),
}
