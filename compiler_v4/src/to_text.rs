pub trait ToText {
    #[must_use]
    fn to_text(&self, trailing_newline: bool) -> String {
        let mut builder = TextBuilder::default();
        self.build_text(&mut builder);
        builder.finish(trailing_newline)
    }
    fn build_text(&self, builder: &mut TextBuilder);
}

impl<T: ToText> ToText for &T {
    fn build_text(&self, builder: &mut TextBuilder) {
        (*self).build_text(builder);
    }
}

#[derive(Debug, Default)]
pub struct TextBuilder {
    text: String,
    indentation: usize,
}
impl TextBuilder {
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
        self.push("\n");
        self.push("  ".repeat(self.indentation));
    }
    pub fn push_children_multiline<'c, C>(&mut self, children: impl IntoIterator<Item = &'c C>)
    where
        C: ToText + 'c,
    {
        self.push_children_custom_multiline(children, |builder, child| {
            child.build_text(builder);
        });
    }
    pub fn push_children_custom_multiline<C>(
        &mut self,
        children: impl IntoIterator<Item = C>,
        push_child: impl FnMut(&mut Self, &C),
    ) {
        self.push_indented(|builder| {
            builder.push_custom_multiline(children, push_child);
        });
    }
    pub fn push_multiline<'c, C>(&mut self, items: impl IntoIterator<Item = &'c C>)
    where
        C: ToText + 'c,
    {
        self.push_custom_multiline(items, |builder, item| item.build_text(builder));
    }
    pub fn push_custom_multiline<C>(
        &mut self,
        items: impl IntoIterator<Item = C>,
        mut push_item: impl FnMut(&mut Self, &C),
    ) {
        for item in items {
            self.push_newline();
            push_item(self, &item);
        }
    }

    pub fn push_children<C: ToText>(
        &mut self,
        children: impl IntoIterator<Item = C>,
        separator: impl AsRef<str>,
    ) {
        self.push_children_custom(
            children,
            |builder, child| child.build_text(builder),
            separator,
        );
    }
    pub fn push_children_custom<C>(
        &mut self,
        children: impl IntoIterator<Item = C>,
        mut push_child: impl FnMut(&mut Self, &C),
        separator: impl AsRef<str>,
    ) {
        let mut children = children.into_iter();
        let Some(first) = children.next() else {
            return;
        };
        push_child(self, &first);

        for child in children {
            self.push(separator.as_ref());
            push_child(self, &child);
        }
    }

    pub fn push_comment_line(&mut self, text: impl AsRef<str>) {
        let text = text.as_ref();
        if text.is_empty() {
            self.push("#");
        } else {
            self.push("# ");
            self.push(text);
        }
        self.push_newline();
    }

    pub fn push(&mut self, text: impl AsRef<str>) {
        self.text.push_str(text.as_ref());
    }

    #[must_use]
    pub fn finish(mut self, trailing_newline: bool) -> String {
        if trailing_newline && !self.text.is_empty() && !self.text.ends_with('\n') {
            self.push("\n");
        }
        self.text
    }
}
