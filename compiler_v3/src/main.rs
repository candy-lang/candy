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

use clap::{Parser, ValueHint};
use cst::CollectCstErrors;
use rcst_to_cst::rcst_to_cst;
use std::{fs, path::PathBuf};
use tracing::{error, info, warn, Level, Metadata};
use tracing_subscriber::{
    filter, fmt::format::FmtSpan, layer::SubscriberExt, util::SubscriberInitExt, Layer,
};

mod cst;
mod error;
mod id;
mod position;
mod rcst;
mod rcst_to_cst;
mod string_to_rcst;

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
    let source = fs::read_to_string(&options.path).unwrap();

    let rcsts = string_to_rcst::string_to_rcst(&source);
    let csts = rcst_to_cst(&rcsts);

    let errors = csts.collect_errors(&options.path);
    let has_errors = !errors.is_empty();

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
            "candy_compiler_v3::string_to_rcst",
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
