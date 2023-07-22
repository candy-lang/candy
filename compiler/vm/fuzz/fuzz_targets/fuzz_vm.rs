#![no_main]

use candy_frontend::{
    ast::AstDbStorage,
    ast_to_hir::AstToHirStorage,
    cst::CstDbStorage,
    cst_to_ast::CstToAstStorage,
    hir::{self, HirDbStorage},
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
    fiber::EndedReason,
    heap::{HirId, Struct},
    mir_to_lir::compile_lir,
    tracer::DummyTracer,
    vm::Vm,
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

    let lir = compile_lir(&db, MODULE.clone(), TRACING.clone()).0;

    let result = Vm::for_module(&lir, &mut DummyTracer).run_until_completion(&mut DummyTracer);

    let Ok((mut heap, main)) = result.into_main_function() else {
        println!("The module doesn't export a main function.");
        return;
    };

    // Run the `main` function.
    let environment = Struct::create(&mut heap, true, &Default::default());
    let responsible = HirId::create(&mut heap, true, hir::Id::user());
    match Vm::for_function(
        &lir,
        heap,
        main,
        &[environment.into()],
        responsible,
        &mut DummyTracer,
    )
    .run_until_completion(&mut DummyTracer)
    .reason
    {
        EndedReason::Finished(return_value) => {
            println!("The main function returned: {return_value:?}")
        }
        EndedReason::Panicked(panic) => {
            panic!("The main function panicked: {}", panic.reason)
        }
    }
});
