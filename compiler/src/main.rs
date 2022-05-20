#![feature(try_trait_v2)]
#![feature(let_chains)]

mod builtin_functions;
mod compiler;
mod database;
mod discover;
mod incremental;
mod input;
mod language_server;
mod vm;

use crate::{
    compiler::{
        ast_to_hir::AstToHir, cst::CstDb, cst_to_ast::CstToAst, hir, hir_to_lir::HirToLir,
        rcst_to_cst::RcstToCst, string_to_rcst::StringToRcst,
    },
    database::{Database, PROJECT_DIRECTORY},
    input::Input,
    language_server::utils::LspPositionConversion,
    vm::{Status, Vm},
};
use compiler::lir::Lir;
use itertools::Itertools;
use language_server::CandyLanguageServer;
use log::{debug, error, info, LevelFilter};
use lspower::{LspService, Server};
use notify::{watcher, RecursiveMode, Watcher};
use std::{
    env::current_dir,
    fs,
    path::PathBuf,
    sync::{mpsc::channel, Arc},
    time::Duration,
};
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
                Err(e) => error!("watch error: {:#?}", e),
            }
        }
    }
}
fn raw_build(file: &PathBuf, debug: bool) -> Option<Arc<Lir>> {
    let path_string = file.to_string_lossy();
    debug!("Building `{}`.", path_string);

    let input: Input = file.clone().into();
    let db = Database::default();

    info!("Parsing string to RCST…");
    let rcst = db
        .rcst(input.clone())
        .unwrap_or_else(|| panic!("File `{}` not found.", path_string));
    if debug {
        let hir_file = file.clone_with_extension("candy.rcst");
        fs::write(hir_file, format!("{:#?}\n", rcst.clone())).unwrap();
    }

    info!("Parsing RCST to CST…");
    let cst = db
        .cst(input.clone())
        .unwrap_or_else(|| panic!("File `{}` not found.", path_string));
    if debug {
        let cst_file = file.clone_with_extension("candy.cst");
        fs::write(cst_file, format!("{:#?}\n", cst.clone())).unwrap();
    }

    info!("Lowering CST to AST…");
    let (asts, ast_cst_id_map) = db
        .ast(input.clone())
        .unwrap_or_else(|| panic!("File `{}` not found.", path_string));
    if debug {
        let ast_file = file.clone_with_extension("candy.ast");
        fs::write(
            ast_file,
            format!("{}\n", asts.iter().map(|ast| format!("{}", ast)).join("\n")),
        )
        .unwrap();

        let ast_to_cst_ids_file = file.clone_with_extension("candy.ast_to_cst_ids");
        fs::write(
            ast_to_cst_ids_file,
            ast_cst_id_map
                .keys()
                .into_iter()
                .sorted_by_key(|it| it.local)
                .map(|key| format!("{} -> {}\n", key, ast_cst_id_map[key].0))
                .join(""),
        )
        .unwrap();
    }

    info!("Compiling AST to HIR…");
    let (hir, hir_ast_id_map) = db
        .hir(input.clone())
        .unwrap_or_else(|| panic!("File `{}` not found.", path_string));
    if debug {
        let hir_file = file.clone_with_extension("candy.hir");
        fs::write(hir_file, format!("{}", hir.clone())).unwrap();

        let hir_ast_id_file = file.clone_with_extension("candy.hir_to_ast_ids");
        fs::write(
            hir_ast_id_file,
            hir_ast_id_map
                .keys()
                .into_iter()
                .map(|key| format!("{} -> {}\n", key, hir_ast_id_map[key]))
                .join(""),
        )
        .unwrap();
    }

    info!("Compiling HIR to LIR…");
    let lir = db
        .lir(input.clone())
        .unwrap_or_else(|| panic!("File `{}` not found.", path_string));
    if debug {
        let lir_file = file.clone_with_extension("candy.lir");
        fs::write(lir_file, format!("{}", lir)).unwrap();
    }

    Some(lir)
}

fn run(options: CandyRunOptions) {
    *PROJECT_DIRECTORY.lock().unwrap() = Some(current_dir().unwrap());

    debug!("Running `{}`.\n", options.file.display());

    let input: Input = options.file.clone().into();
    let db = Database::default();

    let lir = match raw_build(&options.file, false) {
        Some(lir) => lir,
        None => {
            log::info!("Build failed.");
            return;
        }
    };

    let path_string = options.file.to_string_lossy();
    debug!("Running `{}`.", path_string);

    let mut vm = Vm::new(lir.chunks.clone());
    vm.run(1000);
    match vm.status() {
        Status::Running => info!("VM is still running."),
        Status::Done(value) => info!("VM is done: {:#?}", value),
        Status::Panicked(value) => {
            error!("VM panicked: {:#?}", value);

            error!("Stack trace:");
            let (_, hir_to_ast_ids) = db.hir(input.clone()).unwrap();
            let (_, ast_to_cst_ids) = db.ast(input.clone()).unwrap();
            for hir_id in vm.current_stack_trace().into_iter().rev() {
                let ast_id = hir_to_ast_ids[&hir_id].clone();
                let cst_id = ast_to_cst_ids[&ast_id];
                let cst = db.find_cst(input.clone(), cst_id);
                let start = db.offset_to_lsp(input.clone(), cst.span.start);
                let end = db.offset_to_lsp(input.clone(), cst.span.end);
                error!(
                    "{}, {}, {:?}, {}:{} – {}:{}",
                    hir_id, ast_id, cst_id, start.0, start.1, end.0, end.1
                );
            }
        }
    }
}

async fn lsp() {
    info!("Starting language server…");
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
        .level_for("salsa", LevelFilter::Error)
        .level_for("tokio_util", LevelFilter::Error)
        .level_for("lspower::transport", LevelFilter::Error)
        .level_for("candy::compiler::string_to_rcst", LevelFilter::Debug)
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
