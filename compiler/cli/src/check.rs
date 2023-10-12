use crate::{
    database::Database,
    utils::{module_for_path, packages_path},
    Exit, ProgramResult,
};
use candy_frontend::{ast_to_hir::AstToHir, hir::CollectErrors};
use clap::{arg, Parser, ValueHint};
use std::path::PathBuf;
use tracing::warn;

/// Check a Candy program for obvious errors.
///
/// This command finds very obvious errors in your program. For more extensive
/// error reporting, fuzzing the Candy program is recommended instead.
#[derive(Parser, Debug)]
pub struct Options {
    /// The file or package to check. If none is provided, the package of your
    /// current working directory will be checked.
    #[arg(value_hint = ValueHint::FilePath)]
    path: Option<PathBuf>,
}

pub fn check(options: Options) -> ProgramResult {
    let packages_path = packages_path();
    let db = Database::new_with_file_system_module_provider(packages_path);
    let module = module_for_path(options.path)?;

    // TODO: Once my other PR is merged, update this to get the MIR instead.
    // This will return a tuple containing the MIR and errors, even from
    // imported modules.

    let (hir, _) = db.hir(module).unwrap();
    let mut errors = vec![];
    hir.collect_errors(&mut errors);
    let has_errors = !errors.is_empty();

    for error in errors {
        warn!("{}", error.to_string_with_location(&db));
    }

    if has_errors {
        Err(Exit::CodeContainsErrors)
    } else {
        Ok(())
    }
}
