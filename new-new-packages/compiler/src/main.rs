mod builtin_functions;
mod compiler;
mod database;
mod discover;
mod incremental;
mod input;
mod language_server;

use crate::compiler::ast_to_hir::AstToHir;
use crate::compiler::cst_to_ast::CstToAst;
use crate::compiler::hir;
use crate::compiler::rcst_to_cst::RcstToCst;
use crate::compiler::string_to_rcst::StringToRcst;
use crate::{database::Database, input::Input};
use itertools::Itertools;
use language_server::CandyLanguageServer;
use log;
use lspower::{LspService, Server};
use notify::{watcher, RecursiveMode, Watcher};
use std::path::PathBuf;
use std::sync::mpsc::channel;
use std::sync::Arc;
use std::time::Duration;
use structopt::StructOpt;

#[derive(StructOpt, Debug)]
#[structopt(name = "candy", about = "The 🍭 Candy CLI.")]
enum CandyOptions {
    Build(CandyBuildOptions),
    Run(CandyRunOptions),
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
    #[structopt(parse(from_os_str))]
    file: PathBuf,
}

#[tokio::main]
async fn main() {
    init_logger();
    match CandyOptions::from_args() {
        CandyOptions::Build(options) => build(options),
        CandyOptions::Run(options) => run(options),
        CandyOptions::Lsp => lsp().await,
    }
}

fn build(options: CandyBuildOptions) {
    raw_build(&options.file, options.debug);

    if options.watch {
        let (tx, rx) = channel();
        let mut watcher = watcher(tx, Duration::from_secs(1)).unwrap();
        watcher
            .watch(&options.file, RecursiveMode::Recursive)
            .unwrap();
        loop {
            match rx.recv() {
                Ok(_) => {
                    raw_build(&options.file, options.debug);
                }
                Err(e) => println!("watch error: {:#?}", e),
            }
        }
    }
}
fn raw_build(file: &PathBuf, debug: bool) -> Option<Arc<hir::Body>> {
    let path_string = file.to_string_lossy();
    log::debug!("Building `{}`.", path_string);

    let input = Input::File(file.to_owned());
    let db = Database::default();

    log::info!("Parsing string to RCST…");
    let rcst = db
        .rcst(input.clone())
        .unwrap_or_else(|| panic!("File `{}` not found.", path_string));
    if debug {
        let mut file = file.clone();
        assert!(file.set_extension("candy.rcst"));
        std::fs::write(file, format!("{:#?}\n", rcst.clone())).unwrap();
    }

    log::info!("Parsing RCST to CST…");
    let cst = db
        .cst(input.clone())
        .unwrap_or_else(|| panic!("File `{}` not found.", path_string));
    if debug {
        let mut file = file.clone();
        assert!(file.set_extension("candy.cst"));
        std::fs::write(file, format!("{:#?}\n", cst.clone())).unwrap();
    }

    log::info!("Lowering CST to AST…");
    let (asts, _, errors) = db
        .ast_raw(input.clone())
        .unwrap_or_else(|| panic!("File `{}` not found.", path_string));
    if debug {
        let mut file = file.clone();
        assert!(file.set_extension("candy.ast"));
        std::fs::write(file, format!("{:#?}\n", asts.clone())).unwrap();
    }
    if !errors.is_empty() {
        log::error!("Errors occurred while lowering CST to AST:\n{:#?}", errors);
        return None;
    }

    log::info!("Compiling AST to HIR…");
    let (hir, _, errors) = db
        .hir_raw(input.clone())
        .unwrap_or_else(|| panic!("File `{}` not found.", path_string));
    if debug {
        let mut file = file.clone();
        assert!(file.set_extension("candy.hir"));
        std::fs::write(file, format!("{:#?}\n", hir.clone())).unwrap();
    }
    if !errors.is_empty() {
        log::error!("Errors occurred while lowering AST to HIR:\n{:#?}", errors);
        return None;
    }

    // let reports = analyze((*lambda).clone());
    // for report in reports {
    //     log::error!("Report: {:?}", report);
    // }

    Some(hir)
}

fn run(options: CandyRunOptions) {
    let _hir = raw_build(&options.file, false);

    let path_string = options.file.to_string_lossy();
    log::debug!("Running `{}`.", path_string);

    // log::info!("Executing code…");
    // let mut fiber = fiber::Fiber::new(hir.as_ref().clone());
    // fiber.run();
    // match fiber.status() {
    //     FiberStatus::Running => log::info!("Fiber is still running."),
    //     FiberStatus::Done(value) => log::info!("Fiber is done: {:#?}", value),
    //     FiberStatus::Panicked(value) => log::error!("Fiber panicked: {:#?}", value),
    // }
}

async fn lsp() {
    log::info!("Starting language server…");
    let (service, messages) = LspService::new(|client| CandyLanguageServer::from_client(client));
    Server::new(tokio::io::stdin(), tokio::io::stdout())
        .interleave(messages)
        .serve(service)
        .await;
}

fn init_logger() {
    fern::Dispatch::new()
        .format(|out, message, record| {
            out.finish(format_args!(
                "{} [{:>5}] {} {}",
                chrono::Local::now().format("%H:%M:%S"),
                record.level(),
                record.target(),
                message
            ))
        })
        .level_for("salsa", log::LevelFilter::Error)
        .level_for("tokio_util", log::LevelFilter::Error)
        .level_for("lspower::transport", log::LevelFilter::Error)
        .chain(std::io::stderr())
        .apply()
        .unwrap();
}
