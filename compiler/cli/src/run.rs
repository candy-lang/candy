use crate::{
    database::Database,
    utils::{module_for_path, packages_path},
    Exit, ProgramResult,
};
use candy_frontend::{ast_to_hir::AstToHir, hir, TracingConfig};
use candy_vm::{
    heap::{Data, Handle, HirId, Struct, Tag, Text},
    mir_to_lir::compile_lir,
    tracer::stack_trace::StackTracer,
    StateAfterRunForever, Vm, VmFinished,
};
use clap::{Parser, ValueHint};
use std::{
    io::{self, BufRead, Write},
    path::PathBuf,
    rc::Rc,
    time::{Duration, Instant},
};
use tracing::{debug, error, info};

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
    let db = Database::new_with_file_system_module_provider(packages_path);
    let module = module_for_path(options.path)?;

    let tracing = TracingConfig::off();

    debug!("Running {module}.");

    let compilation_start = Instant::now();
    let lir = Rc::new(compile_lir(&db, module, tracing).0);

    let compilation_end = Instant::now();
    debug!(
        "Compilation took {}.",
        format_duration(compilation_end - compilation_start),
    );

    let VmFinished {
        mut heap,
        tracer,
        result,
    } = Vm::for_module(&*lir, StackTracer::default()).run_forever_without_handles();
    let exports = match result {
        Ok(exports) => exports,
        Err(panic) => {
            error!("The module panicked: {}", panic.reason);
            error!("{} is responsible.", panic.responsible);
            if let Some(span) = db.hir_id_to_span(&panic.responsible) {
                error!("Responsible is at {span:?}.");
            }
            error!("This is the stack trace:\n{}", tracer.format(&db),);
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
    // TODO: Add more environment stuff.
    let stdout_symbol = heap.default_symbols().stdout;
    let stdout = Handle::new(&mut heap, 1);
    let stdin_symbol = heap.default_symbols().stdin;
    let stdin = Handle::new(&mut heap, 0);
    let environment = Struct::create_with_symbol_keys(
        &mut heap,
        true,
        [(stdout_symbol, **stdout), (stdin_symbol, **stdin)],
    )
    .into();
    let platform = HirId::create(&mut heap, true, hir::Id::platform());
    let mut vm = Vm::for_function(
        lir.clone(),
        heap,
        main,
        &[environment],
        platform,
        StackTracer::default(),
    );

    let result = loop {
        match vm.run_forever() {
            StateAfterRunForever::CallingHandle(mut call) => {
                if call.handle == stdout {
                    let message = call.arguments[0];

                    match message.into() {
                        Data::Text(text) => println!("{}", text.get()),
                        _ => info!("Non-text value sent to stdout: {message:?}"),
                    }
                    let nothing = Tag::create_nothing(call.heap());
                    vm = call.complete(nothing);
                } else if call.handle == stdin {
                    print!(">> ");
                    io::stdout().flush().unwrap();
                    let input = {
                        let stdin = io::stdin();
                        stdin.lock().lines().next().unwrap().unwrap()
                    };
                    let text = Text::create(call.heap(), true, &input);
                    vm = call.complete(text);
                } else {
                    unreachable!()
                }
            }
            StateAfterRunForever::Finished(VmFinished { result, .. }) => match result {
                Ok(return_value) => {
                    debug!("The main function returned: {return_value:?}");
                    break Ok(());
                }
                Err(panic) => {
                    error!("The main function panicked: {}", panic.reason);
                    error!("{} is responsible.", panic.responsible);
                    error!("This is the stack trace:\n{}", tracer.format(&db),);
                    break Err(Exit::CodePanicked);
                }
            },
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
