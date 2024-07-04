#![feature(box_patterns, option_take_if, try_blocks)]
#![warn(clippy::nursery, clippy::pedantic, unused_crate_dependencies)]
#![allow(
    clippy::cognitive_complexity,
    clippy::match_same_arms,
    clippy::missing_errors_doc,
    clippy::missing_panics_doc,
    clippy::module_name_repetitions,
    clippy::similar_names,
    clippy::too_many_lines
)]

use ast::CollectAstErrors;
use ast_to_hir::ast_to_hir;
use clap::{Parser, ValueHint};
use error::CompilerError;
use hir::Hir;
use hir_to_c::hir_to_c;
use std::{
    fs,
    path::{self, Path, PathBuf},
    process,
    time::{Duration, Instant},
};
use tracing::{debug, error, info, warn, Level, Metadata};
use tracing_subscriber::{
    filter, fmt::format::FmtSpan, layer::SubscriberExt, util::SubscriberInitExt, Layer,
};

mod ast;
mod ast_to_hir;
mod error;
mod hir;
mod hir_to_c;
mod id;
mod position;
mod string_to_ast;
mod utils;

#[derive(Parser, Debug)]
#[command(name = "candy", about = "The 🍭 Candy CLI.")]
enum CandyOptions {
    Check(CheckOptions),
    Compile(CompileOptions),
}

fn main() -> ProgramResult {
    let options = CandyOptions::parse();

    init_logger();

    match options {
        CandyOptions::Check(options) => check(options),
        CandyOptions::Compile(options) => compile(options),
    }
}
pub type ProgramResult = Result<(), Exit>;
#[derive(Debug)]
pub enum Exit {
    FileNotFound,
    CodeContainsErrors,
}

#[derive(Parser, Debug)]
struct CheckOptions {
    /// The file or package to check.
    #[arg(value_hint = ValueHint::FilePath)]
    path: PathBuf,
}

#[allow(clippy::needless_pass_by_value)]
fn check(options: CheckOptions) -> ProgramResult {
    let source = fs::read_to_string(&options.path).unwrap();

    let started_at = Instant::now();
    let (_, errors) = compile_hir(&options.path, &source);
    debug!("Check took {}.", format_duration(started_at.elapsed()));

    if errors.is_empty() {
        info!("No errors found 🎉");
        Ok(())
    } else {
        for error in errors {
            error!("{}", error.to_string_with_location(&source));
        }
        Err(Exit::CodeContainsErrors)
    }
}

#[derive(Parser, Debug)]
struct CompileOptions {
    /// The file or package to compile to C.
    #[arg(value_hint = ValueHint::FilePath)]
    path: PathBuf,
}

#[allow(clippy::needless_pass_by_value)]
fn compile(options: CompileOptions) -> ProgramResult {
    let source = fs::read_to_string(&options.path).unwrap();

    let started_at = Instant::now();
    let (hir, errors) = compile_hir(&options.path, &source);

    if !errors.is_empty() {
        for error in errors {
            error!("{}", error.to_string_with_location(&source));
        }
        return Err(Exit::CodeContainsErrors);
    }

    let c_code = hir_to_c(&hir);
    debug!(
        "Compilation to C took {}.",
        format_duration(started_at.elapsed())
    );

    let c_path = options.path.with_extension("c");
    fs::write(&c_path, c_code).unwrap();
    process::Command::new("clang-format")
        .args(["-i", c_path.to_str().unwrap()])
        .spawn()
        .unwrap()
        .wait()
        .unwrap();

    let executable_path = options.path.with_extension("");
    process::Command::new("gcc")
        .args([
            c_path.to_str().unwrap(),
            "-o",
            executable_path.to_str().unwrap(),
        ])
        .spawn()
        .unwrap()
        .wait()
        .unwrap();

    info!("Done 🎉");
    Ok(())
}

fn compile_hir(path: &Path, source: &str) -> (Hir, Vec<CompilerError>) {
    let asts = string_to_ast::string_to_ast(path, source);
    let mut errors = asts.collect_errors();

    let (hir, mut hir_errors) = ast_to_hir(path, &asts);
    errors.append(&mut hir_errors);

    (hir, errors)
}

fn format_duration(duration: Duration) -> String {
    if duration < Duration::from_millis(1) {
        format!("{} µs", duration.as_micros())
    } else {
        format!("{} ms", duration.as_millis())
    }
}

fn init_logger() {
    let console_log = tracing_subscriber::fmt::layer()
        .compact()
        .with_writer(std::io::stderr)
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
            "candy_v3::string_to_ast",
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
