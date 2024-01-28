use crate::{
    database::Database,
    utils::{module_for_path, packages_path},
    Exit, ProgramResult,
};
use candy_frontend::{
    hir_to_mir::ExecutionTarget, tracing::CallTracingMode, TracingConfig, TracingMode,
};
use candy_vm::{
    environment::DefaultEnvironment, heap::Heap, lir_to_byte_code::compile_byte_code,
    tracer::stack_trace::StackTracer, Vm, VmFinished,
};
use clap::{Parser, ValueHint};
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};
use tracing::{debug, error};

/// Run a Candy program.
///
/// This command runs the given file, or, if no file is provided, the package of
/// your current working directory. The module should export a `main` function.
/// This function is then called with an environment.
#[derive(Parser, Debug)]
pub struct Options {
    /// The file or package to run. If none is provided, the package of your
    /// current working directory will be run.
    #[arg(value_hint = ValueHint::FilePath)]
    path: Option<PathBuf>,

    #[arg(last(true))]
    arguments: Vec<String>,
}

pub fn run(options: Options) -> ProgramResult {
    let packages_path = packages_path();
    let db = Database::new_with_file_system_module_provider(packages_path.clone());
    let module = module_for_path(options.path)?;

    let tracing = TracingConfig {
        register_fuzzables: TracingMode::Off,
        calls: CallTracingMode::OnlyForPanicTraces,
        evaluated_expressions: TracingMode::Off,
    };

    debug!("Running {module}.");

    let compilation_start = Instant::now();
    let byte_code = compile_byte_code(&db, ExecutionTarget::MainFunction(module), tracing).0;

    let compilation_end = Instant::now();
    debug!(
        "Compilation took {}.",
        format_duration(compilation_end - compilation_start),
    );

    debug!("Running program.");
    let mut heap = Heap::default();
    let (environment_object, mut environment) =
        DefaultEnvironment::new(&mut heap, &options.arguments);
    let vm = Vm::for_main_function(
        &byte_code,
        &mut heap,
        environment_object,
        StackTracer::default(),
    );
    let VmFinished { result, tracer, .. } =
        vm.run_forever_with_environment(&mut heap, &mut environment);
    let result = match result {
        Ok(return_value) => {
            debug!("The main function returned: {return_value:?}");
            Ok(())
        }
        Err(panic) => {
            error!("The program panicked: {}", panic.reason);
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
        format_duration(execution_end - compilation_end),
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
