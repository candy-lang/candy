use crate::Exit;
use candy_frontend::module::{Module, ModuleFromPathError, ModuleKind, PackagesPath};
use std::{
    env::{current_dir, current_exe},
    path::PathBuf,
};
use tracing::error;

pub fn packages_path() -> PackagesPath {
    // We assume the candy executable lives inside the Candy Git repository
    // inside the `$candy/target/` directory.
    let candy_exe = current_exe().unwrap();
    let target_dir = candy_exe
        .ancestors()
        .find(|path| path.ends_with("target"))
        .unwrap();
    let candy_repo = target_dir.parent().unwrap();
    PackagesPath::try_from(candy_repo.join("packages").as_path()).unwrap()
}

pub fn module_for_path(path: impl Into<Option<PathBuf>>) -> Result<Module, Exit> {
    let packages_path = packages_path();
    if let Some(file) = path.into() {
        Module::from_path(&packages_path, &file, ModuleKind::Code).map_err(|error| match error {
            ModuleFromPathError::NotFound(_) => {
                error!("The given file doesn't exist.");
                Exit::FileNotFound
            }
            ModuleFromPathError::NotInPackage(_) => {
                error!("The given file is not in a Candy package.");
                Exit::NotInCandyPackage
            }
        })
    } else {
        let Some(package) = packages_path.find_surrounding_package(&current_dir().unwrap()) else {
            error!("You are not in a Candy package. Either navigate into a package or specify a Candy file.");
            error!("Candy packages are folders that contain a `_package.candy` file. This file marks the root folder of a package. Relative imports can only happen within the package.");
            return Err(Exit::NotInCandyPackage);
        };
        Ok(Module::new(package, vec![], ModuleKind::Code))
    }
}
