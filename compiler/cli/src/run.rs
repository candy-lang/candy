use crate::{
    database::Database,
    services::{stdin::StdinService, stdout::StdoutService},
    utils::{module_for_path, packages_path},
    Exit, ProgramResult,
};
use candy_frontend::{ast_to_hir::AstToHir, hir, TracingConfig};
use candy_vm::{
    execution_controller::RunForever,
    fiber::EndedReason,
    heap::{HirId, SendPort, Struct, SymbolId},
    mir_to_lir::compile_lir,
    return_value_into_main_function,
    tracer::stack_trace::StackTracer,
    vm::{Status, Vm},
};
use clap::{Parser, ValueHint};
use std::{path::PathBuf, rc::Rc};
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
    let db = Database::new_with_file_system_module_provider(packages_path);
    let module = module_for_path(options.path)?;

    let tracing = TracingConfig::off();

    debug!("Running {module}.");

    let mut tracer = StackTracer::default();
    let lir = Rc::new(compile_lir(&db, module, tracing).0);

    let mut ended = Vm::for_module(&*lir, &mut tracer).run_until_completion(&mut tracer);

    let main = match ended.reason {
        EndedReason::Finished(return_value) => {
            return_value_into_main_function(&lir.symbol_table, return_value).unwrap()
        }
        EndedReason::Panicked(panic) => {
            error!("The module panicked: {}", panic.reason);
            error!("{} is responsible.", panic.responsible);
            if let Some(span) = db.hir_id_to_span(panic.responsible) {
                error!("Responsible is at {span:?}.");
            }
            error!(
                "This is the stack trace:\n{}",
                tracer.format_panic_stack_trace_to_root_fiber(&db, &lir.as_ref().symbol_table),
            );
            return Err(Exit::CodePanicked);
        }
    };

    debug!("Running main function.");
    // TODO: Add more environment stuff.
    let mut vm = Vm::uninitialized(lir.clone());
    let mut stdout = StdoutService::new(&mut vm);
    let mut stdin = StdinService::new(&mut vm);
    let fields = [
        (
            SymbolId::STDOUT,
            SendPort::create(&mut ended.heap, stdout.channel),
        ),
        (
            SymbolId::STDIN,
            SendPort::create(&mut ended.heap, stdin.channel),
        ),
    ];
    let environment = Struct::create_with_symbol_keys(&mut ended.heap, true, fields).into();
    let mut tracer = StackTracer::default();
    let platform = HirId::create(&mut ended.heap, true, hir::Id::platform());
    vm.initialize_for_function(ended.heap, main, &[environment], platform, &mut tracer);
    loop {
        match vm.status() {
            Status::CanRun => {
                vm.run(&mut RunForever, &mut tracer);
            }
            Status::WaitingForOperations => {}
            _ => break,
        }
        stdout.run(&mut vm);
        stdin.run(&mut vm);
        vm.free_unreferenced_channels();
    }
    let ended = vm.tear_down(&mut tracer);
    let result = match ended.reason {
        EndedReason::Finished(return_value) => {
            debug!("The main function returned: {return_value:?}");
            Ok(())
        }
        EndedReason::Panicked(panic) => {
            error!("The main function panicked: {}", panic.reason);
            error!("{} is responsible.", panic.responsible);
            error!(
                "This is the stack trace:\n{}",
                tracer.format_panic_stack_trace_to_root_fiber(&db, &lir.as_ref().symbol_table),
            );
            Err(Exit::CodePanicked)
        }
    };
    drop(lir); // Make sure the LIR is kept around until here.
    result
}
