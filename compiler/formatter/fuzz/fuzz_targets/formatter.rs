#![no_main]

use candy_formatter::Formatter;
use candy_frontend::{
    ast::{
        Assignment, AssignmentBody, Ast, AstDbStorage, AstKind, Call, Function, List, Match,
        MatchCase, OrPattern, Struct, StructAccess, Text,
    },
    cst::CstDbStorage,
    cst_to_ast::{CstToAst, CstToAstStorage},
    module::{
        GetModuleContentQuery, InMemoryModuleProvider, Module, ModuleDbStorage, ModuleKind,
        ModuleProvider, ModuleProviderOwner, Package,
    },
    position::Offset,
    rcst_to_cst::{RcstToCst, RcstToCstStorage},
    string_to_rcst::StringToRcstStorage,
};
use lazy_static::lazy_static;
use libfuzzer_sys::fuzz_target;

lazy_static! {
    static ref PACKAGE: Package = Package::User("/".into());
    static ref MODULE: Module = Module {
        package: PACKAGE.clone(),
        path: vec!["fuzzer".to_string()],
        kind: ModuleKind::Code,
    };
}

#[salsa::database(
    AstDbStorage,
    CstDbStorage,
    CstToAstStorage,
    ModuleDbStorage,
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

    let Ok(old_cst) = db.cst(MODULE.clone()) else {
        return;
    };
    let (old_ast, _) = db.ast(MODULE.clone()).unwrap();
    let mut old_ast = old_ast.as_ref().to_owned();
    old_ast.normalize_spans();

    let formatted_source = old_cst.format_to_string();
    db.module_provider.add_str(&MODULE, &formatted_source);
    GetModuleContentQuery.in_db_mut(&mut db).invalidate(&MODULE);

    let new_cst = db.cst(MODULE.clone()).unwrap();
    assert!(!new_cst.format_to_edits().has_edits());

    let (new_ast, _) = db.ast(MODULE.clone()).unwrap();
    let mut new_ast = new_ast.as_ref().to_owned();
    new_ast.normalize_spans();
    assert_eq!(old_ast, new_ast);
});

trait NormalizeSpans {
    fn normalize_spans(&mut self);
}
impl<T: NormalizeSpans> NormalizeSpans for [T] {
    fn normalize_spans(&mut self) {
        for item in self {
            item.normalize_spans();
        }
    }
}
impl NormalizeSpans for Ast {
    fn normalize_spans(&mut self) {
        match &mut self.kind {
            AstKind::Int(_) => {}
            AstKind::Text(Text(parts)) => parts.normalize_spans(),
            AstKind::TextPart(_) | AstKind::Identifier(_) | AstKind::Symbol(_) => {}
            AstKind::List(List(items)) => items.normalize_spans(),
            AstKind::Struct(Struct { fields }) => {
                for (key, value) in fields {
                    if let Some(key) = key {
                        key.normalize_spans();
                    }
                    value.normalize_spans();
                }
            }
            AstKind::StructAccess(StructAccess { struct_, key: _ }) => struct_.normalize_spans(),
            AstKind::Function(function) => function.normalize_spans(),
            AstKind::Call(Call {
                receiver,
                arguments,
                is_from_pipe: _,
            }) => {
                receiver.normalize_spans();
                arguments.normalize_spans();
            }
            AstKind::Assignment(Assignment { is_public: _, body }) => match body {
                AssignmentBody::Function { name: _, function } => function.normalize_spans(),
                AssignmentBody::Body { pattern, body } => {
                    pattern.normalize_spans();
                    body.normalize_spans();
                }
            },
            AstKind::Match(Match { expression, cases }) => {
                expression.normalize_spans();
                cases.normalize_spans();
            }
            AstKind::MatchCase(MatchCase { pattern, body }) => {
                pattern.normalize_spans();
                body.normalize_spans();
            }
            AstKind::OrPattern(OrPattern(patterns)) => patterns.normalize_spans(),
            AstKind::Error { errors } => {
                for error in errors {
                    error.span = Offset(0)..Offset(error.span.end.0 - error.span.start.0);
                }
            }
        }
    }
}
impl NormalizeSpans for Function {
    fn normalize_spans(&mut self) {
        self.body.normalize_spans();
    }
}
