#![feature(label_break_value)]
#![feature(let_chains)]
#![feature(never_type)]
#![feature(try_trait_v2)]

mod builtin_functions;
mod compiler;
mod database;
mod discover;
mod fuzzer;
mod input;
mod language_server;
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
    database::{Database, PROJECT_DIRECTORY},
    input::Input,
    language_server::utils::LspPositionConversion,
    vm::{use_provider::DbUseProvider, value::Value, Status, Vm},
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
        CandyOptions::Fuzz(options) => fuzz(options),
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
                Err(e) => log::error!("watch error: {e:#?}"),
            }
        }
    }
}
fn raw_build(file: &PathBuf, debug: bool) -> Option<Arc<Lir>> {
    let path_string = file.to_string_lossy();
    log::info!("Building `{path_string}`.");

    let input: Input = file.clone().into();
    let db = Database::default();

    log::debug!("Parsing string to RCST…");
    let rcst = db
        .rcst(input.clone())
        .unwrap_or_else(|err| panic!("Error parsing file `{}`: {:?}", path_string, err));
    if debug {
        let rcst_file = file.clone_with_extension("candy.rcst");
        fs::write(rcst_file, format!("{:#?}\n", rcst.clone())).unwrap();
    }

    log::debug!("Turning RCST to CST…");
    let cst = db.cst(input.clone()).expect("RCST should have failed");
    if debug {
        let cst_file = file.clone_with_extension("candy.cst");
        fs::write(cst_file, format!("{:#?}\n", cst.clone())).unwrap();
    }

    log::debug!("Abstracting CST to AST…");
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
                .map(|key| format!("{key} -> {}\n", ast_cst_id_map[key].0))
                .join(""),
        )
        .unwrap();
    }

    log::debug!("Turning AST to HIR…");
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
                .map(|key| format!("{key} -> {}\n", hir_ast_id_map[key]))
                .join(""),
        )
        .unwrap();
    }

    log::debug!("Lowering HIR to LIR…");
    let lir = db
        .lir(input.clone())
        .unwrap_or_else(|| panic!("File `{}` not found.", path_string));
    if debug {
        let lir_file = file.clone_with_extension("candy.lir");
        fs::write(lir_file, format!("{lir}")).unwrap();
    }

    let mut errors = vec![];
    hir.collect_errors(&mut errors);
    for CompilerError { span, payload, .. } in errors {
        let (start_line, start_col) = db.offset_to_lsp(input.clone(), span.start);
        let (end_line, end_col) = db.offset_to_lsp(input.clone(), span.end);
        log::warn!("{start_line}:{start_col} – {end_line}:{end_col}: {payload:?}");
    }

    Some(lir)
}

fn run(options: CandyRunOptions) {
    *PROJECT_DIRECTORY.lock().unwrap() = Some(current_dir().unwrap());

    let input: Input = options.file.clone().into();
    let db = Database::default();

    if raw_build(&options.file, false).is_none() {
        log::info!("Build failed.");
        return;
    };
    let module_closure = Value::module_closure_of_input(&db, input.clone()).unwrap();

    let path_string = options.file.to_string_lossy();
    log::info!("Running `{path_string}`.");

    let mut vm = Vm::new();
    let use_provider = DbUseProvider { db: &db };
    vm.set_up_module_closure_execution(&use_provider, module_closure);

    loop {
        vm.run(&use_provider, 10000);
        match vm.status() {
            Status::Running => log::info!("VM is still running."),
            Status::Done => {
                let return_value = vm.tear_down_module_closure_execution();
                log::info!("VM is done. Export map: {return_value}");
                break;
            }
            Status::Panicked(value) => {
                log::error!("VM panicked with value {value}.");
                log::error!("This is the stack trace:");
                vm.tracer.dump_stack_trace(&db, input.clone());
                break;
            }
        }
    }

    if options.debug {
        let trace = vm.tracer.dump_call_tree();
        let trace_file = options.file.clone_with_extension("candy.trace");
        fs::write(trace_file.clone(), trace).unwrap();
        log::info!(
            "Trace has been written to `{}`.",
            trace_file.as_path().display()
        );
    }
}

fn fuzz(options: CandyFuzzOptions) {
    *PROJECT_DIRECTORY.lock().unwrap() = Some(current_dir().unwrap());

    log::debug!("Building `{}`.\n", options.file.display());

    let input: Input = options.file.clone().into();
    let db = Database::default();

    if raw_build(&options.file, false).is_none() {
        log::info!("Build failed.");
        return;
    }

    let path_string = options.file.to_string_lossy();
    log::debug!("Fuzzing `{path_string}`.");

    fuzzer::fuzz(&db, input);
}

async fn lsp() {
    log::info!("Starting language server…");
    let (service, socket) = LspService::new(|client| CandyLanguageServer::from_client(client));
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
