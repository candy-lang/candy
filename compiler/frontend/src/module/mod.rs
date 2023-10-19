pub use self::{
    module::{Module, ModuleFromPathError, ModuleKind},
    module_provider::{
        FileSystemModuleProvider, InMemoryModuleProvider, ModuleProvider, OverlayModuleProvider,
    },
    module_provider_owner::{ModuleProviderOwner, MutableModuleProviderOwner},
    package::{Package, PackagesPath},
    use_path::UsePath,
};
use salsa::query_group;
use std::sync::Arc;

#[allow(clippy::module_inception)]
mod module;
mod module_provider;
mod module_provider_owner;
mod package;
mod use_path;

#[query_group(ModuleDbStorage)]
pub trait ModuleDb: ModuleProviderOwner {
    fn get_module_content_as_string(&self, module: Module) -> Option<Arc<String>>;
    fn get_module_content(&self, module: Module) -> Option<Arc<Vec<u8>>>;
}

fn get_module_content_as_string(db: &dyn ModuleDb, module: Module) -> Option<Arc<String>> {
    let content = get_module_content(db, module)?;
    String::from_utf8((*content).clone()).ok().map(Arc::new)
}

#[allow(clippy::needless_pass_by_value)]
fn get_module_content(db: &dyn ModuleDb, module: Module) -> Option<Arc<Vec<u8>>> {
    // The following line of code shouldn't be neccessary, but it is.
    //
    // We call `GetModuleContentQuery.in_db_mut(self).invalidate(module);`
    // in `Database.did_open_module(…)`, `.did_change_module(…)`, and
    // `.did_close_module(…)` which correctly forces Salsa to re-run this query
    // function the next time this module is used. However, even though the
    // return value changes, Salsa doesn't record an updated `changed_at` value
    // in its internal `ActiveQuery` struct. `Runtime.report_untracked_read()`
    // manually sets this to the current revision.
    db.salsa_runtime().report_untracked_read();

    db.get_module_provider().get_content(&module)
}
