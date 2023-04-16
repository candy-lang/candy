use super::{
    module::{Module, ModuleKind},
    Package,
};
use std::fmt::Display;

#[derive(Debug)]
pub enum UsePath {
    Managed(String),
    Relative {
        parent_navigations: usize,
        path: String,
    },
}
impl UsePath {
    const PARENT_NAVIGATION_CHAR: char = '.';

    pub fn parse(mut path: &str) -> Result<Self, String> {
        let mut dots = 0;
        while path.starts_with(UsePath::PARENT_NAVIGATION_CHAR) {
            dots += 1;
            path = &path[UsePath::PARENT_NAVIGATION_CHAR.len_utf8()..];
        }
        let path = {
            if !path.chars().all(|c| c.is_ascii_alphanumeric() || c == '.') {
                return Err("The target name can only contain letters and dots.".to_string());
            }
            path.to_string()
        };

        Ok(match dots {
            0 => UsePath::Managed(path),
            i => UsePath::Relative {
                parent_navigations: i - 1, // two dots means one parent navigation
                path,
            },
        })
    }

    pub fn resolve_relative_to(&self, current_module: Module) -> Result<Module, String> {
        Ok(match self {
            UsePath::Managed(name) => Module {
                package: Package::Managed(name.into()),
                path: vec![],
                kind: ModuleKind::Code,
            },
            UsePath::Relative {
                parent_navigations,
                path,
            } => {
                let kind = if path.contains('.') {
                    ModuleKind::Asset
                } else {
                    ModuleKind::Code
                };

                let mut total_path = current_module.path;
                for _ in 0..*parent_navigations {
                    if total_path.pop().is_none() {
                        return Err("The path contains too many parent navigations. You can't navigate out of the current package.".to_string());
                    }
                }
                total_path.push(path.to_string());

                Module {
                    package: current_module.package,
                    path: total_path,
                    kind,
                }
            }
        })
    }
}
impl Display for UsePath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UsePath::Managed(name) => write!(f, "{}", name),
            UsePath::Relative {
                parent_navigations,
                path,
            } => write!(f, "{}{}", ".".repeat(1 + parent_navigations), path),
        }
    }
}
