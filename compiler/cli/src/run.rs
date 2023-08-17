use crate::{
    database::Database,
    utils::{module_for_path, packages_path},
    Exit, ProgramResult,
};
use candy_frontend::{ast_to_hir::AstToHir, hir, TracingConfig};
use candy_language_server::utils::LspPositionConversion;
use candy_vm::{
    environment::{DefaultEnvironment, Environment},
    heap::{Heap, HirId},
    mir_to_lir::compile_lir,
    tracer::stack_trace::StackTracer,
    Vm, VmFinished,
};
use clap::{Parser, ValueHint};
use std::{
    path::PathBuf,
    rc::Rc,
    time::{Duration, Instant},
};
use tracing::{debug, error};

/// Run a Candy program.
///
/// This command runs the given file, or, if no file is provided, the package of
/// your current working directory. The module should export a `main` function.
/// This function is then called with an environment.
#[derive(Parser, Debug)]
pub(crate) struct Options {
    /// The file or package to run. If none is provided, the package of your
    /// current working directory will be run.
    #[arg(value_hint = ValueHint::FilePath)]
    path: Option<PathBuf>,
}

pub(crate) fn run(options: Options) -> ProgramResult {
    let packages_path = packages_path();
    let db = Database::new_with_file_system_module_provider(packages_path.clone());
    let module = module_for_path(options.path)?;

    let tracing = TracingConfig::off();

    debug!("Running {module}.");

    let compilation_start = Instant::now();
    let lir = Rc::new(compile_lir(&db, module.clone(), tracing).0);

    let compilation_end = Instant::now();
    debug!(
        "Compilation took {}.",
        format_duration(compilation_end - compilation_start),
    );

    let mut heap = Heap::default();
    let VmFinished { tracer, result } = Vm::for_module(&*lir, &mut heap, StackTracer::default())
        .run_forever_without_handles(&mut heap);
    let exports = match result {
        Ok(exports) => exports,
        Err(panic) => {
            error!("The module panicked: {}", panic.reason);
            error!("{} is responsible.", panic.responsible);
            if let Some(span) = db.hir_id_to_span(&panic.responsible) {
                let current_package_path = module.package.to_path(&packages_path).unwrap();
                let file = panic
                    .responsible
                    .module
                    .try_to_path(&packages_path)
                    .and_then(|it| {
                        it.strip_prefix(current_package_path)
                            .unwrap_or(&it)
                            .to_str()
                            .map(|it| it.to_string())
                    })
                    .unwrap_or_else(|| panic.responsible.module.to_string());
                let range = db.range_to_lsp_range(panic.responsible.module.clone(), span);
                error!(
                    "{file}:{}:{} – {}:{}",
                    range.start.line, range.start.character, range.end.line, range.end.character,
                );
            }
            error!("This is the stack trace:\n{}", tracer.format(&db));
            return Err(Exit::CodePanicked);
        }
    };
    let main = match exports.into_main_function(&heap) {
        Ok(main) => main,
        Err(error) => {
            error!("{error}");
            return Err(Exit::NoMainFunction);
        }
    };
    let discovery_end = Instant::now();
    debug!(
        "main function discovery took {}.",
        format_duration(discovery_end - compilation_end),
    );

    debug!("Running main function.");
    let (environment_object, mut environment) = DefaultEnvironment::new(&mut heap);
    let platform = HirId::create(&mut heap, true, hir::Id::platform());
    let vm = Vm::for_function(
        lir.clone(),
        &mut heap,
        main,
        &[environment_object],
        platform,
        StackTracer::default(),
    );
    let VmFinished { result, .. } = vm.run_forever_with_environment(&mut heap, &mut environment);
    let result = match result {
        Ok(return_value) => {
            debug!("The main function returned: {return_value:?}");
            Ok(())
        }
        Err(panic) => {
            error!("The main function panicked: {}", panic.reason);
            error!("{} is responsible.", panic.responsible);
            error!("This is the stack trace:\n{}", tracer.format(&db));
            Err(Exit::CodePanicked)
        }
    };
    let execution_end = Instant::now();
    debug!(
        "Execution took {}.",
        format_duration(execution_end - discovery_end),
    );

    drop(lir); // Make sure the LIR is kept around until here.
    result
}

fn format_duration(duration: Duration) -> String {
    if duration < Duration::from_millis(1) {
        format!("{} µs", duration.as_micros())
    } else {
        format!("{} ms", duration.as_millis())
    }
}
