use crate::{
    database::Database,
    debug,
    utils::{module_for_path, packages_path},
    Exit, ProgramResult,
};
use clap::{Parser, ValueHint};
use std::path::PathBuf;
use tracing::{error, info};

/// Fuzz a Candy module.
///
/// This command runs the given file or, if no file is provided, the package of
/// your current working directory. It finds all fuzzable functions and then
/// fuzzes them.
///
/// Fuzzable functions are functions written without curly braces.
#[derive(Parser, Debug)]
pub struct Options {
    /// The file or package to fuzz. If none is provided, the package of your
    /// current working directory will be fuzzed.
    #[arg(value_hint = ValueHint::FilePath)]
    path: Option<PathBuf>,
}

pub fn fuzz(options: Options) -> ProgramResult {
    let db = Database::new_with_file_system_module_provider(packages_path());
    let module = module_for_path(options.path)?;

    debug!("Fuzzing `{module}`…");
    let failing_cases = candy_fuzzer::fuzz(&db, module);

    if failing_cases.is_empty() {
        info!("All found fuzzable functions seem fine.");
        Ok(())
    } else {
        error!("");
        error!("Finished fuzzing.");
        error!("These are the failing cases:");
        for case in failing_cases {
            error!("");
            case.dump(&db);
        }
        Err(Exit::FuzzingFoundFailingCases)
    }
}
