mod builtin_functions;
mod compiler;
mod interpreter;
mod language_server;

use crate::compiler::ast_to_hir::CompileVecAstsToHir;
use crate::compiler::cst_to_ast::LowerCstToAst;
use crate::compiler::string_to_cst::StringToCst;
use crate::interpreter::fiber::FiberStatus;
use crate::interpreter::*;
use language_server::CandyLanguageServer;
use log;
use lspower::{LspService, Server};
use simplelog::{
    ColorChoice, CombinedLogger, Config, LevelFilter, TermLogger, TerminalMode, WriteLogger,
};
use std::{fs::File, path::PathBuf};
use structopt::StructOpt;

#[derive(StructOpt, Debug)]
#[structopt(name = "candy", about = "The 🍭 Candy CLI.")]
enum CandyOptions {
    Run(CandyRunOptions),
    Lsp,
}

#[derive(StructOpt, Debug)]
struct CandyRunOptions {
    #[structopt(long)]
    print_cst: bool,

    #[structopt(long)]
    print_ast: bool,

    #[structopt(long)]
    print_hir: bool,

    #[structopt(long)]
    no_run: bool,

    #[structopt(parse(from_os_str))]
    file: PathBuf,
}

#[tokio::main]
async fn main() {
    match CandyOptions::from_args() {
        CandyOptions::Run(options) => run(options),
        CandyOptions::Lsp => lsp().await,
    }
}

fn run(options: CandyRunOptions) {
    init_logger(TerminalMode::Mixed);
    log::debug!("Running `{}`.\n", options.file.to_string_lossy());

    let test_code = std::fs::read_to_string(options.file.clone())
        .unwrap_or_else(|_| panic!("File `{}` not found.", options.file.to_string_lossy()));

    log::info!("Parsing string to CST…");
    let cst = test_code.parse_cst();
    if options.print_cst {
        log::info!("CST: {:#?}", cst);
    }

    log::info!("Lowering CST to AST…");
    let (asts, ast_cst_id_mapping, errors) = cst.clone().into_ast();
    if options.print_ast {
        log::info!("AST: {:#?}", asts);
    }
    if !errors.is_empty() {
        log::error!("Errors occurred while lowering CST to AST:\n{:#?}", errors);
        return;
    }

    log::info!("Compiling AST to HIR…");
    let (lambda, _, errors) = asts.compile_to_hir(cst, ast_cst_id_mapping);
    if options.print_hir {
        log::info!("HIR: {:#?}", lambda);
    }
    if !errors.is_empty() {
        log::error!("Errors occurred while lowering AST to HIR:\n{:#?}", errors);
        return;
    }

    if !options.no_run {
        log::info!("Executing code…");
        let mut fiber = fiber::Fiber::new(lambda);
        fiber.run();
        match fiber.status() {
            FiberStatus::Running => log::info!("Fiber is still running."),
            FiberStatus::Done(value) => log::info!("Fiber is done: {:#?}", value),
            FiberStatus::Panicked(value) => log::error!("Fiber panicked: {:#?}", value),
        }
    }
}

async fn lsp() {
    init_logger(TerminalMode::Stderr);
    log::info!("Starting language server…");
    let (service, messages) = LspService::new(|client| CandyLanguageServer::from_client(client));
    Server::new(tokio::io::stdin(), tokio::io::stdout())
        .interleave(messages)
        .serve(service)
        .await;
}

fn init_logger(terminal_mode: TerminalMode) {
    TermLogger::init(
        LevelFilter::Trace,
        Config::default(),
        terminal_mode,
        ColorChoice::Auto,
    )
    .unwrap();
}
