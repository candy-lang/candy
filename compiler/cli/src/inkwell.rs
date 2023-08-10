use std::path::PathBuf;
use std::sync::Arc;

use candy_backend_inkwell::CodeGen;
use candy_frontend::error::{CompilerError, CompilerErrorPayload};
use candy_frontend::mir::Mir;
use candy_frontend::mir_optimize::OptimizeMir;
use candy_frontend::{hir, TracingConfig};
use clap::{Parser, ValueHint};
use rustc_hash::FxHashSet;

use crate::database::Database;
use crate::utils::{module_for_path, packages_path};
use crate::{Exit, ProgramResult};

/// Compile a Candy program to a native binary.
///
/// This command compiles the given file, or, if no file is provided, the package of
/// your current working directory. The module should export a `main` function.
/// This function is then called with an environment.
#[derive(Parser, Debug)]
pub(crate) struct Options {
    /// If enabled, print the generated LLVM IR to stderr.
    #[arg(long = "print-llvm-ir", default_value_t = false)]
    print_llvm_ir: bool,

    /// If enabled, print the output of the Candy main function.
    #[arg(long = "print-main-output", default_value_t = false)]
    print_main_output: bool,

    /// If enabled, build the Candy runtime from scratch.
    #[arg(long = "build-rt", default_value_t = false)]
    build_rt: bool,

    /// If enabled, compile the LLVM bitcode with debug information.
    #[arg(short = 'g', default_value_t = false)]
    debug: bool,

    /// The file or package to run. If none is provided, run the package of your
    /// current working directory.
    #[arg(value_hint = ValueHint::FilePath)]
    path: Option<PathBuf>,
}

pub(crate) fn compile(options: Options) -> ProgramResult {
    let packages_path = packages_path();
    let db = Database::new_with_file_system_module_provider(packages_path);
    let path = options
        .path
        .as_ref()
        .map_or_else(|| "Unknown".into(), |p| p.to_string_lossy().to_string());
    let module = module_for_path(options.path)?;

    let (mir, errors) = db
        .optimized_mir(module.clone(), TracingConfig::off())
        .map(|(mir, _, errors)| (mir, errors))
        .unwrap_or_else(|error| {
            let payload = CompilerErrorPayload::Module(error);
            let mir = Mir::build(|body| {
                let reason = body.push_text(payload.to_string());
                let responsible = body.push_hir_id(hir::Id::user());
                body.push_panic(reason, responsible);
            });
            let errors =
                FxHashSet::from_iter([CompilerError::for_whole_module(module.clone(), payload)]);
            (Arc::new(mir), Arc::new(errors))
        });

    if !errors.is_empty() {
        for error in errors.iter() {
            println!("{:?}", error);
        }
        std::process::exit(1);
    }

    let context = candy_backend_inkwell::inkwell::context::Context::create();
    let mut codegen = CodeGen::new(&context, &path, mir);
    let mut bc_path = PathBuf::new();
    bc_path.push(&format!("{path}.bc"));
    codegen
        .compile(&bc_path, options.print_llvm_ir, options.print_main_output)
        .map_err(|e| Exit::LlvmError(e.to_string()))?;
    codegen
        .compile_asm_and_link(&path, options.build_rt, options.debug)
        .map_err(|_| Exit::ExternalError)?;

    ProgramResult::Ok(())
}
