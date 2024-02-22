//! All parsers take an input and return an input that may have advanced a
//! little.
//!
//! Note: The parser is indentation-first. Indentation is more important than
//! parentheses, brackets, etc. If some part of a definition can't be parsed,
//! all the surrounding code still has a chance to be properly parsed – even
//! mid-writing after putting the opening bracket of a struct.

mod body;
mod expression;
mod function;
mod int;
mod list;
mod literal;
mod struct_;
mod text;
mod utils;
mod whitespace;
mod word;

use crate::{
    cst::{CstError, CstKind},
    module::{Module, ModuleDb, ModuleKind, Package},
    rcst::Rcst,
    rich_ir::{RichIrBuilder, ToRichIr, TokenType},
};
use enumset::EnumSet;
use std::{str, sync::Arc};

#[salsa::query_group(StringToRcstStorage)]
pub trait StringToRcst: ModuleDb {
    fn rcst(&self, module: Module) -> RcstResult;
}

pub type RcstResult = Result<Arc<Vec<Rcst>>, ModuleError>;

fn rcst(db: &dyn StringToRcst, module: Module) -> RcstResult {
    if module.kind != ModuleKind::Code {
        return Err(ModuleError::IsNotCandy);
    }

    if let Package::Tooling(_) = &module.package {
        return Err(ModuleError::IsToolingModule);
    }
    let source = db
        .get_module_content(module)
        .ok_or(ModuleError::DoesNotExist)?;
    let Ok(source) = str::from_utf8(source.as_slice()) else {
        return Err(ModuleError::InvalidUtf8);
    };
    Ok(Arc::new(parse_rcst(source)))
}
#[must_use]
pub fn parse_rcst(source: &str) -> Vec<Rcst> {
    let (mut rest, mut rcsts) = body::body(source, 0);
    if !rest.is_empty() {
        let trailing_newline = if rest.ends_with("\r\n") {
            let (_, newline) = literal::newline(&rest[rest.len() - 2..]).unwrap();
            rest = &rest[..rest.len() - 2];
            Some(newline)
        } else if rest.ends_with('\n') {
            let (_, newline) = literal::newline(&rest[rest.len() - 1..]).unwrap();
            rest = &rest[..rest.len() - 1];
            Some(newline)
        } else {
            None
        };
        rcsts.push(
            CstKind::Error {
                unparsable_input: rest.to_string(),
                error: CstError::UnparsedRest,
            }
            .into(),
        );
        if let Some(trailing_newline) = trailing_newline {
            rcsts.push(trailing_newline);
        }
    }
    rcsts
}

#[derive(PartialEq, Eq, Debug, Clone, Copy, Hash)]
pub enum ModuleError {
    DoesNotExist,
    InvalidUtf8,
    IsNotCandy,
    IsToolingModule,
}
impl ToRichIr for ModuleError {
    fn build_rich_ir(&self, builder: &mut RichIrBuilder) {
        let text = match self {
            Self::DoesNotExist => return,
            Self::InvalidUtf8 => "# Invalid UTF-8",
            Self::IsNotCandy => "# Is not Candy code",
            Self::IsToolingModule => "# Is a tooling module",
        };
        builder.push(text, TokenType::Comment, EnumSet::empty());
    }
}
