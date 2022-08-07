#![feature(async_closure)]
#![feature(box_patterns)]
#![feature(label_break_value)]
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
use fern::colors::{Color, ColoredLevelConfig};
use itertools::Itertools;
use language_server::CandyLanguageServer;
use log::{self, LevelFilter};
use notify::{watcher, RecursiveMode, Watcher};
use std::{
    env::current_dir,
    fs,
    path::PathBuf,
    sync::{mpsc::channel, Arc},
    time::Duration,
};
use structopt::StructOpt;
use tower_lsp::{LspService, Server};

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
                Err(e) => log::error!("watch error: {e:#?}"),
            }
        }
    }
}
fn raw_build(module: Module, debug: bool) -> Option<Arc<Lir>> {
    log::info!("Building `{module}`.");

    let db = Database::default();

    log::debug!("Parsing string to RCST…");
    let rcst = db
        .rcst(module.clone())
        .unwrap_or_else(|err| panic!("Error parsing file `{}`: {:?}", module, err));
    if debug {
        let rcst_file = module.associated_debug_file("rcst");
        fs::write(rcst_file, format!("{:#?}\n", rcst)).unwrap();
    }

    log::debug!("Turning RCST to CST…");
    let cst = db.cst(module.clone()).unwrap();
    if debug {
        let cst_file = module.associated_debug_file("cst");
        fs::write(cst_file, format!("{:#?}\n", cst)).unwrap();
    }

    log::debug!("Abstracting CST to AST…");
    let (asts, ast_cst_id_map) = db.ast(module.clone()).unwrap();
    if debug {
        let ast_file = module.associated_debug_file("ast");
        fs::write(
            ast_file,
            format!("{}\n", asts.iter().map(|ast| format!("{}", ast)).join("\n")),
        )
        .unwrap();

        let ast_to_cst_ids_file = module.associated_debug_file("ast_to_cst_ids");
        fs::write(
            ast_to_cst_ids_file,
            ast_cst_id_map
                .keys()
                .into_iter()
                .sorted_by_key(|it| it.local)
                .map(|key| format!("{key} -> {}\n", ast_cst_id_map[key].0))
                .join(""),
        )
        .unwrap();
    }

    log::debug!("Turning AST to HIR…");
    let (hir, hir_ast_id_map) = db.hir(module.clone()).unwrap();
    if debug {
        let hir_file = module.associated_debug_file("hir");
        fs::write(hir_file, format!("{}", hir)).unwrap();

        let hir_ast_id_file = module.associated_debug_file("hir_to_ast_ids");
        fs::write(
            hir_ast_id_file,
            hir_ast_id_map
                .keys()
                .into_iter()
                .map(|key| format!("{key} -> {}\n", hir_ast_id_map[key]))
                .join(""),
        )
        .unwrap();
    }

    log::debug!("Lowering HIR to LIR…");
    let lir = db.lir(module.clone()).unwrap();
    if debug {
        let lir_file = module.associated_debug_file("lir");
        fs::write(lir_file, format!("{lir}")).unwrap();
    }

    let mut errors = vec![];
    hir.collect_errors(&mut errors);
    for CompilerError { span, payload, .. } in errors {
        let (start_line, start_col) = db.offset_to_lsp(module.clone(), span.start);
        let (end_line, end_col) = db.offset_to_lsp(module.clone(), span.end);
        log::warn!("{start_line}:{start_col} – {end_line}:{end_col}: {payload:?}");
    }

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
        log::info!("Build failed.");
        return;
    };
    let module_closure = Closure::of_module(&db, module.clone()).unwrap();

    let path_string = options.file.to_string_lossy();
    log::info!("Running `{path_string}`.");

    let use_provider = DbUseProvider { db: &db };
    let vm = Vm::new_for_running_module_closure(&use_provider, module_closure);
    let TearDownResult {
        tracer,
        result,
        heap,
        ..
    } = vm.run_synchronously_until_completion(&db);

    match result {
        Ok(return_value) => log::info!(
            "The module exports these definitions: {}",
            return_value.format(&heap)
        ),
        Err(reason) => {
            log::error!("The module panicked because {reason}.");
            log::error!("This is the stack trace:");
            tracer.dump_stack_trace(&db, &heap);
        }
    }

    if options.debug {
        let trace = tracer.dump_call_tree();
        let trace_file = module.associated_debug_file("trace");
        fs::write(trace_file.clone(), trace).unwrap();
        log::info!(
            "Trace has been written to `{}`.",
            trace_file.as_path().display()
        );
    }
}

async fn fuzz(options: CandyFuzzOptions) {
    let module = Module::from_package_root_and_file(
        current_dir().unwrap(),
        options.file.clone(),
        ModuleKind::Code,
    );
    log::debug!("Building `{}`.\n", module);

    if raw_build(module.clone(), false).is_none() {
        log::info!("Build failed.");
        return;
    }

    log::debug!("Fuzzing `{module}`.");
    let db = Database::default();
    fuzzer::fuzz(&db, module).await;
}

async fn lsp() {
    log::info!("Starting language server…");
    let (service, socket) = LspService::new(CandyLanguageServer::from_client);
    Server::new(tokio::io::stdin(), tokio::io::stdout(), socket)
        .serve(service)
        .await;
}

fn init_logger() {
    let colors = ColoredLevelConfig::new().debug(Color::BrightBlack);
    fern::Dispatch::new()
        .format(move |out, message, record| {
            let color = colors.get_color(&record.level());
            out.finish(format_args!(
                "\x1B[{}m{} [{:>5}] {} {}\x1B[0m",
                color.to_fg_str(),
                chrono::Local::now().format("%H:%M:%S"),
                record.level(),
                record.target(),
                message
            ))
        })
        .level_for("candy::compiler::hir_to_lir", LevelFilter::Debug)
        .level_for("candy::compiler::string_to_rcst", LevelFilter::Debug)
        .level_for("candy::language_server::hints", LevelFilter::Debug)
        .level_for("candy::vm::builtin_functions", LevelFilter::Warn)
        .level_for("candy::vm::heap", LevelFilter::Debug)
        .level_for("candy::vm::vm", LevelFilter::Info)
        .level_for("lspower::transport", LevelFilter::Error)
        .level_for("salsa", LevelFilter::Error)
        .level_for("tokio_util", LevelFilter::Error)
        .chain(std::io::stderr())
        .apply()
        .unwrap();
}
