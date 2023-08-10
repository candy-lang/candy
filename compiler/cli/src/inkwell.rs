use std::path::PathBuf;
use std::sync::Arc;

use backend_inkwell::CodeGen;
use candy_frontend::error::{CompilerError, CompilerErrorPayload};
use candy_frontend::mir::Mir;
use candy_frontend::mir_optimize::OptimizeMir;
use candy_frontend::{hir, TracingConfig};
use clap::{Parser, ValueHint};

use crate::database::Database;
use crate::utils::{module_for_path, packages_path};
use crate::{Exit, ProgramResult};

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

    let context = backend_inkwell::inkwell::context::Context::create();
    let module = context.create_module(&path);
    let builder = context.create_builder();
    let codegen = CodeGen::new(&context, module, builder, mir);
    let bc_path = PathBuf::from(format!("{path}.bc"));
    codegen
        .compile(&bc_path, options.print_llvm_ir, options.print_main_output)
        .map_err(|e| Exit::LLVMError(e.to_string()))?;
    std::process::Command::new("llc")
        .arg(&bc_path)
        .args(["-O3"])
        .spawn()
        .unwrap()
        .wait()
        .unwrap();
    if options.build_rt {
        std::process::Command::new("make")
            .args(["-C", "compiler/backend_inkwell/candy_rt/", "clean"])
            .spawn()
            .unwrap()
            .wait()
            .unwrap();

        std::process::Command::new("make")
            .args(["-C", "compiler/backend_inkwell/candy_rt/", "candy_rt.a"])
            .spawn()
            .unwrap()
            .wait()
            .unwrap();
    }
    let s_path = PathBuf::from(format!("{path}.s"));
    std::process::Command::new("clang")
        .args([
            s_path.to_str().unwrap(),
            "compiler/backend_inkwell/candy_rt/candy_rt.a",
            if options.debug { "-g" } else { "" },
            "-O3",
            "-flto",
            "-o",
            &s_path.to_str().unwrap().replace(".candy.s", ""),
        ])
        .spawn()
        .unwrap()
        .wait()
        .unwrap();

    ProgramResult::Ok(())
}
