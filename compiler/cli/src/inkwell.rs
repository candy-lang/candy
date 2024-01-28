use crate::{
    database::Database,
    utils::{module_for_path, packages_path},
    Exit, ProgramResult,
};
use candy_backend_inkwell::CodeGen;
use candy_frontend::{
    error::{CompilerError, CompilerErrorPayload},
    hir,
    hir_to_mir::ExecutionTarget,
    mir::Mir,
    mir_optimize::OptimizeMir,
    module, TracingConfig,
};
use clap::{Parser, ValueHint};
use rustc_hash::FxHashSet;
use std::{ffi::OsStr, path::PathBuf, sync::Arc};
use tracing::error;

/// Compile a Candy program to a native binary.
///
/// This command compiles the given file, or, if no file is provided, the package of
/// your current working directory. The module should export a `main` function.
/// This function is then called with an environment.
#[allow(clippy::struct_excessive_bools)]
#[derive(Parser, Debug)]
pub struct Options {
    /// If enabled, print the generated LLVM IR to stderr.
    #[arg(long = "print-llvm-ir", default_value_t = false)]
    print_llvm_ir: bool,

    /// If enabled, print the output of the Candy main function.
    #[arg(long = "print-main-output", default_value_t = false)]
    print_main_output: bool,

    /// If enabled, build the Candy runtime from scratch.
    #[arg(long = "build-runtime", default_value_t = false)]
    build_runtime: bool,

    /// If enabled, compile the LLVM bitcode with debug information.
    #[arg(short = 'g', default_value_t = false)]
    debug: bool,

    /// The linker to be used. Defaults to `ld.lld`
    #[arg(long, default_value = "ld.lld")]
    linker: String,

    /// The file or package to compile. If none is provided, compile the package
    /// of your current working directory.
    #[arg(value_hint = ValueHint::FilePath)]
    path: Option<PathBuf>,
}

pub fn compile(options: &Options) -> ProgramResult {
    let packages_path = packages_path();
    let db = Database::new_with_file_system_module_provider(packages_path);
    let module = module_for_path(options.path.clone())?;
    let path = options
        .path
        .as_ref()
        .unwrap_or_else(|| match &module.package {
            module::Package::User(user) => user,
            module::Package::Managed(managed) => managed,
            _ => unreachable!(),
        })
        .file_name()
        .unwrap_or_else(|| OsStr::new("Executable"))
        .to_string_lossy()
        .to_string();

    #[allow(clippy::map_unwrap_or)]
    let (mir, errors) = db
        .optimized_mir(
            ExecutionTarget::MainFunction(module.clone()),
            TracingConfig::off(),
        )
        .map(|(mir, errors)| (mir, errors))
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
        for error in errors.as_ref() {
            println!("{:?}", error);
        }
        std::process::exit(1);
    }

    let context = candy_backend_inkwell::inkwell::context::Context::create();
    let codegen = CodeGen::new(&context, &path, mir);
    let llvm_candy_module = codegen
        .compile(options.print_llvm_ir, options.print_main_output)
        .map_err(|e| Exit::LlvmError(e.to_string()))?;
    llvm_candy_module
        .compile_obj_and_link(&path, options.build_runtime, options.debug, &options.linker)
        .map_err(|err| {
            error!("Failed to compile and link executable: {}", err);
            Exit::ExternalError
        })?;

    ProgramResult::Ok(())
}
