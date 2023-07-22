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
    rcst_to_cst::RcstToCst,
    rich_ir::{RichIr, RichIrAnnotation, TokenType},
    string_to_rcst::StringToRcst,
    TracingConfig, TracingMode,
};
use candy_vm::{lir::RichIrForLir, mir_to_lir::compile_lir};
use clap::{Parser, ValueHint};
use colored::{Color, Colorize};
use diffy::{create_patch, PatchFormatter};
use itertools::Itertools;
use regex::Regex;
use std::{
    env, fs, io,
    path::{Path, PathBuf},
};
use walkdir::WalkDir;

/// Debug the Candy compiler itself.
///
/// This command compiles the given file and outputs its intermediate
/// representation.
#[derive(Parser, Debug)]
pub(crate) enum Options {
    /// Raw Concrete Syntax Tree
    Rcst(OnlyPath),

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

    #[command(subcommand)]
    Gold(GoldOptions),
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
        Options::Rcst(options) => {
            let module = module_for_path(options.path)?;
            let rcst = db.rcst(module.clone());
            RichIr::for_rcst(&module, &rcst)
        }
        Options::Cst(options) => {
            let module = module_for_path(options.path)?;
            let cst = db.cst(module.clone());
            RichIr::for_cst(&module, &cst)
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
                .map(|(mir, _, _)| RichIr::for_mir(&module, &mir, &tracing))
        }
        Options::Lir(options) => {
            let module = module_for_path(options.path.clone())?;
            let tracing = options.to_tracing_config();
            let (lir, _) = compile_lir(&db, module.clone(), tracing.clone());
            Some(RichIr::for_lir(&module, &lir, &tracing))
        }
        Options::Gold(options) => return options.run(&db),
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

/// Dump IRs next to the original files to compare outputs of different compiler
/// versions.
#[derive(Parser, Debug)]
pub(crate) enum GoldOptions {
    /// For each Candy file, generate the IRs next to the file.
    Generate(GoldPath),

    /// For each Candy file, check if the IRs next to the file are up-to-date.
    Check(GoldPath),
}
#[derive(Parser, Debug)]
pub(crate) struct GoldPath {
    #[arg(value_hint = ValueHint::DirPath)]
    directory: Option<PathBuf>,
}
impl GoldOptions {
    fn run(&self, db: &Database) -> ProgramResult {
        match &self {
            GoldOptions::Generate(options) => options
                .visit_irs(db, |_file, _ir_namee, ir_file, ir| {
                    fs::write(ir_file, ir).unwrap()
                }),
            GoldOptions::Check(options) => {
                let mut did_change = false;
                let formatter = PatchFormatter::new().with_color();
                options.visit_irs(db, |file, ir_name, ir_file, ir| {
                    let old_ir = match fs::read_to_string(ir_file) {
                        Ok(old_ir) => old_ir,
                        Err(error) if error.kind() == io::ErrorKind::NotFound => {
                            println!("{ir_name} of {} doesn't exist yet", file.display());
                            did_change = true;
                            return;
                        }
                        Err(err) => panic!("{err}"),
                    };

                    let patch = create_patch(&old_ir, &ir);
                    if !patch.hunks().is_empty() {
                        did_change = true;
                        println!("{ir_name} of {} changed:", file.display());
                        // The first two lines contain “--- original” and
                        // “+++ modified”, which we don't want to print.
                        println!(
                            "{}",
                            formatter
                                .fmt_patch(&patch)
                                .to_string()
                                .lines()
                                .skip(2)
                                .join("\n"),
                        );
                        println!();
                    }
                })?;
                if did_change {
                    println!("❌ Some goldens are outdated");
                    Err(Exit::GoldOutdated)
                } else {
                    println!("✅ All goldens are up-to-date");
                    Ok(())
                }
            }
        }
    }
}
impl GoldPath {
    fn visit_irs(
        &self,
        db: &Database,
        mut visitor: impl FnMut(&Path, &str, &Path, String),
    ) -> ProgramResult {
        let directory = self
            .directory
            .to_owned()
            .unwrap_or_else(|| env::current_dir().unwrap());
        if !directory.is_dir() {
            print!("{} is not a directory", directory.display());
            return Err(Exit::DirectoryNotFound);
        }

        let output_directory = directory.join(".goldens");
        fs::create_dir_all(&output_directory).unwrap();

        let tracing_config = TracingConfig::off();
        let address_regex = Regex::new(r"0x[0-9a-f]{1,16}").unwrap();
        for file in WalkDir::new(&directory)
            .into_iter()
            .map(|it| it.unwrap())
            .filter(|it| it.file_type().is_file())
            .filter(|it| it.file_name().to_string_lossy().ends_with(".candy"))
        {
            let path = file.path();
            let module = module_for_path(path.to_owned())?;
            let directory = output_directory.join(path.strip_prefix(&directory).unwrap());
            fs::create_dir_all(&directory).unwrap();

            let mut visit = |ir_name: &str, ir: String| {
                let ir_file = directory.join(format!("{ir_name}.txt"));
                visitor(path, ir_name, &ir_file, ir)
            };

            let rcst = db.rcst(module.clone());
            let rcst = RichIr::for_rcst(&module, &rcst).unwrap();
            visit("RCST", rcst.text);

            let cst = db.cst(module.clone());
            let cst = RichIr::for_cst(&module, &cst).unwrap();
            visit("CST", cst.text);

            let (ast, _) = db.ast(module.clone()).unwrap();
            let ast = RichIr::for_ast(&module, &ast);
            visit("AST", ast.text);

            let (hir, _) = db.hir(module.clone()).unwrap();
            let hir = RichIr::for_hir(&module, &hir);
            visit("HIR", hir.text);

            let (mir, _) = db.mir(module.clone(), tracing_config.clone()).unwrap();
            let mir = RichIr::for_mir(&module, &mir, &tracing_config);
            visit("MIR", mir.text);

            let (optimized_mir, _, _) = db
                .optimized_mir(module.clone(), tracing_config.clone())
                .unwrap();
            let optimized_mir = RichIr::for_optimized_mir(&module, &optimized_mir, &tracing_config);
            visit("Optimized MIR", optimized_mir.text);

            // LIR
            let (lir, _) = compile_lir(db, module.clone(), tracing_config.clone());
            let lir = RichIr::for_lir(&module, &lir, &tracing_config);

            // Remove addresses of constants them from the output since they are
            // random.
            let lir = address_regex.replace_all(&lir.text, "<removed address>");

            // Sort the constant heap alphabetically to make the output more
            // stable.
            let mut lines = lir.lines().collect_vec();
            let (constants_start, _) = lines
                .iter()
                .find_position(|&&it| it == "# Constant heap")
                .unwrap();
            let (constants_end, _) = lines
                .iter()
                .find_position(|&&it| it == "# Instructions")
                .unwrap();
            lines[constants_start + 1..constants_end - 1].sort();
            // Re-add the trailing newline
            lines.push("");

            visit("LIR", lines.iter().join("\n"));
        }
        Ok(())
    }
}
