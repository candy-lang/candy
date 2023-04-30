use candy_frontend::{
    ast::AstDbStorage,
    ast_to_hir::AstToHirStorage,
    cst::CstDbStorage,
    cst_to_ast::CstToAstStorage,
    hir::{self, HirDbStorage},
    hir_to_mir::HirToMirStorage,
    mir_optimize::OptimizeMirStorage,
    module::{
        GetModuleContentQuery, InMemoryModuleProvider, Module, ModuleDbStorage, ModuleKind,
        ModuleProvider, ModuleProviderOwner, MutableModuleProviderOwner, Package, PackagesPath,
    },
    position::PositionConversionStorage,
    rcst_to_cst::RcstToCstStorage,
    string_to_rcst::StringToRcstStorage,
    TracingConfig,
};
use candy_vm::{
    channel::Packet,
    fiber::ExecutionResult,
    heap::{HirId, Struct},
    lir::Lir,
    mir_to_lir::compile_lir,
    tracer::DummyTracer,
    vm::Vm,
};
use lazy_static::lazy_static;
use rustc_hash::FxHashMap;
use std::{borrow::Borrow, fs, sync::Arc};
use walkdir::WalkDir;

const TRACING: TracingConfig = TracingConfig::off();
lazy_static! {
    static ref PACKAGE: Package = Package::User("/".into());
    static ref MODULE: Module = Module {
        package: PACKAGE.clone(),
        path: vec!["benchmark".to_string()],
        kind: ModuleKind::Code,
    };
}

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
impl MutableModuleProviderOwner for Database {
    fn get_in_memory_module_provider(&mut self) -> &mut InMemoryModuleProvider {
        &mut self.module_provider
    }
    fn invalidate_module(&mut self, module: &Module) {
        GetModuleContentQuery.in_db_mut(self).invalidate(module);
    }
}

pub fn setup_and_compile(source_code: &str) -> Arc<Lir> {
    let mut db = setup();
    compile(&mut db, source_code)
}

pub fn setup() -> Database {
    let mut db = Database::default();
    load_core(&mut db.module_provider);
    db.module_provider.add_str(&MODULE, r#"_ = use "..Core""#);

    // Load `Core` into the cache.
    let errors = compile_lir(&db, MODULE.clone(), TRACING.clone()).1;
    assert!(errors.is_empty());

    db
}
fn load_core(module_provider: &mut InMemoryModuleProvider) {
    let packages_path = PackagesPath::try_from("../../packages").unwrap();
    let core_path = packages_path.join("Core");
    let package = Package::Managed("Core".into());

    for file in WalkDir::new(&core_path)
        .into_iter()
        .map(|it| it.unwrap())
        .filter(|it| it.file_type().is_file())
        .filter(|it| it.file_name().to_string_lossy().ends_with(".candy"))
    {
        let module = Module::from_package_and_path(
            &packages_path,
            package.clone(),
            file.path(),
            ModuleKind::Code,
        )
        .unwrap();

        let source_code = fs::read_to_string(file.path()).unwrap();
        module_provider.add_str(&module, source_code);
    }
}

pub fn compile(db: &mut Database, source_code: &str) -> Arc<Lir> {
    db.did_open_module(&MODULE, source_code.as_bytes().to_owned());

    compile_lir(db, MODULE.clone(), TRACING.clone()).0
}

pub fn run(lir: impl Borrow<Lir>) -> Packet {
    let mut tracer = DummyTracer::default();
    let (mut heap, main) = Vm::for_module(lir.borrow())
        .run_until_completion(&mut tracer)
        .into_main_function()
        .unwrap();

    // Run the `main` function.
    let environment = Struct::create(&mut heap, &FxHashMap::default());
    let responsible = HirId::create(&mut heap, hir::Id::user());
    match Vm::for_closure(lir, heap, main, &[environment.into()], responsible)
        .run_until_completion(&mut tracer)
    {
        ExecutionResult::Finished(return_value) => return_value,
        ExecutionResult::Panicked { reason, .. } => panic!("The main function panicked: {reason}"),
    }
}
