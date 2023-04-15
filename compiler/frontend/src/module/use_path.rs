use super::module::{Module, ModuleKind};
use std::fmt::Display;

#[derive(Debug)]
pub struct UsePath {
    parent_navigations: usize,
    path: String,
}
impl UsePath {
    const PARENT_NAVIGATION_CHAR: char = '.';

    pub fn parse(mut path: &str) -> Result<Self, String> {
        let parent_navigations = {
            let mut navigations = 0;
            while path.starts_with(UsePath::PARENT_NAVIGATION_CHAR) {
                navigations += 1;
                path = &path[UsePath::PARENT_NAVIGATION_CHAR.len_utf8()..];
            }
            match navigations {
                0 => return Err("The target must start with at least one dot.".to_string()),
                i => i - 1, // two dots means one parent navigation
            }
        };
        let path = {
            if !path.chars().all(|c| c.is_ascii_alphanumeric() || c == '.') {
                return Err("The target name can only contain letters and dots.".to_string());
            }
            path.to_string()
        };
        Ok(UsePath {
            parent_navigations,
            path,
        })
    }

    pub fn resolve_relative_to(&self, current_module: Module) -> Result<Module, String> {
        let kind = if self.path.contains('.') {
            ModuleKind::Asset
        } else {
            ModuleKind::Code
        };

        let mut path = current_module.path;
        for _ in 0..self.parent_navigations {
            if path.pop().is_none() {
                return Err("The path contains too many parent navigations. You can't navigate out of the current package.".to_string());
            }
        }
        path.push(self.path.to_string());

        Ok(Module {
            package: current_module.package,
            path: path.clone(),
            kind,
        })
    }
}
impl Display for UsePath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}{}", ".".repeat(self.parent_navigations), self.path)
    }
}
