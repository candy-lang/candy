use crate::{
    ast::Ast,
    builtin_functions::BuiltinFunction,
    hir,
    lir::{self, Lir},
    mir::{self, Mir},
    module::Module,
    position::Offset,
    rcst_to_cst::CstResult,
    string_to_rcst::{ModuleError, RcstResult},
    tracing::CallTracingMode,
    TracingConfig, TracingMode,
};
use derive_more::From;
use enumset::{EnumSet, EnumSetType};
use itertools::Itertools;
use num_bigint::{BigInt, BigUint};
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
    LirId(lir::Id),
    LirConstantId(lir::ConstantId),
    LirBodyId(lir::BodyId),
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
    #[must_use]
    fn to_rich_ir(&self, trailing_newline: bool) -> RichIr {
        let mut builder = RichIrBuilder::default();
        self.build_rich_ir(&mut builder);
        builder.finish(trailing_newline)
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
        builder.push_multiline(self);
    }
}
impl ToRichIr for BigInt {
    fn build_rich_ir(&self, builder: &mut RichIrBuilder) {
        let range = builder.push(self.to_string(), TokenType::Int, EnumSet::empty());
        builder.push_reference(self.clone(), range);
    }
}
impl ToRichIr for BigUint {
    fn build_rich_ir(&self, builder: &mut RichIrBuilder) {
        let range = builder.push(self.to_string(), TokenType::Int, EnumSet::empty());
        builder.push_reference(BigInt::from(self.clone()), range);
    }
}

#[macro_export]
macro_rules! impl_debug_via_richir {
    ($type:ty) => {
        impl std::fmt::Debug for $type {
            fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(f, "{}", self.to_rich_ir(false).text)
            }
        }
    };
}
#[macro_export]
macro_rules! impl_display_via_richir {
    ($type:ty) => {
        impl std::fmt::Display for $type {
            fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(f, "{}", self.to_rich_ir(false).text)
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
    pub fn push_indented_foldable(&mut self, build_children: impl FnOnce(&mut Self)) {
        self.push_indented(|builder| builder.push_foldable(build_children));
    }
    pub fn push_foldable(&mut self, build_children: impl FnOnce(&mut Self)) {
        let start = self.ir.text.len().into();
        build_children(self);
        let end = self.ir.text.len().into();
        self.ir.folding_ranges.push(start..end);
    }

    pub fn push_indented(&mut self, build_children: impl FnOnce(&mut Self)) {
        self.indent();
        build_children(self);
        self.dedent();
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
        self.push_children_custom_multiline(children, |builder, child| {
            child.build_rich_ir(builder);
        });
    }
    pub fn push_children_custom_multiline<C>(
        &mut self,
        children: impl IntoIterator<Item = C>,
        push_child: impl FnMut(&mut Self, &C),
    ) {
        self.push_indented(|builder| {
            builder.push_newline();
            builder.push_custom_multiline(children, push_child);
        });
    }
    pub fn push_multiline<'c, C>(&mut self, items: impl IntoIterator<Item = &'c C>)
    where
        C: ToRichIr + 'c,
    {
        self.push_custom_multiline(items, |builder, item| item.build_rich_ir(builder));
    }
    pub fn push_custom_multiline<C>(
        &mut self,
        items: impl IntoIterator<Item = C>,
        mut push_item: impl FnMut(&mut Self, &C),
    ) {
        for (index, item) in items.into_iter().enumerate() {
            if index > 0 {
                self.push_newline();
            }
            push_item(self, &item);
        }
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
        );
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
        self.ir.references.entry(key.into()).or_default().definition = Some(range);
    }
    pub fn push_reference(&mut self, key: impl Into<ReferenceKey>, range: Range<Offset>) {
        self.ir
            .references
            .entry(key.into())
            .or_default()
            .references
            .push(range);
    }

    pub fn push_tracing_config(&mut self, tracing_config: TracingConfig) {
        fn push_mode(builder: &mut RichIrBuilder, title: &str, mode: TracingMode) {
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
            tracing_config.register_fuzzables,
        );
        self.push_comment_line(format!(
            "• Include tracing of calls? {}",
            match tracing_config.calls {
                CallTracingMode::Off => "No",
                CallTracingMode::OnlyCurrent => "Only for the current module",
                CallTracingMode::OnlyForPanicTraces => "Only for panic traces",
                CallTracingMode::All => "Yes",
            },
        ));
        push_mode(
            self,
            "Include tracing of evaluated expressions?",
            tracing_config.evaluated_expressions,
        );
    }

    #[must_use]
    pub fn finish(mut self, trailing_newline: bool) -> RichIr {
        if trailing_newline && !self.ir.text.is_empty() && !self.ir.text.ends_with('\n') {
            self.push("\n", None, EnumSet::empty());
        }
        self.ir
    }
}

impl RichIr {
    #[must_use]
    pub fn for_rcst(module: &Module, rcst: &RcstResult) -> Option<Self> {
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
            Err(error) => error.build_rich_ir(&mut builder),
        }
        Some(builder.finish(true))
    }
    #[must_use]
    pub fn for_cst(module: &Module, cst: &CstResult) -> Option<Self> {
        if cst == &Err(ModuleError::DoesNotExist) {
            return None;
        }

        Some(Self::for_ir("CST", module, None, |builder| {
            match cst {
                Ok(cst) => {
                    // TODO: `impl ToRichIr for Cst`
                    builder.push(
                        cst.iter().map(ToString::to_string).join(""),
                        None,
                        EnumSet::empty(),
                    );
                }
                Err(error) => error.build_rich_ir(builder),
            };
        }))
    }
    #[must_use]
    pub fn for_ast(module: &Module, asts: &[Ast]) -> Self {
        Self::for_ir("AST", module, None, |builder| asts.build_rich_ir(builder))
    }
    #[must_use]
    pub fn for_hir(module: &Module, body: &hir::Body) -> Self {
        Self::for_ir("HIR", module, None, |builder| body.build_rich_ir(builder))
    }
    #[must_use]
    pub fn for_mir(module: &Module, mir: &Mir, tracing_config: TracingConfig) -> Self {
        Self::for_ir("MIR", module, tracing_config, |builder| {
            mir.build_rich_ir(builder);
        })
    }
    #[must_use]
    pub fn for_optimized_mir(module: &Module, mir: &Mir, tracing_config: TracingConfig) -> Self {
        Self::for_ir("Optimized MIR", module, tracing_config, |builder| {
            mir.build_rich_ir(builder);
        })
    }
    #[must_use]
    pub fn for_lir(module: &Module, lir: &Lir, tracing_config: TracingConfig) -> Self {
        Self::for_ir("LIR", module, tracing_config, |builder| {
            lir.build_rich_ir(builder);
        })
    }
    #[must_use]
    pub fn for_optimized_lir(module: &Module, lir: &Lir, tracing_config: TracingConfig) -> Self {
        Self::for_ir("Optimized LIR", module, tracing_config, |builder| {
            lir.build_rich_ir(builder);
        })
    }
    #[must_use]
    fn for_ir(
        ir_name: &str,
        module: &Module,
        tracing_config: impl Into<Option<TracingConfig>>,
        build_rich_ir: impl FnOnce(&mut RichIrBuilder),
    ) -> Self {
        let mut builder = RichIrBuilder::default();
        builder.push_comment_line(format!("{ir_name} for module {module}"));
        if let Some(tracing_config) = tracing_config.into() {
            builder.push_tracing_config(tracing_config);
            builder.push_newline();
        }

        build_rich_ir(&mut builder);

        builder.finish(true)
    }
}
