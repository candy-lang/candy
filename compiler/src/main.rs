#![feature(async_closure)]
#![feature(box_patterns)]
#![feature(let_chains)]
#![feature(never_type)]
#![feature(try_trait_v2)]
#![allow(clippy::module_inception)]

mod builtin_functions;
mod compiler;
mod database;
mod fuzzer;
mod language_server;
mod module;
mod utils;
mod vm;

use crate::{
    compiler::{
        ast_to_hir::AstToHir,
        cst_to_ast::CstToAst,
        error::CompilerError,
        hir::{self, CollectErrors, Id},
        mir_to_lir::MirToLir,
        rcst_to_cst::RcstToCst,
        string_to_rcst::StringToRcst,
    },
    database::Database,
    language_server::utils::LspPositionConversion,
    module::{Module, ModuleKind},
    vm::{
        context::{DbUseProvider, RunForever},
        tracer::{full::FullTracer, Tracer},
        Closure, ExecutionResult, FiberId, Status, Struct, Vm,
    },
};
use compiler::{
    hir_to_mir::{HirToMir, TracingConfig},
    lir::Lir,
    optimize::OptimizeMir,
};
use itertools::Itertools;
use language_server::CandyLanguageServer;
use notify::{watcher, RecursiveMode, Watcher};
use std::{
    collections::HashMap,
    convert::TryInto,
    env::current_dir,
    path::PathBuf,
    sync::{mpsc::channel, Arc},
    time::Duration,
};
use structopt::StructOpt;
use tower_lsp::{LspService, Server};
use tracing::{debug, error, info, warn, Level, Metadata};
use tracing_subscriber::{
    filter,
    fmt::{format::FmtSpan, writer::BoxMakeWriter},
    prelude::*,
};
use vm::{ChannelId, CompletedOperation, OperationId};

#[derive(StructOpt, Debug)]
#[structopt(name = "candy", about = "The 🍭 Candy CLI.")]
enum CandyOptions {
    Build(CandyBuildOptions),
    Run(CandyRunOptions),
    Fuzz(CandyFuzzOptions),
    Lsp,
}

#[derive(StructOpt, Debug)]
struct CandyBuildOptions {
    #[structopt(long)]
    debug: bool,

    #[structopt(long)]
    watch: bool,

    #[structopt(parse(from_os_str))]
    file: PathBuf,
}

#[derive(StructOpt, Debug)]
struct CandyRunOptions {
    #[structopt(long)]
    debug: bool,

    #[structopt(parse(from_os_str))]
    file: PathBuf,
}

#[derive(StructOpt, Debug)]
struct CandyFuzzOptions {
    #[structopt(parse(from_os_str))]
    file: PathBuf,
}

#[tokio::main]
async fn main() -> ProgramResult {
    match CandyOptions::from_args() {
        CandyOptions::Build(options) => build(options),
        CandyOptions::Run(options) => run(options),
        CandyOptions::Fuzz(options) => fuzz(options).await,
        CandyOptions::Lsp => lsp().await,
    }
}

type ProgramResult = Result<(), Exit>;
#[derive(Debug)]
enum Exit {
    FileNotFound,
    FuzzingFoundFailingCases,
    CodePanicked,
}

fn build(options: CandyBuildOptions) -> ProgramResult {
    init_logger(true);
    let db = Database::default();
    let module = Module::from_package_root_and_file(
        current_dir().unwrap(),
        options.file.clone(),
        ModuleKind::Code,
    );
    let result = raw_build(
        &db,
        module.clone(),
        &TracingConfig::default(),
        options.debug,
    );

    if !options.watch {
        result.ok_or(Exit::FileNotFound).map(|_| ())
    } else {
        let (tx, rx) = channel();
        let mut watcher = watcher(tx, Duration::from_secs(1)).unwrap();
        watcher
            .watch(&options.file, RecursiveMode::Recursive)
            .unwrap();
        loop {
            match rx.recv() {
                Ok(_) => {
                    raw_build(
                        &db,
                        module.clone(),
                        &TracingConfig::default(),
                        options.debug,
                    );
                }
                Err(e) => error!("watch error: {e:#?}"),
            }
        }
    }
}
fn raw_build(
    db: &Database,
    module: Module,
    config: &TracingConfig,
    debug: bool,
) -> Option<Arc<Lir>> {
    tracing::span!(Level::DEBUG, "Parsing string to RCST").in_scope(|| {
        let rcst = db
            .rcst(module.clone())
            .unwrap_or_else(|err| panic!("Error parsing file `{}`: {:?}", module, err));
        if debug {
            module.dump_associated_debug_file("rcst", &format!("{:#?}\n", rcst));
        }
    });

    tracing::span!(Level::DEBUG, "Turning RCST to CST").in_scope(|| {
        let cst = db.cst(module.clone()).unwrap();
        if debug {
            module.dump_associated_debug_file("cst", &format!("{:#?}\n", cst));
        }
    });

    tracing::span!(Level::DEBUG, "Abstracting CST to AST").in_scope(|| {
        let (asts, ast_cst_id_map) = db.ast(module.clone()).unwrap();
        if debug {
            module.dump_associated_debug_file(
                "ast",
                &format!("{}\n", asts.iter().map(|ast| format!("{}", ast)).join("\n")),
            );
            module.dump_associated_debug_file(
                "ast_to_cst_ids",
                &ast_cst_id_map
                    .keys()
                    .into_iter()
                    .sorted_by_key(|it| it.local)
                    .map(|key| format!("{key} -> {}\n", ast_cst_id_map[key].0))
                    .join(""),
            );
        }
    });

    tracing::span!(Level::DEBUG, "Turning AST to HIR").in_scope(|| {
        let (hir, hir_ast_id_map) = db.hir(module.clone()).unwrap();
        if debug {
            module.dump_associated_debug_file("hir", &format!("{}", hir));
            module.dump_associated_debug_file(
                "hir_to_ast_ids",
                &hir_ast_id_map
                    .keys()
                    .into_iter()
                    .map(|key| format!("{key} -> {}\n", hir_ast_id_map[key]))
                    .join(""),
            );
        }

        let mut errors = vec![];
        hir.collect_errors(&mut errors);
        for CompilerError { span, payload, .. } in errors {
            let (start_line, start_col) = db.offset_to_lsp(module.clone(), span.start);
            let (end_line, end_col) = db.offset_to_lsp(module.clone(), span.end);
            warn!("{start_line}:{start_col} – {end_line}:{end_col}: {payload:?}");
        }
    });

    tracing::span!(Level::DEBUG, "Turning HIR to MIR").in_scope(|| {
        let mir = db.mir(module.clone(), config.clone()).unwrap();
        if debug {
            module.dump_associated_debug_file("mir", &format!("{mir:?}"));
        }
    });
    tracing::span!(Level::DEBUG, "Optimizing MIR").in_scope(|| {
        let mir = db
            .mir_with_obvious_optimized(module.clone(), config.clone())
            .unwrap();
        if debug {
            module.dump_associated_debug_file("optimized_mir", &format!("{mir:?}"));
        }
    });

    let lir = tracing::span!(Level::DEBUG, "Lowering MIR to LIR").in_scope(|| {
        let lir = db.lir(module.clone(), config.clone()).unwrap();
        if debug {
            module.dump_associated_debug_file("lir", &format!("{lir}"));
        }
        lir
    });

    Some(lir)
}

fn run(options: CandyRunOptions) -> ProgramResult {
    init_logger(true);
    let db = Database::default();
    let module = Module::from_package_root_and_file(
        current_dir().unwrap(),
        options.file.clone(),
        ModuleKind::Code,
    );

    let config = TracingConfig {
        register_fuzzables: false,
        trace_calls: false,
        trace_evaluated_expressions: false,
    };
    if raw_build(&db, module.clone(), &config, false).is_none() {
        warn!("File not found.");
        return Err(Exit::FileNotFound);
    };

    let path_string = options.file.to_string_lossy();
    debug!("Running `{path_string}`.");

    let module_closure = Closure::of_module(&db, module.clone(), config.clone()).unwrap();
    let mut tracer = FullTracer::default();

    let mut vm = Vm::new();
    vm.set_up_for_running_module_closure(module.clone(), module_closure);
    vm.run(
        &DbUseProvider { db: &db, config },
        &mut RunForever,
        &mut tracer,
    );
    if let Status::WaitingForOperations = vm.status() {
        error!("The module waits on channel operations. Perhaps, the code tried to read from a channel without sending a packet into it.");
        // TODO: Show stack traces of all fibers?
    }
    let result = vm.tear_down();

    if options.debug {
        module.dump_associated_debug_file("trace", &format!("{tracer:?}"));
    }

    let (mut heap, exported_definitions): (_, Struct) = match result {
        ExecutionResult::Finished(return_value) => {
            debug!("The module exports these definitions: {return_value:?}",);
            let exported = return_value
                .heap
                .get(return_value.address)
                .data
                .clone()
                .try_into()
                .unwrap();
            (return_value.heap, exported)
        }
        ExecutionResult::Panicked {
            reason,
            responsible,
        } => {
            error!("The module panicked because {reason}.");
            error!("{responsible} is responsible.");
            let span = db.hir_id_to_span(responsible).unwrap();
            error!("Responsible is at {span:?}.");
            error!(
                "This is the stack trace:\n{}",
                tracer.format_panic_stack_trace_to_root_fiber(&db)
            );
            return Err(Exit::CodePanicked);
        }
    };

    let main = heap.create_symbol("Main".to_string());
    let main = match exported_definitions.get(&heap, main) {
        Some(main) => main,
        None => {
            error!("The module doesn't contain a main function.");
            return Err(Exit::CodePanicked);
        }
    };

    debug!("Running main function.");
    // TODO: Add more environment stuff.
    let mut vm = Vm::new();
    let mut stdout = StdoutService::new(&mut vm);
    let environment = {
        let stdout_symbol = heap.create_symbol("Stdout".to_string());
        let stdout_port = heap.create_send_port(stdout.channel);
        heap.create_struct(HashMap::from([(stdout_symbol, stdout_port)]))
    };
    let platform = heap.create_hir_id(Id::platform());
    tracer.for_fiber(FiberId::root()).call_started(
        platform,
        main,
        vec![environment],
        platform,
        &heap,
    );
    vm.set_up_for_running_closure(heap, main, &[environment], Id::platform());
    loop {
        match vm.status() {
            Status::CanRun => {
                debug!("VM still running.");
                vm.run(
                    &DbUseProvider {
                        db: &db,
                        config: TracingConfig::default(),
                    },
                    &mut RunForever,
                    &mut tracer,
                );
            }
            Status::WaitingForOperations => {}
            _ => break,
        }
        stdout.run(&mut vm);
        for channel in vm.unreferenced_channels.iter().copied().collect_vec() {
            vm.free_channel(channel);
        }
    }
    match vm.tear_down() {
        ExecutionResult::Finished(return_value) => {
            tracer
                .for_fiber(FiberId::root())
                .call_ended(return_value.address, &return_value.heap);
            debug!("The main function returned: {return_value:?}");
            Ok(())
        }
        ExecutionResult::Panicked {
            reason,
            responsible,
        } => {
            error!("The main function panicked because {reason}.");
            error!("{responsible} is responsible.");
            error!(
                "This is the stack trace:\n{}",
                tracer.format_panic_stack_trace_to_root_fiber(&db)
            );
            Err(Exit::CodePanicked)
        }
    }
}

/// A state machine that corresponds to a loop that always calls `receive` on
/// the stdout channel and then logs that packet.
struct StdoutService {
    channel: ChannelId,
    current_receive: OperationId,
}
impl StdoutService {
    fn new(vm: &mut Vm) -> Self {
        let channel = vm.create_channel(1);
        let current_receive = vm.receive(channel);
        Self {
            channel,
            current_receive,
        }
    }
    fn run(&mut self, vm: &mut Vm) {
        if let Some(CompletedOperation::Received { packet }) =
            vm.completed_operations.remove(&self.current_receive)
        {
            info!("Sent to stdout: {packet:?}");
            self.current_receive = vm.receive(self.channel);
        }
    }
}

async fn fuzz(options: CandyFuzzOptions) -> ProgramResult {
    init_logger(true);
    let db = Database::default();
    let module = Module::from_package_root_and_file(
        current_dir().unwrap(),
        options.file.clone(),
        ModuleKind::Code,
    );
    let config = TracingConfig {
        register_fuzzables: true,
        trace_calls: false,
        trace_evaluated_expressions: false,
    };

    if raw_build(&db, module.clone(), &config, false).is_none() {
        warn!("File not found.");
        return Err(Exit::FileNotFound);
    }

    debug!("Fuzzing `{module}`.");
    let failing_cases = fuzzer::fuzz(&db, module).await;

    if failing_cases.is_empty() {
        info!("All found fuzzable closures seem fine.");
        Ok(())
    } else {
        error!("");
        error!("Finished fuzzing.");
        error!("These are the failing cases:");
        for case in failing_cases {
            error!("");
            case.dump(&db);
        }
        Err(Exit::FuzzingFoundFailingCases)
    }
}

async fn lsp() -> ProgramResult {
    init_logger(false);
    info!("Starting language server…");
    let (service, socket) = LspService::new(CandyLanguageServer::from_client);
    Server::new(tokio::io::stdin(), tokio::io::stdout(), socket)
        .serve(service)
        .await;
    Ok(())
}

fn init_logger(use_stdout: bool) {
    let writer = if use_stdout {
        BoxMakeWriter::new(std::io::stdout)
    } else {
        BoxMakeWriter::new(std::io::stderr)
    };
    let console_log = tracing_subscriber::fmt::layer()
        .compact()
        .with_writer(writer)
        .with_span_events(FmtSpan::ENTER)
        .with_filter(filter::filter_fn(|metadata| {
            // For external packages, show only the error logs.
            metadata.level() <= &Level::ERROR
                || metadata
                    .module_path()
                    .unwrap_or_default()
                    .starts_with("candy")
        }))
        .with_filter(filter::filter_fn(level_for(
            "candy::compiler::optimize",
            Level::DEBUG,
        )))
        .with_filter(filter::filter_fn(level_for(
            "candy::compiler::string_to_rcst",
            Level::WARN,
        )))
        .with_filter(filter::filter_fn(level_for(
            "candy::compiler",
            Level::DEBUG,
        )))
        .with_filter(filter::filter_fn(level_for(
            "candy::language_server",
            Level::TRACE,
        )))
        .with_filter(filter::filter_fn(level_for("candy::vm", Level::DEBUG)))
        .with_filter(filter::filter_fn(level_for(
            "candy::vm::heap",
            Level::DEBUG,
        )));
    tracing_subscriber::registry().with(console_log).init();
}
fn level_for(module: &'static str, level: Level) -> impl Fn(&Metadata) -> bool {
    move |metadata| {
        if metadata
            .module_path()
            .unwrap_or_default()
            .starts_with(module)
        {
            metadata.level() <= &level
        } else {
            true
        }
    }
}
