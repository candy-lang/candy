use crate::{
    database::Database,
    debug,
    utils::{module_for_path, packages_path},
    CandyFuzzOptions, Exit, ProgramResult,
};
use candy_frontend::rich_ir::ToRichIr;
use tracing::{error, info};

pub(crate) fn fuzz(options: CandyFuzzOptions) -> ProgramResult {
    let db = Database::new_with_file_system_module_provider(packages_path());
    let module = module_for_path(options.path)?;

    debug!("Fuzzing `{}`…", module.to_rich_ir());
    let failing_cases = candy_fuzzer::fuzz(&db, module);

    if failing_cases.is_empty() {
        info!("All found fuzzable closures seem fine.");
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
