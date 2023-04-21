#![no_main]

use candy_frontend::{
    ast::AstDbStorage,
    ast_to_hir::AstToHirStorage,
    cst::CstDbStorage,
    cst_to_ast::CstToAstStorage,
    hir::HirDbStorage,
    hir_to_mir::HirToMirStorage,
    mir_optimize::OptimizeMirStorage,
    module::{
        InMemoryModuleProvider, Module, ModuleDbStorage, ModuleKind, ModuleProvider,
        ModuleProviderOwner, Package,
    },
    position::PositionConversionStorage,
    rcst_to_cst::RcstToCstStorage,
    string_to_rcst::StringToRcstStorage,
    TracingConfig,
};
use candy_vm::{
    context::DbUseProvider,
    fiber::ExecutionResult,
    heap::Struct,
    mir_to_lir::{MirToLir, MirToLirStorage},
    run_lir, run_main,
    tracer::DummyTracer,
};
use lazy_static::lazy_static;
use libfuzzer_sys::fuzz_target;

const TRACING: TracingConfig = TracingConfig::off();
lazy_static! {
    static ref PACKAGE: Package = Package::User("/".into());
    static ref MODULE: Module = Module {
        package: PACKAGE.clone(),
        path: vec!["fuzzer".to_string()],
        kind: ModuleKind::Code,
    };
}

#[derive()]
#[salsa::database(
    AstDbStorage,
    AstToHirStorage,
    CstDbStorage,
    CstToAstStorage,
    HirDbStorage,
    HirToMirStorage,
    MirToLirStorage,
    ModuleDbStorage,
    OptimizeMirStorage,
    PositionConversionStorage,
    RcstToCstStorage,
    StringToRcstStorage
)]
#[derive(Default)]
pub struct Database {
    storage: salsa::Storage<Self>,
    module_provider: InMemoryModuleProvider,
}
impl salsa::Database for Database {}
impl ModuleProviderOwner for Database {
    fn get_module_provider(&self) -> &dyn ModuleProvider {
        &self.module_provider
    }
}

fuzz_target!(|data: &[u8]| {
    let mut db = Database::default();
    db.module_provider.add(&MODULE, data.to_vec());

    let lir = db.lir(MODULE.clone(), TRACING.clone()).unwrap();

    let use_provider = DbUseProvider {
        db: &db,
        tracing: TRACING.clone(),
    };
    let mut tracer = DummyTracer::default();
    let (mut heap, main) = run_lir(
        MODULE.clone(),
        lir.as_ref().to_owned(),
        &use_provider,
        &mut tracer,
    )
    .into_main_function()
    .unwrap();

    // Run the `main` function.
    let environment = Struct::create(&mut heap, &Default::default());
    match run_main(heap, main, environment, &use_provider, &mut tracer) {
        ExecutionResult::Finished(return_value) => {
            println!("The main function returned: {return_value:?}")
        }
        ExecutionResult::Panicked { reason, .. } => panic!("The main function panicked: {reason}"),
    }
});
