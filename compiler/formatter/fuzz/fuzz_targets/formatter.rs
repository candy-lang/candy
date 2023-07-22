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
use extension_trait::extension_trait;
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
    let old_ast = db.ast(MODULE.clone()).unwrap();

    let formatted_source = old_cst.format_to_string();
    db.module_provider.add_str(&MODULE, &formatted_source);
    GetModuleContentQuery.in_db_mut(&mut db).invalidate(&MODULE);

    let new_cst = db.cst(MODULE.clone()).unwrap();
    assert!(!new_cst.format_to_edits().has_edits());

    let new_ast = db.ast(MODULE.clone()).unwrap();
    assert_eq!(old_ast, new_ast);
});

#[extension_trait]
impl NormalizeAstSpans for Ast {
    fn normalize_spans(&mut self) {
        match &mut self.kind {
            AstKind::Int(_) => todo!(),
            AstKind::Text(Text(parts)) => {
                for part in parts {
                    part.normalize_spans();
                }
            }
            AstKind::TextPart(_) | AstKind::Identifier(_) | AstKind::Symbol(_) => {}
            AstKind::List(List(items)) => {
                for item in items {
                    item.normalize_spans();
                }
            }
            AstKind::Struct(Struct { fields }) => {
                for (key, value) in fields {
                    if let Some(key) = key {
                        key.normalize_spans();
                    }
                    value.normalize_spans();
                }
            }
            AstKind::StructAccess(StructAccess { struct_, key: _ }) => {
                struct_.normalize_spans();
            }
            AstKind::Function(function) => function.normalize_spans(),
            AstKind::Call(Call {
                receiver,
                arguments,
                is_from_pipe: _,
            }) => {
                receiver.normalize_spans();
                for argument in arguments {
                    argument.normalize_spans();
                }
            }
            AstKind::Assignment(Assignment { is_public: _, body }) => match body {
                AssignmentBody::Function { name: _, function } => function.normalize_spans(),
                AssignmentBody::Body { pattern, body } => {
                    pattern.normalize_spans();
                    for ast in body {
                        ast.normalize_spans();
                    }
                }
            },
            AstKind::Match(Match { expression, cases }) => {
                expression.normalize_spans();
                for case in cases {
                    case.normalize_spans();
                }
            }
            AstKind::MatchCase(MatchCase { pattern, body }) => {
                pattern.normalize_spans();
                for ast in body {
                    ast.normalize_spans();
                }
            }
            AstKind::OrPattern(OrPattern(patterns)) => {
                for pattern in patterns {
                    pattern.normalize_spans();
                }
            }
            AstKind::Error { child, errors } => {
                if let Some(child) = child {
                    child.normalize_spans();
                }
                for error in errors {
                    error.span = Offset(0)..Offset(error.span.end.0 - error.span.start.0);
                }
            }
        }
    }
}

#[extension_trait]
impl NormalizeFunctionSpans for Function {
    fn normalize_spans(&mut self) {
        for ast in &mut self.body {
            ast.normalize_spans();
        }
    }
}
