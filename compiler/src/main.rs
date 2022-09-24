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
mod vm;

use crate::{
    compiler::{
        ast_to_hir::AstToHir,
        cst_to_ast::CstToAst,
        error::CompilerError,
        hir::{self, CollectErrors},
        hir_to_lir::HirToLir,
        rcst_to_cst::RcstToCst,
        string_to_rcst::StringToRcst,
    },
    database::Database,
    language_server::utils::LspPositionConversion,
    module::{Module, ModuleKind},
    vm::{
        context::{DbUseProvider, ModularContext, RunForever},
        Closure, Status, Struct, TearDownResult, Vm,
    },
};
use compiler::lir::Lir;
use itertools::Itertools;
use language_server::CandyLanguageServer;
use notify::{watcher, RecursiveMode, Watcher};
use std::{
    convert::TryInto,
    env::current_dir,
    path::PathBuf,
    sync::{mpsc::channel, Arc},
    time::Duration,
};
use structopt::StructOpt;
use tower_lsp::{LspService, Server};
use tracing::{debug, error, info, warn, Level, Metadata};
use tracing_subscriber::{filter, fmt::format::FmtSpan, prelude::*};

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
async fn main() {
    init_logger();
    match CandyOptions::from_args() {
        CandyOptions::Build(options) => build(options),
        CandyOptions::Run(options) => run(options),
        CandyOptions::Fuzz(options) => fuzz(options).await,
        CandyOptions::Lsp => lsp().await,
    }
}

fn build(options: CandyBuildOptions) {
    let module = Module::from_package_root_and_file(
        current_dir().unwrap(),
        options.file.clone(),
        ModuleKind::Code,
    );
    raw_build(module.clone(), options.debug);

    if options.watch {
        let (tx, rx) = channel();
        let mut watcher = watcher(tx, Duration::from_secs(1)).unwrap();
        watcher
            .watch(&options.file, RecursiveMode::Recursive)
            .unwrap();
        loop {
            match rx.recv() {
                Ok(_) => {
                    raw_build(module.clone(), options.debug);
                }
                Err(e) => error!("watch error: {e:#?}"),
            }
        }
    }
}
fn raw_build(module: Module, debug: bool) -> Option<Arc<Lir>> {
    let db = Database::default();

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

    let lir = tracing::span!(Level::DEBUG, "Lowering HIR to LIR").in_scope(|| {
        let lir = db.lir(module.clone()).unwrap();
        if debug {
            module.dump_associated_debug_file("lir", &format!("{lir}"));
        }
        lir
    });

    Some(lir)
}

fn run(options: CandyRunOptions) {
    let module = Module::from_package_root_and_file(
        current_dir().unwrap(),
        options.file.clone(),
        ModuleKind::Code,
    );
    let db = Database::default();

    if raw_build(module.clone(), false).is_none() {
        warn!("Build failed.");
        return;
    };
    let module_closure = Closure::of_module(&db, module.clone()).unwrap();

    let path_string = options.file.to_string_lossy();
    info!("Running `{path_string}`.");
    let mut vm = Vm::new();
    vm.set_up_for_running_module_closure(module_closure);
    loop {
        info!("Tree: {:#?}", vm);
        match vm.status() {
            Status::CanRun => {
                debug!("VM still running.");
                vm.run(&mut ModularContext {
                    use_provider: DbUseProvider { db: &db },
                    execution_controller: RunForever,
                });
                // TODO: handle operations
            }
            Status::WaitingForOperations => {
                todo!("VM can't proceed until some operations complete.");
            }
            _ => break,
        }
    }
    info!("Tree: {:#?}", vm);
    let TearDownResult {
        tracer,
        result,
        mut heap,
        ..
    } = vm.tear_down();

    if options.debug {
        module.dump_associated_debug_file("trace", &tracer.full_trace().format(&heap));
    }

    let exported_definitions: Struct = match result {
        Ok(return_value) => {
            info!(
                "The module exports these definitions: {}",
                return_value.format(&heap),
            );
            heap.get(return_value).data.clone().try_into().unwrap()
        }
        Err(reason) => {
            error!("The module panicked because {reason}.");
            error!("This is the stack trace:");
            tracer.dump_stack_trace(&db, &heap);
            return;
        }
    };

    let main = heap.create_symbol("Main".to_string());
    let main = match exported_definitions.get(&heap, main) {
        Some(main) => main,
        None => {
            error!("The module doesn't contain a main function.");
            return;
        }
    };

    info!("Running main function.");
    // TODO: Add environment stuff.
    let mut vm = Vm::new();
    let stdout = vm.create_channel();
    let environment = {
        let stdout_symbol = heap.create_symbol("Stdout".to_string());
        let stdout_port = heap.create_send_port(stdout);
        heap.create_struct([(stdout_symbol, stdout_port)].into_iter().collect())
    };
    vm.set_up_for_running_closure(heap, main, &[environment]);
    loop {
        info!("Tree: {:#?}", vm);
        match vm.status() {
            Status::CanRun => {
                debug!("VM still running.");
                vm.run(&mut ModularContext {
                    use_provider: DbUseProvider { db: &db },
                    execution_controller: RunForever,
                });
                // TODO: handle operations
            }
            Status::WaitingForOperations => {
                todo!("VM can't proceed until some operations complete.");
            }
            _ => break,
        }
        let stdout_operations = vm
            .external_operations
            .get_mut(&stdout)
            .unwrap()
            .drain(..)
            .collect_vec();
        for operation in stdout_operations {
            match operation {
                vm::Operation::Send {
                    performing_fiber,
                    packet,
                } => {
                    info!("Sent to stdout: {}", packet.value.format(&packet.heap));
                    vm.complete_send(performing_fiber);
                }
                vm::Operation::Receive { .. } => unreachable!(),
                vm::Operation::Drop => vm.free_channel(stdout),
            }
        }
    }
    info!("Tree: {:#?}", vm);
    let TearDownResult {
        tracer,
        result,
        heap,
        ..
    } = vm.tear_down();

    match result {
        Ok(return_value) => info!("The main function returned: {}", return_value.format(&heap)),
        Err(reason) => {
            error!("The main function panicked because {reason}.");
            error!("This is the stack trace:");
            tracer.dump_stack_trace(&db, &heap);
        }
    }
}

async fn fuzz(options: CandyFuzzOptions) {
    let module = Module::from_package_root_and_file(
        current_dir().unwrap(),
        options.file.clone(),
        ModuleKind::Code,
    );

    if raw_build(module.clone(), false).is_none() {
        info!("Build failed.");
        return;
    }

    debug!("Fuzzing `{module}`.");
    let db = Database::default();
    fuzzer::fuzz(&db, module).await;
}

async fn lsp() {
    info!("Starting language server…");
    let (service, socket) = LspService::new(CandyLanguageServer::from_client);
    Server::new(tokio::io::stdin(), tokio::io::stdout(), socket)
        .serve(service)
        .await;
}

fn init_logger() {
    let console_log = tracing_subscriber::fmt::layer()
        .compact()
        .with_span_events(FmtSpan::ENTER)
        .with_filter(filter::filter_fn(|metadata| {
            // For external packages, show only the error logs.
            metadata.level() <= &Level::ERROR
                || metadata
                    .module_path()
                    .unwrap_or_default()
                    .starts_with("candy")
        }))
        .with_filter(filter::filter_fn(level_for("candy::compiler", Level::WARN)))
        .with_filter(filter::filter_fn(level_for(
            "candy::language_server",
            Level::DEBUG,
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
