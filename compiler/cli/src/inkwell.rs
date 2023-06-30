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
use crate::ProgramResult;

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
            let errors = vec![CompilerError::for_whole_module(module.clone(), payload)]
                .into_iter()
                .collect();
            (Arc::new(mir), Arc::new(errors))
        });

    let context = backend_inkwell::inkwell::context::Context::create();
    let module = context.create_module(&path);
    let builder = context.create_builder();
    let mut codegen = CodeGen::new(&context, module, builder, mir);
    codegen.compile();

    ProgramResult::Ok(())
}
