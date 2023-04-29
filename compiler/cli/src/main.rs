mod check;
mod database;
mod debug;
mod fuzz;
mod lsp;
mod run;
mod services;
mod utils;

use candy_frontend::{TracingConfig, TracingMode};
use clap::{Parser, ValueHint};
use std::path::PathBuf;
use tracing::{debug, Level, Metadata};
use tracing_subscriber::{
    filter,
    fmt::{format::FmtSpan, writer::BoxMakeWriter},
    prelude::*,
};

#[derive(Parser, Debug)]
#[command(name = "candy", about = "The 🍭 Candy CLI.")]
enum CandyOptions {
    Run(CandyRunOptions),

    Check(CandyCheckOptions),

    Fuzz(CandyFuzzOptions),

    #[command(subcommand)]
    Debug(CandyDebugOptions),

    /// Start a Language Server.
    Lsp,
}

/// Debug the Candy compiler itself.
///
/// This command compiles the given file and outputs its intermediate
/// representation.
#[derive(Parser, Debug)]
enum CandyDebugOptions {
    /// Concrete Syntax Tree
    Cst(CandyDebugPath),

    /// Abstract Syntax Tree
    Ast(CandyDebugPath),

    /// High-Level Intermediate Representation
    Hir(CandyDebugPath),

    /// Mid-Level Intermediate Representation
    Mir(CandyDebugPathAndTracing),

    /// Optimized Mid-Level Intermediate Representation
    OptimizedMir(CandyDebugPathAndTracing),

    /// Low-Level Intermediate Representation
    Lir(CandyDebugPathAndTracing),
}
#[derive(Parser, Debug)]
struct CandyDebugPath {
    path: PathBuf,
}
#[derive(Parser, Debug)]
struct CandyDebugPathAndTracing {
    path: PathBuf,

    #[arg(long)]
    register_fuzzables: bool,

    #[arg(long)]
    trace_calls: bool,

    #[arg(long)]
    trace_evaluated_expressions: bool,
}
impl CandyDebugPathAndTracing {
    fn to_tracing_config(&self) -> TracingConfig {
        TracingConfig {
            register_fuzzables: TracingMode::only_current_or_off(self.register_fuzzables),
            calls: TracingMode::only_current_or_off(self.trace_calls),
            evaluated_expressions: TracingMode::only_current_or_off(
                self.trace_evaluated_expressions,
            ),
        }
    }
}

/// Check a Candy program for obvious errors.
///
/// This command finds very obvious errors in your program. For more extensive
/// error reporting, fuzzing the Candy program is recommended instead.
#[derive(Parser, Debug)]
struct CandyCheckOptions {
    /// The file or package to check. If none is provided, the package of your
    /// current working directory will be checked.
    #[arg(value_hint = ValueHint::FilePath)]
    path: Option<PathBuf>,
}

/// Run a Candy program.
///
/// This command runs the given file, or, if no file is provided, the package of
/// your current working directory. The module should export a `main` function.
/// This function is then called with an environment.
#[derive(Parser, Debug)]
struct CandyRunOptions {
    /// The file or package to run. If none is provided, the package of your
    /// current working directory will be run.
    #[arg(value_hint = ValueHint::FilePath)]
    path: Option<PathBuf>,
}

/// Fuzz a Candy module.
///
/// This command runs the given file or, if no file is provided, the package of
/// your current working directory. It finds all fuzzable functions and then
/// fuzzes them.
///
/// Fuzzable functions are functions written without curly braces.
#[derive(Parser, Debug)]
struct CandyFuzzOptions {
    /// The file or package to fuzz. If none is provided, the package of your
    /// current working directory will be fuzzed.
    #[arg(value_hint = ValueHint::FilePath)]
    path: Option<PathBuf>,
}

#[tokio::main]
async fn main() -> ProgramResult {
    let options = CandyOptions::parse();

    let should_log_to_stdout = !matches!(options, CandyOptions::Lsp);
    init_logger(should_log_to_stdout);

    match options {
        CandyOptions::Debug(options) => debug::debug(options),
        CandyOptions::Check(options) => check::check(options),
        CandyOptions::Run(options) => run::run(options),
        CandyOptions::Fuzz(options) => fuzz::fuzz(options),
        CandyOptions::Lsp => lsp::lsp().await,
    }
}

type ProgramResult = Result<(), Exit>;
#[derive(Debug)]
enum Exit {
    CodePanicked,
    FileNotFound,
    FuzzingFoundFailingCases,
    NotInCandyPackage,
    CodeContainsErrors,
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
            "candy_frontend::mir_optimize",
            Level::INFO,
        )))
        .with_filter(filter::filter_fn(level_for(
            "candy_frontend::string_to_rcst",
            Level::WARN,
        )))
        .with_filter(filter::filter_fn(level_for("candy_frontend", Level::DEBUG)))
        .with_filter(filter::filter_fn(level_for("candy_fuzzer", Level::DEBUG)))
        .with_filter(filter::filter_fn(level_for(
            "candy_language_server",
            Level::TRACE,
        )))
        .with_filter(filter::filter_fn(level_for("candy_vm", Level::DEBUG)))
        .with_filter(filter::filter_fn(level_for("candy_vm::heap", Level::DEBUG)));
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
