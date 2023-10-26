use crate::{
    database::Database,
    utils::{module_for_path, packages_path},
    Exit, ProgramResult,
};
#[cfg(feature = "inkwell")]
use candy_backend_inkwell::LlvmIrDb;
use candy_frontend::{
    ast_to_hir::AstToHir,
    cst_to_ast::CstToAst,
    hir_to_mir::{ExecutionTarget, HirToMir},
    lir_optimize::OptimizeLir,
    mir_optimize::OptimizeMir,
    mir_to_lir::MirToLir,
    module::Module,
    position::Offset,
    rcst_to_cst::RcstToCst,
    rich_ir::{RichIr, RichIrAnnotation, TokenType},
    string_to_rcst::StringToRcst,
    utils::DoHash,
    TracingConfig, TracingMode,
};
use candy_vm::{byte_code::RichIrForByteCode, heap::HeapData, mir_to_byte_code::compile_byte_code};
use clap::{Parser, ValueEnum, ValueHint};
use colored::{Color, Colorize};
use diffy::{create_patch, PatchFormatter};
use itertools::Itertools;
use lazy_static::lazy_static;
use regex::{Captures, Regex, RegexBuilder};
use rustc_hash::FxHashMap;
use std::{
    env, fs, io,
    path::{Path, PathBuf},
    str,
};
use walkdir::WalkDir;

/// Debug the Candy compiler itself.
///
/// This command compiles the given file and outputs its intermediate
/// representation.
#[derive(Parser, Debug)]
pub enum Options {
    /// Raw Concrete Syntax Tree
    Rcst(OnlyPath),

    /// Concrete Syntax Tree
    Cst(OnlyPath),

    /// Abstract Syntax Tree
    Ast(OnlyPath),

    /// High-Level Intermediate Representation
    Hir(OnlyPath),

    /// Mid-Level Intermediate Representation
    Mir(PathAndExecutionTargetAndTracing),

    /// Optimized Mid-Level Intermediate Representation
    OptimizedMir(PathAndExecutionTargetAndTracing),

    /// Low-Level Intermediate Representation
    Lir(PathAndExecutionTargetAndTracing),

    /// Optimized Low-Level Intermediate Representation
    OptimizedLir(PathAndExecutionTargetAndTracing),

    /// VM Byte Code
    VmByteCode(PathAndExecutionTargetAndTracing),

    /// LLVM Intermediate Representation
    #[cfg(feature = "inkwell")]
    LlvmIr(PathAndExecutionTarget),

    #[command(subcommand)]
    Gold(Gold),
}

#[derive(Parser, Debug)]
pub struct OnlyPath {
    #[arg(value_hint = ValueHint::FilePath)]
    path: PathBuf,
}

#[derive(Parser, Debug)]
pub struct PathAndExecutionTargetAndTracing {
    #[arg(value_hint = ValueHint::FilePath)]
    path: PathBuf,

    #[arg(long, value_enum, default_value_t = ExecutionTargetKind::Module)]
    execution_target: ExecutionTargetKind,

    #[arg(long)]
    register_fuzzables: bool,

    #[arg(long)]
    trace_calls: bool,

    #[arg(long)]
    trace_evaluated_expressions: bool,
}
impl PathAndExecutionTargetAndTracing {
    #[must_use]
    const fn to_tracing_config(&self) -> TracingConfig {
        TracingConfig {
            register_fuzzables: TracingMode::only_current_or_off(self.register_fuzzables),
            calls: TracingMode::only_current_or_off(self.trace_calls),
            evaluated_expressions: TracingMode::only_current_or_off(
                self.trace_evaluated_expressions,
            ),
        }
    }
}

#[derive(Parser, Debug)]
pub struct PathAndExecutionTarget {
    #[arg(value_hint = ValueHint::FilePath)]
    path: PathBuf,

    #[arg(long, value_enum, default_value_t = ExecutionTargetKind::Module)]
    execution_target: ExecutionTargetKind,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, ValueEnum)]
pub enum ExecutionTargetKind {
    Module,
    MainFunction,
}
impl ExecutionTargetKind {
    const fn resolve(self, module: Module) -> ExecutionTarget {
        match self {
            Self::Module => ExecutionTarget::Module(module),
            Self::MainFunction => ExecutionTarget::MainFunction(module),
        }
    }
}

pub fn debug(options: Options) -> ProgramResult {
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
            let execution_target = options.execution_target.resolve(module.clone());
            let tracing = options.to_tracing_config();
            let mir = db.mir(execution_target, tracing.clone());
            mir.ok()
                .map(|(mir, _)| RichIr::for_mir(&module, &mir, &tracing))
        }
        Options::OptimizedMir(options) => {
            let module = module_for_path(options.path.clone())?;
            let execution_target = options.execution_target.resolve(module.clone());
            let tracing = options.to_tracing_config();
            let mir = db.optimized_mir(execution_target, tracing.clone());
            mir.ok()
                .map(|(mir, _, _)| RichIr::for_optimized_mir(&module, &mir, &tracing))
        }
        Options::Lir(options) => {
            let module = module_for_path(options.path.clone())?;
            let execution_target = options.execution_target.resolve(module.clone());
            let tracing = options.to_tracing_config();
            let lir = db.lir(execution_target, tracing.clone());
            lir.ok()
                .map(|(lir, _)| RichIr::for_lir(&module, &lir, &tracing))
        }
        Options::OptimizedLir(options) => {
            let module = module_for_path(options.path.clone())?;
            let execution_target = options.execution_target.resolve(module.clone());
            let tracing = options.to_tracing_config();
            let lir = db.optimized_lir(execution_target, tracing.clone());
            lir.ok()
                .map(|(lir, _)| RichIr::for_lir(&module, &lir, &tracing))
        }
        Options::VmByteCode(options) => {
            let module = module_for_path(options.path.clone())?;
            let execution_target = options.execution_target.resolve(module.clone());
            let tracing = options.to_tracing_config();
            let (vm_byte_code, _) = compile_byte_code(&db, execution_target, tracing.clone());
            Some(RichIr::for_byte_code(&module, &vm_byte_code, &tracing))
        }
        #[cfg(feature = "inkwell")]
        Options::LlvmIr(options) => {
            let module = module_for_path(options.path.clone())?;
            let execution_target = options.execution_target.resolve(module);
            db.llvm_ir(execution_target).ok()
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
        let before_annotation = str::from_utf8(&bytes[*displayed_byte..*range.start]).unwrap();
        print!("{before_annotation}");

        let in_annotation = str::from_utf8(&bytes[*range.start..*range.end]).unwrap();

        #[allow(clippy::option_if_let_else)]
        if let Some(token_type) = token_type {
            let color = match token_type {
                TokenType::Module => Color::BrightYellow,
                TokenType::Parameter => Color::Red,
                TokenType::Variable => Color::Yellow,
                TokenType::Symbol => Color::Magenta,
                TokenType::Function => Color::Blue,
                TokenType::Comment => Color::Green,
                TokenType::Text => Color::Cyan,
                TokenType::Int => Color::Red,
                TokenType::Address => Color::BrightGreen,
                TokenType::Constant => Color::BrightYellow,
            };
            print!("{}", in_annotation.color(color));
        } else {
            print!("{in_annotation}");
        }

        displayed_byte = range.end;
    }
    let rest = str::from_utf8(&bytes[*displayed_byte..]).unwrap();
    println!("{rest}");

    Ok(())
}

/// Dump IRs next to the original files to compare outputs of different compiler
/// versions.
#[derive(Parser, Debug)]
pub enum Gold {
    /// For each Candy file, generate the IRs next to the file.
    Generate(GoldOptions),

    /// For each Candy file, check if the IRs next to the file are up-to-date.
    Check(GoldOptions),
}
#[derive(Parser, Debug)]
pub struct GoldOptions {
    #[arg(value_hint = ValueHint::DirPath)]
    directory: Option<PathBuf>,

    #[arg(long, value_enum, default_value_t = ExecutionTargetKind::MainFunction)]
    execution_target: ExecutionTargetKind,

    #[arg(long, value_hint = ValueHint::DirPath)]
    output_directory: Option<PathBuf>,
}
impl Gold {
    fn run(&self, db: &Database) -> ProgramResult {
        match &self {
            Self::Generate(options) => options.visit_irs(db, |_file, _ir_name, ir_file, ir| {
                fs::write(ir_file, ir).unwrap();
            }),
            Self::Check(options) => {
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
impl GoldOptions {
    const TRACING_CONFIG: TracingConfig = TracingConfig::off();
    fn visit_irs(
        &self,
        db: &Database,
        mut visitor: impl FnMut(&Path, &str, &Path, String),
    ) -> ProgramResult {
        let directory = self
            .directory
            .clone()
            .unwrap_or_else(|| env::current_dir().unwrap());
        if !directory.is_dir() {
            print!("{} is not a directory", directory.display());
            return Err(Exit::DirectoryNotFound);
        }

        let output_directory = self
            .output_directory
            .clone()
            .unwrap_or_else(|| directory.join(".goldens"));
        fs::create_dir_all(&output_directory).unwrap();

        for file in WalkDir::new(&directory)
            .into_iter()
            .map(Result::unwrap)
            .filter(|it| it.file_type().is_file())
            .filter(|it| it.file_name().to_string_lossy().ends_with(".candy"))
        {
            let path = file.path();
            let module = module_for_path(path.to_owned())?;
            let execution_target = self.execution_target.resolve(module.clone());
            let directory = output_directory.join(path.strip_prefix(&directory).unwrap());
            fs::create_dir_all(&directory).unwrap();

            let mut visit = |ir_name: &str, ir: String| {
                let ir_file = directory.join(format!("{ir_name}.txt"));
                visitor(path, ir_name, &ir_file, ir);
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

            let (mir, _) = db
                .mir(execution_target.clone(), Self::TRACING_CONFIG.clone())
                .unwrap();
            let mir = RichIr::for_mir(&module, &mir, &Self::TRACING_CONFIG);
            visit("MIR", mir.text);

            let (optimized_mir, _, _) = db
                .optimized_mir(execution_target.clone(), Self::TRACING_CONFIG.clone())
                .unwrap();
            let optimized_mir =
                RichIr::for_optimized_mir(&module, &optimized_mir, &Self::TRACING_CONFIG);
            visit("Optimized MIR", optimized_mir.text);

            let (lir, _) = db
                .lir(execution_target.clone(), Self::TRACING_CONFIG.clone())
                .unwrap();
            let lir = RichIr::for_lir(&module, &lir, &Self::TRACING_CONFIG);
            visit("LIR", lir.text);

            let (optimized_lir, _) = db
                .optimized_lir(execution_target.clone(), Self::TRACING_CONFIG.clone())
                .unwrap();
            let optimized_lir = RichIr::for_lir(&module, &optimized_lir, &Self::TRACING_CONFIG);
            visit("Optimized LIR", optimized_lir.text);

            let (vm_byte_code, _) =
                compile_byte_code(db, execution_target.clone(), Self::TRACING_CONFIG.clone());
            let vm_byte_code_rich_ir =
                RichIr::for_byte_code(&module, &vm_byte_code, &Self::TRACING_CONFIG);
            visit(
                "VM Byte Code",
                Self::format_byte_code(&vm_byte_code, &vm_byte_code_rich_ir),
            );

            #[cfg(feature = "inkwell")]
            {
                let llvm_ir = db.llvm_ir(execution_target).unwrap();
                visit("LLVM IR", llvm_ir.text);
            }
        }
        Ok(())
    }

    fn format_byte_code(byte_code: &candy_vm::byte_code::ByteCode, rich_ir: &RichIr) -> String {
        let address_replacements: FxHashMap<_, _> = byte_code
            .constant_heap
            .iter()
            .map(|constant| {
                (
                    format!("{:p}", constant.address()),
                    format!(
                        "<replaced address {:016x}>",
                        HeapData::from(constant).do_hash(),
                    ),
                )
            })
            .collect();

        // Replace addresses of constants with content hashes since addresses
        // are random.
        let byte_code = ADDRESS_REGEX.replace_all(&rich_ir.text, |captures: &Captures| {
            let full_match = captures.get(0).unwrap();
            let full_match_str = full_match.as_str();
            let address = captures.iter().skip(1).find_map(|it| it).unwrap();
            format!(
                "{}{}{}",
                &full_match_str[..address.start() - full_match.start()],
                address_replacements[address.as_str()],
                &full_match_str[address.end() - full_match.start()..],
            )
        });

        // Sort the constant heap alphabetically to make the output more
        // stable.
        let mut lines = byte_code.lines().collect_vec();
        let (constants_start, _) = lines
            .iter()
            .find_position(|&&it| it == "# Constant heap")
            .unwrap();
        let (constants_end, _) = lines
            .iter()
            .find_position(|&&it| it == "# Instructions")
            .unwrap();
        lines[constants_start + 1..constants_end - 1].sort_unstable();
        // Re-add the trailing newline
        lines.push("");

        lines.iter().join("\n")
    }
}

lazy_static! {
    static ref ADDRESS_REGEX: Regex = {
        const ADDRESS: &str = "0x[0-9a-f]{1,16}";
        // Addresses of constants in the constant heap.
        let constant_heap = format!(r"^({ADDRESS}): ");
        // Addresses of constants in pushConstant instructions.
        let push_constant = format!(r"^ *\d+: pushConstant ({ADDRESS}) ");
        RegexBuilder::new(&format!("{constant_heap}|{push_constant}"))
            .multi_line(true)
            .build()
            .unwrap()
    };
}
