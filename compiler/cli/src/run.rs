use crate::{
    database::Database,
    services::{stdin::StdinService, stdout::StdoutService},
    utils::{module_for_path, packages_path},
    CandyRunOptions, Exit, ProgramResult,
};
use candy_frontend::{ast_to_hir::AstToHir, hir, rich_ir::ToRichIr, TracingConfig};
use candy_vm::{
    context::{DbUseProvider, RunForever},
    fiber::{ExecutionResult, FiberId},
    heap::{HirId, SendPort, Struct},
    mir_to_lir::MirToLir,
    run_lir,
    tracer::{full::FullTracer, Tracer},
    vm::{Status, Vm},
};
use tracing::{debug, error};

pub(crate) fn run(options: CandyRunOptions) -> ProgramResult {
    let packages_path = packages_path();
    let db = Database::new_with_file_system_module_provider(packages_path);
    let module = module_for_path(options.path)?;

    let tracing = TracingConfig::off();

    debug!("Running {}.", module.to_rich_ir());

    let mut tracer = FullTracer::default();
    let lir = db.lir(module.clone(), tracing.clone()).unwrap();
    let use_provider = DbUseProvider {
        db: &db,
        tracing: tracing.clone(),
    };
    let result = run_lir(module, lir.as_ref().to_owned(), &use_provider, &mut tracer);

    let (mut heap, main) = match result {
        ExecutionResult::Finished(return_value) => return_value.into_main_function().unwrap(),
        ExecutionResult::Panicked {
            reason,
            responsible,
        } => {
            error!("The module panicked: {reason}");
            error!("{responsible} is responsible.");
            if let Some(span) = db.hir_id_to_span(responsible) {
                error!("Responsible is at {span:?}.");
            }
            error!(
                "This is the stack trace:\n{}",
                tracer.format_panic_stack_trace_to_root_fiber(&db),
            );
            return Err(Exit::CodePanicked);
        }
    };

    debug!("Running main function.");
    // TODO: Add more environment stuff.
    let mut vm = Vm::default();
    let mut stdout = StdoutService::new(&mut vm);
    let mut stdin = StdinService::new(&mut vm);
    let fields = [
        ("Stdout", SendPort::create(&mut heap, stdout.channel)),
        ("Stdin", SendPort::create(&mut heap, stdin.channel)),
    ];
    let environment = Struct::create_with_symbol_keys(&mut heap, fields).into();
    let platform = HirId::create(&mut heap, hir::Id::platform());
    tracer.for_fiber(FiberId::root()).call_started(
        platform,
        main.into(),
        vec![environment],
        platform,
        &heap,
    );
    vm.set_up_for_running_closure(heap, main, &[environment], hir::Id::platform());
    loop {
        match vm.status() {
            Status::CanRun => {
                vm.run(
                    &DbUseProvider {
                        db: &db,
                        tracing: tracing.clone(),
                    },
                    &mut RunForever,
                    &mut tracer,
                );
            }
            Status::WaitingForOperations => {}
            _ => break,
        }
        stdout.run(&mut vm);
        stdin.run(&mut vm);
        vm.free_unreferenced_channels();
    }
    match vm.tear_down() {
        ExecutionResult::Finished(return_value) => {
            tracer
                .for_fiber(FiberId::root())
                .call_ended(return_value.object, &return_value.heap);
            debug!("The main function returned: {return_value:?}");
            Ok(())
        }
        ExecutionResult::Panicked {
            reason,
            responsible,
        } => {
            error!("The main function panicked: {reason}");
            error!("{responsible} is responsible.");
            error!(
                "This is the stack trace:\n{}",
                tracer.format_panic_stack_trace_to_root_fiber(&db)
            );
            Err(Exit::CodePanicked)
        }
    }
}
