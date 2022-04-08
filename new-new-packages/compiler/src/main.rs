#![feature(try_trait_v2)]

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
use crate::database::PROJECT_DIRECTORY;
use crate::{database::Database, input::Input};
use itertools::Itertools;
use language_server::CandyLanguageServer;
use log;
use lspower::{LspService, Server};
use notify::{watcher, RecursiveMode, Watcher};
use std::env::current_dir;
use std::fs;
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
    *PROJECT_DIRECTORY.lock().unwrap() = Some(current_dir().unwrap());

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

    let input: Input = file.clone().into();
    let db = Database::default();

    log::info!("Parsing string to RCST…");
    let rcst = db
        .rcst(input.clone())
        .unwrap_or_else(|| panic!("File `{}` not found.", path_string));
    if debug {
        let hir_file = file.clone_with_extension("candy.rcst");
        fs::write(hir_file, format!("{:#?}\n", rcst.clone())).unwrap();
    }

    log::info!("Parsing RCST to CST…");
    let cst = db
        .cst(input.clone())
        .unwrap_or_else(|| panic!("File `{}` not found.", path_string));
    if debug {
        let cst_file = file.clone_with_extension("candy.cst");
        fs::write(cst_file, format!("{:#?}\n", cst.clone())).unwrap();
    }

    log::info!("Lowering CST to AST…");
    let (asts, ast_cst_id_map, errors) = db
        .ast_raw(input.clone())
        .unwrap_or_else(|| panic!("File `{}` not found.", path_string));
    if debug {
        let ast_file = file.clone_with_extension("candy.ast");
        fs::write(ast_file, format!("{:#?}\n", asts.clone())).unwrap();

        let ast_to_cst_ids_file = file.clone_with_extension("candy.cst_to_ast_ids");
        fs::write(
            ast_to_cst_ids_file,
            ast_cst_id_map
                .keys()
                .into_iter()
                .sorted_by_key(|it| it.local)
                .map(|key| format!("{}: {}", key.local, ast_cst_id_map[key].0))
                .join("\n"),
        )
        .unwrap();
    }

    log::info!("Compiling AST to HIR…");
    let (hir, _, errors) = db
        .hir_raw(input.clone())
        .unwrap_or_else(|| panic!("File `{}` not found.", path_string));
    if debug {
        let hir_file = file.clone_with_extension("candy.hir");
        fs::write(hir_file, format!("{:#?}\n", hir.clone())).unwrap();
    }

    // let reports = analyze((*lambda).clone());
    // for report in reports {
    //     log::error!("Report: {:?}", report);
    // }

    Some(hir)
}

fn run(options: CandyRunOptions) {
    *PROJECT_DIRECTORY.lock().unwrap() = Some(current_dir().unwrap());

    log::debug!("Running `{}`.\n", options.file.display());

    let input: Input = options.file.clone().into();
    let db = Database::default();

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

trait CloneWithExtension {
    fn clone_with_extension(&self, extension: &'static str) -> Self;
}
impl CloneWithExtension for PathBuf {
    fn clone_with_extension(&self, extension: &'static str) -> Self {
        let mut path = self.clone();
        assert!(path.set_extension(extension));
        path
    }
}
