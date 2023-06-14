use crate::{database::Database, utils::LspPositionConversion};
use candy_frontend::{
    ast::{AssignmentBody, AstDb, AstKind},
    ast_to_hir::AstToHir,
    cst_to_ast::CstToAst,
};
use candy_fuzzer::{Fuzzer, Status};
use lsp_types::{Diagnostic, DiagnosticSeverity, Position, Range};
use serde::{Deserialize, Serialize};

#[derive(PartialEq, Eq, Debug, Clone, Serialize, Deserialize)]
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
        callsite: Range, // The callsite that will panic.
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

    pub fn for_fuzzer_status(db: &Database, fuzzer: &Fuzzer) -> Option<Self> {
        let Some(ast_id) = db.hir_to_ast_id(fuzzer.function_id.clone()) else { return None; };
        let Some(ast) = db.find_ast(ast_id) else { return None; };
        let AstKind::Assignment(assignment) = ast.kind else { return None; };
        let AssignmentBody::Function { name, .. } = assignment.body else { return None; };
        let Some(range) = db.ast_id_to_display_span(name.id) else { return None; };
        let range = db.range_to_lsp_range(fuzzer.function_id.module, range);

        let id = fuzzer.function_id.clone();
        let message = match fuzzer.status() {
            Status::StillFuzzing { total_coverage, .. } => {
                let function_range = fuzzer.lir.range_of_function(&id);
                let function_coverage = total_coverage.in_range(&function_range);
                format!("Fuzzing… – {} %", function_coverage.relative_coverage())
            }
            Status::FoundPanic { input, .. } => format!("{input}"),
            Status::TotalCoverageButNoPanic => format!("Fuzzed completely."),
        };
        Some(Insight::FuzzingStatus {
            position: range.end,
            message,
        })
    }
}

impl Insight {
    fn to_diagnostic(&self, db: &Database) -> Option<Diagnostic> {
        match self {
            Insight::Value { position, text } => None,
            Insight::FuzzingStatus { position, message } => None,
            Insight::Panic { callsite, message } => Some(Diagnostic {
                range: callsite.clone(),
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
