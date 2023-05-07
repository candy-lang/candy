use crate::{
    database::Database,
    utils::{module_for_path, packages_path},
    Exit, ProgramResult,
};
use candy_frontend::{
    ast_to_hir::AstToHir,
    cst_to_ast::CstToAst,
    hir_to_mir::HirToMir,
    mir_optimize::OptimizeMir,
    position::Offset,
    rich_ir::{RichIr, RichIrAnnotation, TokenType},
    string_to_rcst::StringToRcst,
    TracingConfig, TracingMode,
};
use candy_vm::{lir::RichIrForLir, mir_to_lir::compile_lir};
use clap::{Parser, ValueHint};
use colored::{Color, Colorize};
use std::path::PathBuf;

/// Debug the Candy compiler itself.
///
/// This command compiles the given file and outputs its intermediate
/// representation.
#[derive(Parser, Debug)]
pub(crate) enum Options {
    /// Concrete Syntax Tree
    Cst(OnlyPath),

    /// Abstract Syntax Tree
    Ast(OnlyPath),

    /// High-Level Intermediate Representation
    Hir(OnlyPath),

    /// Mid-Level Intermediate Representation
    Mir(PathAndTracing),

    /// Optimized Mid-Level Intermediate Representation
    OptimizedMir(PathAndTracing),

    /// Low-Level Intermediate Representation
    Lir(PathAndTracing),
}
#[derive(Parser, Debug)]
pub(crate) struct OnlyPath {
    #[arg(value_hint = ValueHint::FilePath)]
    path: PathBuf,
}
#[derive(Parser, Debug)]
pub(crate) struct PathAndTracing {
    #[arg(value_hint = ValueHint::FilePath)]
    path: PathBuf,

    #[arg(long)]
    register_fuzzables: bool,

    #[arg(long)]
    trace_calls: bool,

    #[arg(long)]
    trace_evaluated_expressions: bool,
}
impl PathAndTracing {
    fn to_tracing_config(&self) -> TracingConfig {
        TracingConfig {
            register_fuzzables: TracingMode::only_current_or_off(self.register_fuzzables),
            calls: TracingMode::only_current_or_off(self.trace_calls),
            evaluated_expressions: TracingMode::only_current_or_off(
                self.trace_evaluated_expressions,
            ),
        }
    }
}

pub(crate) fn debug(options: Options) -> ProgramResult {
    let packages_path = packages_path();
    let db = Database::new_with_file_system_module_provider(packages_path);

    let rich_ir = match options {
        Options::Cst(options) => {
            let module = module_for_path(options.path)?;
            let rcst = db.rcst(module.clone());
            RichIr::for_rcst(&module, &rcst)
        }
        Options::Ast(options) => {
            let module = module_for_path(options.path)?;
            let ast = db.ast(module.clone());
            ast.ok().map(|(ast, _)| RichIr::for_ast(&module, &ast))
        }
        Options::Hir(options) => {
            let module = module_for_path(options.path)?;
            let hir = db.hir(module.clone());
            hir.ok().map(|(hir, _)| RichIr::for_hir(&module, &hir))
        }
        Options::Mir(options) => {
            let module = module_for_path(options.path.clone())?;
            let tracing = options.to_tracing_config();
            let mir = db.mir(module.clone(), tracing.clone());
            mir.ok()
                .map(|(mir, _)| RichIr::for_mir(&module, &mir, &tracing))
        }
        Options::OptimizedMir(options) => {
            let module = module_for_path(options.path.clone())?;
            let tracing = options.to_tracing_config();
            let mir = db.optimized_mir(module.clone(), tracing.clone());
            mir.ok()
                .map(|(mir, _)| RichIr::for_mir(&module, &mir, &tracing))
        }
        Options::Lir(options) => {
            let module = module_for_path(options.path.clone())?;
            let tracing = options.to_tracing_config();
            let (lir, _) = compile_lir(&db, module.clone(), tracing.clone());
            Some(RichIr::for_lir(&module, &lir, &tracing))
        }
    };

    let Some(rich_ir) = rich_ir else {
        return Err(Exit::FileNotFound);
    };

    let bytes = rich_ir.text.as_bytes().to_vec();
    let annotations = rich_ir.annotations.iter();
    let mut displayed_byte = Offset(0);

    for RichIrAnnotation {
        range, token_type, ..
    } in annotations
    {
        assert!(displayed_byte <= range.start);
        let before_annotation = std::str::from_utf8(&bytes[*displayed_byte..*range.start]).unwrap();
        print!("{before_annotation}");

        let in_annotation = std::str::from_utf8(&bytes[*range.start..*range.end]).unwrap();

        if let Some(token_type) = token_type {
            let color = match token_type {
                TokenType::Module => Color::Yellow,
                TokenType::Parameter => Color::Red,
                TokenType::Variable => Color::Yellow,
                TokenType::Symbol => Color::Magenta,
                TokenType::Function => Color::Blue,
                TokenType::Comment => Color::Green,
                TokenType::Text => Color::Cyan,
                TokenType::Int => Color::Red,
                TokenType::Address => Color::BrightGreen,
                TokenType::Constant => Color::Yellow,
            };
            print!("{}", in_annotation.color(color));
        } else {
            print!("{}", in_annotation)
        }

        displayed_byte = range.end;
    }
    let rest = std::str::from_utf8(&bytes[*displayed_byte..]).unwrap();
    println!("{rest}");

    Ok(())
}
