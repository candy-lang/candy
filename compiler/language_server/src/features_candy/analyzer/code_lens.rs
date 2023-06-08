use crate::{database::Database, utils::LspPositionConversion};
use candy_frontend::{
    ast::{AssignmentBody, AstDb, AstKind},
    ast_to_hir::AstToHir,
    cst_to_ast::CstToAst,
    hir::{Body, Expression, Id},
    module::Module,
};
use extension_trait::extension_trait;
use itertools::Itertools;
use lsp_types::Command;
use rustc_hash::FxHashMap;

pub enum CodeLens {
    NotFuzzed,
    Fuzzing {
        coverage: f64,
        inputs: Vec<String>,
    },
    FuzzedCompletely,
    FoundPanic {
        panicking_input: String,
        other_inputs: Vec<String>,
    },
}

impl CodeLens {
    pub fn to_lsp_code_lenses(&self, db: &Database, function: Id) -> Vec<lsp_types::CodeLens> {
        let Some(ast_id) = db.hir_to_ast_id(function.clone()) else { return vec![]; };
        let Some(ast) = db.find_ast(ast_id) else { return vec![]; };
        let AstKind::Assignment(assignment) = ast.kind else { return vec![]; };
        let AssignmentBody::Function { name, .. } = assignment.body else { return vec![]; };
        let Some(range) = db.ast_id_to_display_span(name.id) else { return vec![]; };
        let range = db.range_to_lsp_range(function.module, range);

        let mut commands = vec![];
        match self {
            CodeLens::NotFuzzed => commands.push(Command {
                title: "Fuzzing: Not started yet".to_string(),
                command: "fix world hunger".to_string(),
                arguments: None,
            }),
            CodeLens::Fuzzing { coverage, inputs } => {
                commands.push(Command {
                    title: format!("Fuzzing: {} %", coverage * 100.0),
                    command: "show coverage".to_string(),
                    arguments: None,
                });
                for input in inputs {
                    commands.push(Command {
                        title: input.to_string(),
                        command: "run input".to_string(),
                        arguments: None,
                    });
                }
            }
            CodeLens::FuzzedCompletely => {
                commands.push(Command {
                    title: "Fuzzing: Done".to_string(),
                    command: "done fuzzing".to_string(),
                    arguments: None,
                });
            }
            CodeLens::FoundPanic {
                panicking_input,
                other_inputs,
            } => {
                commands.push(Command {
                    title: format!("Fuzzing: Panicked for {panicking_input}"),
                    command: "show panic".to_string(),
                    arguments: None,
                });
                for input in other_inputs {
                    commands.push(Command {
                        title: input.to_string(),
                        command: "run input".to_string(),
                        arguments: None,
                    });
                }
            }
        };

        commands
            .into_iter()
            .map(|command| lsp_types::CodeLens {
                range,
                command: Some(command),
                data: None,
            })
            .collect_vec()
    }
}

pub fn default_code_lenses(db: &Database, module: Module) -> FxHashMap<Id, CodeLens> {
    let (hir, _) = db.hir(module).unwrap();
    let mut lenses = FxHashMap::default();
    hir.collect_lenses(db, &mut lenses);
    lenses
}

#[extension_trait]
impl LensesFromExpression for Expression {
    fn collect_lenses(&self, id: Id, db: &Database, lenses: &mut FxHashMap<Id, CodeLens>) {
        let Expression::Function(function) = self else { return; };
        if !function.fuzzable {
            return;
        }
        lenses.insert(id, CodeLens::NotFuzzed);
        function.body.collect_lenses(db, lenses);
    }
}

#[extension_trait]
impl LensesFromBody for Body {
    fn collect_lenses(&self, db: &Database, lenses: &mut FxHashMap<Id, CodeLens>) {
        for (id, expression) in &self.expressions {
            expression.collect_lenses(id.clone(), db, lenses);
        }
    }
}
