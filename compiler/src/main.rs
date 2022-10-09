#![feature(async_closure)]
#![feature(box_patterns)]
#![feature(label_break_value)]
#![feature(let_else)]
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
    vm::{use_provider::DbUseProvider, Closure, TearDownResult, Vm},
};
use compiler::lir::Lir;
use itertools::Itertools;
use language_server::CandyLanguageServer;
use notify::{watcher, RecursiveMode, Watcher};
use std::{
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

    let use_provider = DbUseProvider { db: &db };
    let vm = Vm::new_for_running_module_closure(&use_provider, module_closure);
    let TearDownResult {
        tracer,
        result,
        heap,
        ..
    } = vm.run_synchronously_until_completion(&db);

    match result {
        Ok(return_value) => info!(
            "The module exports these definitions: {}",
            return_value.format(&heap)
        ),
        Err(reason) => {
            error!("The module panicked because {reason}.");
            error!("This is the stack trace:");
            tracer.dump_stack_trace(&db, &heap);
        }
    }

    if options.debug {
        module.dump_associated_debug_file("trace", &tracer.format_call_tree(&heap));
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
