#![feature(box_patterns, option_take_if)]
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
use clap::{Parser, ValueHint};
use std::{
    fs,
    path::PathBuf,
    time::{Duration, Instant},
};
use tracing::{debug, error, info, warn, Level, Metadata};
use tracing_subscriber::{
    filter, fmt::format::FmtSpan, layer::SubscriberExt, util::SubscriberInitExt, Layer,
};

mod ast;
mod error;
mod id;
mod position;
mod string_to_ast;

#[derive(Parser, Debug)]
#[command(name = "candy", about = "The 🍭 Candy CLI.")]
enum CandyOptions {
    Check(CheckOptions),
}

fn main() -> ProgramResult {
    let options = CandyOptions::parse();

    init_logger();

    match options {
        CandyOptions::Check(options) => check(options),
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
    /// The file or package to check. If none is provided, the package of your
    /// current working directory will be checked.
    #[arg(value_hint = ValueHint::FilePath)]
    path: PathBuf,
}

#[allow(clippy::needless_pass_by_value)]
fn check(options: CheckOptions) -> ProgramResult {
    let started_at = Instant::now();

    let source = fs::read_to_string(&options.path).unwrap();

    let asts = string_to_ast::string_to_ast(&options.path, &source);

    let errors = asts.collect_errors();
    let has_errors = !errors.is_empty();

    let ended_at = Instant::now();
    debug!("Check took {}.", format_duration(ended_at - started_at));

    if has_errors {
        for error in errors {
            error!("{}", error.to_string_with_location(&source));
        }
        Err(Exit::CodeContainsErrors)
    } else {
        info!("No errors found 🎉");
        Ok(())
    }
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
            "candy_compiler_v3::string_to_ast",
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
