use std::ops::Range;

use enumset::{EnumSet, EnumSetType};

use crate::position::Offset;

#[derive(Default)]
pub struct RichIr {
    pub text: String,
    pub annotations: Vec<RichIrAnnotation>,
}

pub struct RichIrAnnotation {
    pub range: Range<Offset>,
    pub token_type: Option<TokenType>,
    pub token_modifiers: EnumSet<TokenModifier>,
}

#[derive(Debug, Hash, PartialEq, Eq, Clone, Copy)]
pub enum TokenType {
    Int,
    Symbol,
    Text,
}
#[derive(Debug, EnumSetType)]
pub enum TokenModifier {}

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

#[derive(Default)]
pub struct RichIrBuilder {
    ir: RichIr,
    indentation: usize,
}
impl RichIrBuilder {
    pub fn push_newline(&mut self) {
        self.push("\n", None, EnumSet::empty());
        self.push("  ".repeat(self.indentation), None, EnumSet::empty());
    }
    pub fn push_children<CS: AsRef<[C]>, C: ToRichIr>(&mut self, children: CS) {
        self.push_children_custom(children, |builder, child| child.build_rich_ir(builder))
    }
    pub fn push_children_custom<CS: AsRef<[C]>, C, F>(&mut self, children: CS, mut push_child: F)
    where
        F: FnMut(&mut Self, &C),
    {
        let children = children.as_ref();
        if children.is_empty() {
            return;
        }

        self.indentation += 1;
        for child in children {
            self.push_newline();
            push_child(self, child);
        }
        self.indentation -= 1;
    }

    pub fn push<S: AsRef<str>>(
        &mut self,
        text: S,
        token_type: Option<TokenType>,
        token_modifiers: EnumSet<TokenModifier>,
    ) {
        let start = self.ir.text.len().into();
        self.ir.text.push_str(text.as_ref());
        let end = self.ir.text.len().into();
        if token_type.is_some() || !token_modifiers.is_empty() {
            self.ir.annotations.push(RichIrAnnotation {
                range: start..end,
                token_type,
                token_modifiers,
            });
        }
    }

    pub fn finish(self) -> RichIr {
        self.ir
    }
}
