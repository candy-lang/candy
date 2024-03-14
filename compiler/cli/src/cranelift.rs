use std::path::PathBuf;

use candy_backend_cranelift::CodeGen;
use candy_frontend::{module, TracingConfig};
use clap::{Parser, ValueHint};

use crate::database::Database;
use crate::utils::{module_for_path, packages_path};
use crate::ProgramResult;
use candy_frontend::hir_to_mir::ExecutionTarget;
use candy_frontend::lir_optimize::OptimizeLir;
use std::ffi::OsStr;

#[derive(Parser, Debug)]
pub(crate) struct Options {
    /// The file or package to run. If none is provided, the package of your
    /// current working directory will be run.
    #[arg(value_hint = ValueHint::FilePath)]
    path: Option<PathBuf>,
}

pub(crate) fn compile(options: Options) -> ProgramResult {
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
    let (lir, errors) = db
        .optimized_lir(ExecutionTarget::MainFunction(module), TracingConfig::off())
        .unwrap();

    if !errors.is_empty() {
        for error in errors.as_ref() {
            println!("{:?}", error);
        }
        std::process::exit(1);
    }

    let mut codegen = CodeGen::new(lir, path);
    codegen.compile().unwrap();

    ProgramResult::Ok(())
}
