use crate::{
    database::Database,
    utils::{module_for_path, packages_path},
    Exit, ProgramResult,
};
use candy_frontend::{hir, TracingConfig, TracingMode};
use candy_vm::{
    environment::DefaultEnvironment,
    heap::{Heap, HirId},
    mir_to_byte_code::compile_byte_code,
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

    #[arg(last(true))]
    arguments: Vec<String>,
}

pub(crate) fn run(options: Options) -> ProgramResult {
    let packages_path = packages_path();
    let db = Database::new_with_file_system_module_provider(packages_path.clone());
    let module = module_for_path(options.path)?;

    let tracing = TracingConfig {
        register_fuzzables: TracingMode::Off,
        calls: TracingMode::All,
        evaluated_expressions: TracingMode::Off,
    };

    debug!("Running {module}.");

    let compilation_start = Instant::now();
    let byte_code = Rc::new(compile_byte_code(&db, module.clone(), tracing).0);

    let compilation_end = Instant::now();
    debug!(
        "Compilation took {}.",
        format_duration(compilation_end - compilation_start),
    );

    let mut heap = Heap::default();
    let VmFinished { tracer, result } =
        Vm::for_module(&*byte_code, &mut heap, StackTracer::default())
            .run_forever_without_handles(&mut heap);
    let exports = match result {
        Ok(exports) => exports,
        Err(panic) => {
            error!("The module panicked: {}", panic.reason);
            error!("{} is responsible.", panic.responsible);
            error!(
                "This is the stack trace:\n{}",
                tracer.format(&db, &packages_path),
            );
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
    let (environment_object, mut environment) =
        DefaultEnvironment::new(&mut heap, &options.arguments);
    let platform = HirId::create(&mut heap, true, hir::Id::platform());
    let vm = Vm::for_function(
        byte_code.clone(),
        &mut heap,
        platform,
        main,
        &[environment_object, platform.into()],
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
            error!(
                "This is the stack trace:\n{}",
                tracer.format(&db, &packages_path),
            );
            Err(Exit::CodePanicked)
        }
    };
    let execution_end = Instant::now();
    debug!(
        "Execution took {}.",
        format_duration(execution_end - discovery_end),
    );

    drop(byte_code); // Make sure the byte code is kept around until here.
    result
}

fn format_duration(duration: Duration) -> String {
    if duration < Duration::from_millis(1) {
        format!("{} µs", duration.as_micros())
    } else {
        format!("{} ms", duration.as_millis())
    }
}
