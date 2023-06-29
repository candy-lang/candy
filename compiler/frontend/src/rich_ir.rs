use crate::{
    ast::Ast,
    builtin_functions::BuiltinFunction,
    hir,
    mir::{self, Mir},
    module::Module,
    position::Offset,
    string_to_rcst::{ModuleError, RcstResult},
    TracingConfig, TracingMode,
};
use colored::{Color, Colorize};
use derive_more::From;
use enumset::{EnumSet, EnumSetType};
use num_bigint::BigInt;
use rustc_hash::FxHashMap;
use std::{
    fmt::{self, Display, Formatter},
    hash::Hash,
    ops::Range,
};

#[derive(Debug, Default)]
pub struct RichIr {
    pub text: String,
    pub annotations: Vec<RichIrAnnotation>,
    pub references: FxHashMap<ReferenceKey, ReferenceCollection>,
    pub folding_ranges: Vec<Range<Offset>>,
}
impl Display for RichIr {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.text)
    }
}

#[derive(Clone, Debug, Eq, From, Hash, PartialEq)]
pub enum ReferenceKey {
    Int(BigInt),
    Text(String),
    #[from(ignore)]
    Symbol(String),
    BuiltinFunction(BuiltinFunction),
    Module(Module),
    #[from(ignore)]
    ModuleWithSpan(Module, Range<Offset>),
    HirId(hir::Id),
    MirId(mir::Id),
}
#[derive(Debug, Default)]
pub struct ReferenceCollection {
    pub definition: Option<Range<Offset>>,
    pub references: Vec<Range<Offset>>,
}

#[derive(Debug)]
pub struct RichIrAnnotation {
    pub range: Range<Offset>,
    pub token_type: Option<TokenType>,
    pub token_modifiers: EnumSet<TokenModifier>,
}

#[derive(Debug, Hash, PartialEq, Eq, Clone, Copy)]
pub enum TokenType {
    Module,
    Type,
    Parameter,
    Variable,
    Symbol,
    Function,
    Comment,
    Text,
    Int,
    Address,
    Constant,
}
#[derive(Debug, EnumSetType)]
pub enum TokenModifier {
    Builtin,
}

pub trait ToRichIr {
    fn to_rich_ir(&self) -> RichIr {
        let mut builder = RichIrBuilder::default();
        self.build_rich_ir(&mut builder);
        builder.finish()
    }
    fn build_rich_ir(&self, builder: &mut RichIrBuilder);
}

impl ToRichIr for str {
    fn build_rich_ir(&self, builder: &mut RichIrBuilder) {
        builder.push(self, None, EnumSet::empty());
    }
}
impl<T: ToRichIr> ToRichIr for Option<T> {
    fn build_rich_ir(&self, builder: &mut RichIrBuilder) {
        if let Some(value) = self {
            value.build_rich_ir(builder);
        }
    }
}
impl<T: ToRichIr> ToRichIr for Box<T> {
    fn build_rich_ir(&self, builder: &mut RichIrBuilder) {
        self.as_ref().build_rich_ir(builder);
    }
}
impl<T: ToRichIr> ToRichIr for [T] {
    fn build_rich_ir(&self, builder: &mut RichIrBuilder) {
        match self {
            [] => {}
            [first, rest @ ..] => {
                first.build_rich_ir(builder);
                for child in rest {
                    builder.push_newline();
                    child.build_rich_ir(builder);
                }
            }
        }
    }
}

#[macro_export]
macro_rules! impl_debug_via_richir {
    ($type:ty) => {
        impl std::fmt::Debug for $type {
            fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(f, "{}", self.to_rich_ir().text)
            }
        }
    };
}
#[macro_export]
macro_rules! impl_display_via_richir {
    ($type:ty) => {
        impl std::fmt::Display for $type {
            fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(f, "{}", self.to_rich_ir().text)
            }
        }
    };
}

#[derive(Debug, Default)]
pub struct RichIrBuilder {
    ir: RichIr,
    indentation: usize,
}
impl RichIrBuilder {
    pub fn push_foldable(&mut self, build_children: impl FnOnce(&mut Self)) {
        let start = self.ir.text.len().into();
        build_children(self);
        let end = self.ir.text.len().into();
        self.ir.folding_ranges.push(start..end);
    }

    pub fn indent(&mut self) {
        self.indentation += 1;
    }
    pub fn dedent(&mut self) {
        self.indentation -= 1;
    }
    pub fn push_newline(&mut self) {
        self.push("\n", None, EnumSet::empty());
        self.push("  ".repeat(self.indentation), None, EnumSet::empty());
    }
    pub fn push_children_multiline<'c, C>(&mut self, children: impl IntoIterator<Item = &'c C>)
    where
        C: ToRichIr + 'c,
    {
        self.push_children_custom_multiline(children, |builder, child| child.build_rich_ir(builder))
    }
    pub fn push_children_custom_multiline<C>(
        &mut self,
        children: impl IntoIterator<Item = C>,
        mut push_child: impl FnMut(&mut Self, &C),
    ) {
        self.indent();
        for child in children {
            self.push_newline();
            push_child(self, &child);
        }
        self.dedent();
    }

    pub fn push_children<C: ToRichIr>(
        &mut self,
        children: impl AsRef<[C]>,
        separator: impl AsRef<str>,
    ) {
        self.push_children_custom(
            children,
            |builder, child| child.build_rich_ir(builder),
            separator,
        )
    }
    pub fn push_children_custom<C>(
        &mut self,
        children: impl AsRef<[C]>,
        mut push_child: impl FnMut(&mut Self, &C),
        separator: impl AsRef<str>,
    ) {
        match children.as_ref() {
            [] => {}
            [child] => push_child(self, child),
            [first, rest @ ..] => {
                push_child(self, first);
                for child in rest {
                    self.push(separator.as_ref(), None, EnumSet::empty());
                    push_child(self, child);
                }
            }
        }
    }

    pub fn push_comment_line(&mut self, text: impl AsRef<str>) {
        let text = text.as_ref();
        if text.is_empty() {
            self.push("#", TokenType::Comment, EnumSet::empty());
        } else {
            self.push("# ", TokenType::Comment, EnumSet::empty());
            self.push(text, TokenType::Comment, EnumSet::empty());
        }
        self.push_newline();
    }

    pub fn push(
        &mut self,
        text: impl AsRef<str>,
        token_type: impl Into<Option<TokenType>>,
        token_modifiers: EnumSet<TokenModifier>,
    ) -> Range<Offset> {
        let token_type = token_type.into();

        assert!(
            token_modifiers.is_empty() || token_type.is_some(),
            "`token_modifiers` can only be specified if a `token_type` is specified.",
        );
        let start = self.ir.text.len().into();
        self.ir.text.push_str(text.as_ref());
        let end = self.ir.text.len().into();
        let range = start..end;
        if token_type.is_some() || !token_modifiers.is_empty() {
            self.ir.annotations.push(RichIrAnnotation {
                range: range.clone(),
                token_type,
                token_modifiers,
            });
        }
        range
    }

    pub fn push_definition(&mut self, key: impl Into<ReferenceKey>, range: Range<Offset>) {
        self.ir
            .references
            .entry(key.into())
            .or_insert_with(ReferenceCollection::default)
            .definition = Some(range);
    }
    pub fn push_reference(&mut self, key: impl Into<ReferenceKey>, range: Range<Offset>) {
        self.ir
            .references
            .entry(key.into())
            .or_insert_with(ReferenceCollection::default)
            .references
            .push(range);
    }

    pub fn push_tracing_config(&mut self, tracing_config: &TracingConfig) {
        fn push_mode(builder: &mut RichIrBuilder, title: &str, mode: &TracingMode) {
            builder.push_comment_line(format!(
                "• {title} {}",
                match mode {
                    TracingMode::Off => "No",
                    TracingMode::OnlyCurrent => "Only for the current module",
                    TracingMode::All => "Yes",
                },
            ));
        }

        self.push_comment_line("");
        self.push_comment_line("Tracing Config:");
        self.push_comment_line("");
        push_mode(
            self,
            "Include tracing of fuzzable functions?",
            &tracing_config.register_fuzzables,
        );
        push_mode(self, "Include tracing of calls?", &tracing_config.calls);
        push_mode(
            self,
            "Include tracing of evaluated expressions?",
            &tracing_config.evaluated_expressions,
        );
    }

    pub fn finish(self) -> RichIr {
        self.ir
    }
}

impl RichIr {
    pub fn for_rcst(module: &Module, rcst: &RcstResult) -> Option<RichIr> {
        let mut builder = RichIrBuilder::default();
        builder.push(
            format!("# RCST for module {module}"),
            TokenType::Comment,
            EnumSet::empty(),
        );
        builder.push_newline();
        match rcst {
            Ok(rcst) => rcst.build_rich_ir(&mut builder),
            Err(ModuleError::DoesNotExist) => return None,
            Err(ModuleError::InvalidUtf8) => {
                builder.push("# Invalid UTF-8", TokenType::Comment, EnumSet::empty());
            }
            Err(ModuleError::IsNotCandy) => {
                builder.push("# Is not Candy code", TokenType::Comment, EnumSet::empty());
            }
            Err(ModuleError::IsToolingModule) => {
                builder.push(
                    "# Is a tooling module",
                    TokenType::Comment,
                    EnumSet::empty(),
                );
            }
        }
        Some(builder.finish())
    }

    pub fn for_ast(module: &Module, asts: &[Ast]) -> RichIr {
        let mut builder = RichIrBuilder::default();
        builder.push(
            format!("# AST for module {module}"),
            TokenType::Comment,
            EnumSet::empty(),
        );
        builder.push_newline();
        asts.build_rich_ir(&mut builder);
        builder.finish()
    }

    pub fn for_hir(module: &Module, body: &hir::Body) -> RichIr {
        let mut builder = RichIrBuilder::default();
        builder.push(
            format!("# HIR for module {module}"),
            TokenType::Comment,
            EnumSet::empty(),
        );
        builder.push_newline();
        body.build_rich_ir(&mut builder);
        builder.finish()
    }

    pub fn for_mir(module: &Module, mir: &Mir, tracing_config: &TracingConfig) -> RichIr {
        let mut builder = RichIrBuilder::default();
        builder.push(
            format!("# MIR for module {module}"),
            TokenType::Comment,
            EnumSet::empty(),
        );
        builder.push_newline();
        builder.push_tracing_config(tracing_config);
        builder.push_newline();
        mir.build_rich_ir(&mut builder);
        builder.finish()
    }

    pub fn for_optimized_mir(module: &Module, mir: &Mir, tracing_config: &TracingConfig) -> RichIr {
        let mut builder = RichIrBuilder::default();
        builder.push(
            format!("# Optimized MIR for module {module}"),
            TokenType::Comment,
            EnumSet::empty(),
        );
        builder.push_newline();
        builder.push_tracing_config(tracing_config);
        builder.push_newline();
        mir.build_rich_ir(&mut builder);
        builder.finish()
    }

    pub fn print_to_console(&self) {
        let bytes = self.text.as_bytes().to_vec();
        let annotations = self.annotations.iter();
        let mut displayed_byte = Offset(0);

        for RichIrAnnotation {
            range, token_type, ..
        } in annotations
        {
            assert!(displayed_byte <= range.start);
            let before_annotation =
                std::str::from_utf8(&bytes[*displayed_byte..*range.start]).unwrap();
            print!("{before_annotation}");

            let in_annotation = std::str::from_utf8(&bytes[*range.start..*range.end]).unwrap();

            if let Some(token_type) = token_type {
                let color = match token_type {
                    TokenType::Module => Color::Yellow,
                    TokenType::Type => Color::BrightMagenta,
                    TokenType::Parameter => Color::Red,
                    TokenType::Variable => Color::Yellow,
                    TokenType::Symbol => Color::Magenta,
                    TokenType::Function => Color::Blue,
                    TokenType::Comment => Color::Green,
                    TokenType::Text => Color::Cyan,
                    TokenType::Int => Color::Red,
                    TokenType::Address => Color::BrightGreen,
                    TokenType::Constant => Color::Yellow,
                };
                print!("{}", in_annotation.color(color));
            } else {
                print!("{}", in_annotation)
            }

            displayed_byte = range.end;
        }
        let rest = std::str::from_utf8(&bytes[*displayed_byte..]).unwrap();
        println!("{rest}");
    }
}
