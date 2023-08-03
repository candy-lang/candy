use crate::{
    ast_to_hir::AstToHir,
    builtin_functions::BuiltinFunction,
    error::CompilerError,
    id::CountableId,
    impl_display_via_richir,
    module::{Module, ModuleKind, Package},
    rich_ir::{ReferenceKey, RichIrBuilder, ToRichIr, TokenModifier, TokenType},
};

use derive_more::From;
use enumset::EnumSet;
use itertools::Itertools;
use linked_hash_map::LinkedHashMap;
use num_bigint::BigUint;
use rustc_hash::FxHashMap;
use std::{
    fmt::{self, Debug, Display, Formatter},
    hash::{Hash, Hasher},
    sync::Arc,
};
use tracing::info;

#[salsa::query_group(HirDbStorage)]
pub trait HirDb: AstToHir {
    fn find_expression(&self, id: Id) -> Option<Expression>;
    fn containing_body_of(&self, id: Id) -> Arc<Body>;
    fn all_hir_ids(&self, module: Module) -> Vec<Id>;
}
#[allow(clippy::needless_pass_by_value)]
fn find_expression(db: &dyn HirDb, id: Id) -> Option<Expression> {
    let (hir, _) = db.hir(id.module.clone()).ok()?;
    if id.is_root() {
        panic!("You can't get the root because that got lowered into multiple IDs.");
    }

    hir.find(&id).map(|it| it.to_owned())
}
fn containing_body_of(db: &dyn HirDb, id: Id) -> Arc<Body> {
    let parent_id = id.parent().expect("The root scope has no parent.");

    if parent_id.is_root() {
        db.hir(id.module).unwrap().0
    } else {
        match db.find_expression(parent_id).unwrap() {
            Expression::Match { cases, .. } => {
                let body = cases
                    .into_iter()
                    .map(|(_, body)| body)
                    .find(|body| body.expressions.contains_key(&id))
                    .unwrap();
                Arc::new(body)
            }
            Expression::Function(function) => Arc::new(function.body),
            _ => panic!("Parent of an expression must be a function (or root scope)."),
        }
    }
}
fn all_hir_ids(db: &dyn HirDb, module: Module) -> Vec<Id> {
    let Ok((hir, _)) = db.hir(module) else {
        return vec![];
    };
    let mut ids = vec![];
    hir.collect_all_ids(&mut ids);
    info!("All HIR IDs: {ids:?}");
    ids
}

impl Expression {
    pub fn collect_all_ids(&self, ids: &mut Vec<Id>) {
        match self {
            Expression::Int(_) => {}
            Expression::Text(_) => {}
            Expression::Reference(id) => {
                ids.push(id.clone());
            }
            Expression::Symbol(_) => {}
            Expression::List(items) => {
                ids.extend_from_slice(items);
            }
            Expression::Struct(entries) => {
                for (key_id, value_id) in entries.iter() {
                    ids.push(key_id.to_owned());
                    ids.push(value_id.to_owned());
                }
            }
            Expression::Destructure {
                expression,
                pattern: _,
            } => ids.push(expression.to_owned()),
            Expression::PatternIdentifierReference(_) => {}
            Expression::Match { expression, cases } => {
                ids.push(expression.to_owned());
                for (_, body) in cases {
                    body.collect_all_ids(ids);
                }
            }
            Expression::Function(Function {
                parameters, body, ..
            }) => {
                for parameter in parameters {
                    ids.push(parameter.clone());
                }
                body.collect_all_ids(ids);
            }
            Expression::Call {
                function,
                arguments,
            } => {
                ids.push(function.clone());
                ids.extend_from_slice(arguments);
            }
            Expression::UseModule { relative_path, .. } => {
                ids.push(relative_path.clone());
            }
            Expression::Builtin(_) => {}
            Expression::Needs { condition, reason } => {
                ids.push(condition.clone());
                ids.push(reason.clone());
            }
            Expression::Error { .. } => {}
        }
    }
}
impl Body {
    fn collect_all_ids(&self, ids: &mut Vec<Id>) {
        ids.extend(self.expressions.keys().cloned());
        for expression in self.expressions.values() {
            expression.collect_all_ids(ids);
        }
    }
}

#[derive(Clone, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Id {
    pub module: Module,
    pub keys: Vec<IdKey>,
}
#[derive(Clone, Eq, From, Hash, Ord, PartialEq, PartialOrd)]
pub enum IdKey {
    Named { name: String, disambiguator: usize },
    Positional(usize),
}
impl Id {
    pub fn new(module: Module, keys: Vec<IdKey>) -> Self {
        Self { module, keys }
    }

    /// An ID that can be used to blame the tooling. For example, when calling
    /// the `main` function, we want to be able to blame the platform for
    /// passing a wrong environment.
    fn tooling(name: String) -> Self {
        Self {
            module: Module {
                package: Package::Tooling(name),
                path: vec![],
                kind: ModuleKind::Code,
            },
            keys: vec![],
        }
    }
    /// The user of the Candy tooling is responsible. For example, when the user
    /// instructs the tooling to run a non-existent module, then the program
    /// will panic with this responsiblity.
    pub fn user() -> Self {
        Self::tooling("user".to_string())
    }
    /// Refers to the platform (non-Candy code).
    pub fn platform() -> Self {
        Self::tooling("platform".to_string())
    }
    pub fn fuzzer() -> Self {
        Self::tooling("fuzzer".to_string())
    }
    /// A dummy ID that is guaranteed to never be responsible for a panic.
    pub fn dummy() -> Self {
        Self::tooling("dummy".to_string())
    }
    /// TODO: Currently, when a higher-order function calls a function passed as
    /// a parameter, that's registered as a normal call instruction, making the
    /// callsite in the higher-order function responsible for the successful
    /// fulfillment of the passed function's `needs`. We probably want to change
    /// how that works so that the caller of the higher-order function is at
    /// fault when passing a panicking function. After we did that, we should be
    /// able to remove this ID.
    pub fn complicated_responsibility() -> Self {
        Self::tooling("complicated-responsibility".to_string())
    }

    pub fn needs() -> Self {
        Self {
            module: Module {
                package: Package::Anonymous {
                    url: "$generated".to_string(),
                },
                path: vec![],
                kind: ModuleKind::Code,
            },
            keys: vec![IdKey::from("needs")],
        }
    }

    pub fn to_short_debug_string(&self) -> String {
        format!("${}", self.keys.iter().join(":"))
    }

    pub fn is_root(&self) -> bool {
        self.keys.is_empty()
    }

    pub fn parent(&self) -> Option<Id> {
        match self.keys.len() {
            0 => None,
            _ => Some(Id {
                module: self.module.clone(),
                keys: self.keys[..self.keys.len() - 1].to_vec(),
            }),
        }
    }

    pub fn child(&self, key: impl Into<IdKey>) -> Self {
        let mut keys = self.keys.clone();
        keys.push(key.into());
        Self {
            module: self.module.clone(),
            keys,
        }
    }

    pub fn is_same_module_and_any_parent_of(&self, other: &Self) -> bool {
        self.module == other.module
            && self.keys.len() < other.keys.len()
            && self.keys.iter().zip(&other.keys).all(|(a, b)| a == b)
    }

    pub fn function_name(&self) -> String {
        self.keys
            .iter()
            .map(|it| match it {
                IdKey::Positional(index) => format!("<anonymous {index}>"),
                it => it.to_string(),
            })
            .join(" → ")
    }
}
impl Debug for Id {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "{}:{}", self.module, self.keys.iter().join(":"))
    }
}
impl Display for Id {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "{self:?}")
    }
}
impl ToRichIr for Id {
    fn build_rich_ir(&self, builder: &mut RichIrBuilder) {
        let range = builder.push(
            self.to_short_debug_string(),
            TokenType::Variable,
            EnumSet::empty(),
        );
        builder.push_reference(self.to_owned(), range);
    }
}
impl Debug for IdKey {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match &self {
            IdKey::Named {
                name,
                disambiguator,
            } => {
                write!(f, "{name}")?;
                if disambiguator > &0 {
                    write!(f, "#{disambiguator}")?;
                }
                Ok(())
            }
            IdKey::Positional(index) => write!(f, "{index}"),
        }
    }
}
impl Display for IdKey {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "{self:?}")
    }
}
impl From<String> for IdKey {
    fn from(value: String) -> Self {
        IdKey::Named {
            name: value,
            disambiguator: 0,
        }
    }
}
impl From<&str> for IdKey {
    fn from(value: &str) -> Self {
        value.to_string().into()
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Expression {
    Int(BigUint),
    Text(String),
    Reference(Id),
    Symbol(String),
    List(Vec<Id>),
    Struct(FxHashMap<Id, Id>),
    Destructure {
        expression: Id,
        pattern: Pattern,
    },
    PatternIdentifierReference(PatternIdentifierId),
    Match {
        expression: Id,
        /// Each case consists of the pattern to match against, and the body
        /// which starts with [PatternIdentifierReference]s for all identifiers
        /// in the pattern.
        cases: Vec<(Pattern, Body)>,
    },
    Function(Function),
    Builtin(BuiltinFunction),
    Call {
        function: Id,
        arguments: Vec<Id>,
    },
    UseModule {
        current_module: Module,
        relative_path: Id,
    },
    Needs {
        condition: Id,
        reason: Id,
    },
    Error {
        child: Option<Id>,
        errors: Vec<CompilerError>,
    },
}
impl Expression {
    pub fn nothing() -> Self {
        Expression::Symbol("Nothing".to_string())
    }
}
#[allow(clippy::derived_hash_with_manual_eq)]
impl Hash for Expression {
    fn hash<H: Hasher>(&self, state: &mut H) {
        core::mem::discriminant(self).hash(state);
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct PatternIdentifierId(pub usize);
impl CountableId for PatternIdentifierId {
    fn from_usize(id: usize) -> Self {
        Self(id)
    }
    fn to_usize(&self) -> usize {
        self.0
    }
}
impl Debug for PatternIdentifierId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "pattern_identifier_{:x}", self.0)
    }
}
impl Display for PatternIdentifierId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "p${}", self.0)
    }
}
impl ToRichIr for PatternIdentifierId {
    fn build_rich_ir(&self, builder: &mut RichIrBuilder) {
        // TODO: convert to actual reference
        builder.push(self.to_string(), TokenType::Variable, EnumSet::empty());
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Pattern {
    NewIdentifier(PatternIdentifierId),
    Int(BigUint),
    Text(String),
    Tag {
        symbol: String,
        value: Option<Box<Pattern>>,
    },
    List(Vec<Pattern>),
    // Keys may not contain `NewIdentifier`.
    Struct(Vec<(Pattern, Pattern)>),
    Or(Vec<Pattern>),
    Error {
        child: Option<Box<Pattern>>,
        errors: Vec<CompilerError>,
    },
}
#[allow(clippy::derived_hash_with_manual_eq)]
impl Hash for Pattern {
    fn hash<H: Hasher>(&self, state: &mut H) {
        core::mem::discriminant(self).hash(state);
    }
}
impl Pattern {
    pub fn contains_captured_identifiers(&self) -> bool {
        match self {
            Pattern::NewIdentifier(_) => true,
            Pattern::Int(_) | Pattern::Text(_) => false,
            Pattern::Tag { value, .. } => value
                .as_ref()
                .map(|value| value.contains_captured_identifiers())
                .unwrap_or_default(),
            Pattern::List(list) => list.iter().any(|it| it.contains_captured_identifiers()),
            Pattern::Struct(struct_) => struct_
                .iter()
                .any(|(_, value_pattern)| value_pattern.contains_captured_identifiers()),
            Pattern::Or(patterns) => patterns.first().unwrap().contains_captured_identifiers(),
            Pattern::Error { child, .. } => child
                .as_ref()
                .map(|child| child.contains_captured_identifiers())
                .unwrap_or_default(),
        }
    }
    pub fn captured_identifier_count(&self) -> usize {
        match self {
            Pattern::NewIdentifier(_) => 1,
            Pattern::Int(_) | Pattern::Text(_) => 0,
            Pattern::Tag { value, .. } => value
                .as_ref()
                .map(|value| value.captured_identifier_count())
                .unwrap_or_default(),
            Pattern::List(list) => list.iter().map(|it| it.captured_identifier_count()).sum(),
            Pattern::Struct(struct_) => struct_
                .iter()
                .map(|(key, value)| {
                    key.captured_identifier_count() + value.captured_identifier_count()
                })
                .sum(),
            // If the number or captured identifiers isn't the same in both
            // sides, the pattern is invalid and the generated code will panic.
            Pattern::Or(patterns) => patterns.first().unwrap().captured_identifier_count(),
            Pattern::Error { child, .. } => {
                // Since generated code panics in this case, it doesn't matter
                // whether the child captured any identifiers since they can't
                // be accessed anyway.
                child
                    .as_ref()
                    .map(|child| child.captured_identifier_count())
                    .unwrap_or_default()
            }
        }
    }

    /// Returns a mapping from `PatternIdentifierId` to the position of the
    /// corresponding identifier in the `(Match, …)` result (zero-based,
    /// ignoring the `Match` symbol).
    pub fn captured_identifiers(&self) -> Vec<PatternIdentifierId> {
        let mut ids = vec![];
        self.collect_captured_identifiers(&mut ids);
        ids
    }
    fn collect_captured_identifiers(&self, ids: &mut Vec<PatternIdentifierId>) {
        match self {
            Pattern::NewIdentifier(identifier_id) => ids.push(*identifier_id),
            Pattern::Int(_) | Pattern::Text(_) => {}
            Pattern::Tag { value, .. } => {
                if let Some(value) = value {
                    value.collect_captured_identifiers(ids);
                }
            }
            Pattern::List(list) => {
                for pattern in list {
                    pattern.collect_captured_identifiers(ids);
                }
            }
            Pattern::Struct(struct_) => {
                for (_, value_pattern) in struct_ {
                    // Keys can't capture identifiers.
                    value_pattern.collect_captured_identifiers(ids);
                }
            }
            // If the number or captured identifiers isn't the same in both
            // sides, the pattern is invalid and the generated code will panic.
            Pattern::Or(patterns) => patterns.first().unwrap().collect_captured_identifiers(ids),
            Pattern::Error { .. } => {
                // Since generated code panics in this case, it doesn't matter
                // whether the child captured any identifiers since they can't
                // be accessed anyway.
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Function {
    pub parameters: Vec<Id>,
    pub body: Body,
    pub fuzzable: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Body {
    pub expressions: LinkedHashMap<Id, Expression>,
    pub identifiers: FxHashMap<Id, String>,
}
#[allow(clippy::derived_hash_with_manual_eq)]
impl Hash for Body {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.expressions.hash(state);
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum HirError {
    NeedsWithWrongNumberOfArguments { num_args: usize },
    PatternContainsCall,
    PublicAssignmentInNotTopLevel,
    PublicAssignmentWithSameName { name: String },
    UnknownReference { name: String },
}

impl Body {
    pub fn push(&mut self, id: Id, expression: Expression, identifier: Option<String>) {
        self.expressions.insert(id.to_owned(), expression);
        if let Some(identifier) = identifier {
            self.identifiers.insert(id, identifier);
        }
    }
}

impl_display_via_richir!(Expression);
impl ToRichIr for Expression {
    fn build_rich_ir(&self, builder: &mut RichIrBuilder) {
        match self {
            Expression::Int(int) => {
                let range = builder.push(int.to_string(), TokenType::Int, EnumSet::empty());
                builder.push_reference(ReferenceKey::Int(int.to_owned().into()), range);
            }
            Expression::Text(text) => {
                let range =
                    builder.push(format!(r#""{}""#, text), TokenType::Text, EnumSet::empty());
                builder.push_reference(text.to_owned(), range);
            }
            Expression::Reference(reference) => {
                reference.build_rich_ir(builder);
            }
            Expression::Symbol(symbol) => {
                let range = builder.push(symbol, TokenType::Symbol, EnumSet::empty());
                builder.push_reference(ReferenceKey::Symbol(symbol.to_owned()), range);
            }
            Expression::List(items) => {
                builder.push("(", None, EnumSet::empty());
                builder.push_children(items, ", ");
                builder.push(")", None, EnumSet::empty());
            }
            Expression::Struct(fields) => {
                builder.push("[", None, EnumSet::empty());
                builder.push_children_custom(
                    fields.iter().collect_vec(),
                    |builder, (key, value)| {
                        key.build_rich_ir(builder);
                        builder.push(": ", None, EnumSet::empty());
                        value.build_rich_ir(builder);
                    },
                    ", ",
                );
                builder.push("]", None, EnumSet::empty());
            }
            Expression::Destructure {
                expression,
                pattern,
            } => {
                builder.push("destructure ", None, EnumSet::empty());
                expression.build_rich_ir(builder);
                builder.push(" into ", None, EnumSet::empty());
                pattern.build_rich_ir(builder);
            }
            Expression::PatternIdentifierReference(identifier_id) => {
                identifier_id.build_rich_ir(builder)
            }
            Expression::Match { expression, cases } => {
                expression.build_rich_ir(builder);
                builder.push(" %", None, EnumSet::empty());
                builder.push_children_custom_multiline(cases, |builder, (pattern, body)| {
                    pattern.build_rich_ir(builder);
                    builder.push(" ->", None, EnumSet::empty());
                    builder.indent();
                    builder.push_foldable(|builder| {
                        if !body.expressions.is_empty() {
                            builder.push_newline();
                        }
                        body.build_rich_ir(builder);
                    });
                    builder.dedent();
                });
            }
            Expression::Function(function) => {
                builder.push(
                    format!(
                        "{{ ({}) ",
                        if function.fuzzable {
                            "fuzzable"
                        } else {
                            "non-fuzzable"
                        },
                    ),
                    None,
                    EnumSet::empty(),
                );
                function.build_rich_ir(builder);
                builder.push("}", None, EnumSet::empty());
            }
            Expression::Builtin(builtin) => {
                let range = builder.push(
                    format!("builtin{builtin:?}"),
                    TokenType::Function,
                    EnumSet::only(TokenModifier::Builtin),
                );
                builder.push_reference(*builtin, range);
            }
            Expression::Call {
                function,
                arguments,
            } => {
                assert!(!arguments.is_empty(), "A call needs to have arguments.");
                builder.push("call ", None, EnumSet::empty());
                function.build_rich_ir(builder);
                builder.push(" with ", None, EnumSet::empty());
                builder.push_children(arguments, " ");
            }
            Expression::UseModule {
                current_module,
                relative_path,
            } => {
                builder.push("relative to module ", None, EnumSet::empty());
                current_module.build_rich_ir(builder);
                builder.push(", use ", None, EnumSet::empty());
                relative_path.build_rich_ir(builder);
            }
            Expression::Needs { condition, reason } => {
                builder.push("needs ", None, EnumSet::empty());
                condition.build_rich_ir(builder);
                builder.push(" with reason ", None, EnumSet::empty());
                reason.build_rich_ir(builder);
            }
            Expression::Error { child, errors } => {
                build_errors_rich_ir(builder, errors, child);
            }
        }
    }
}
impl ToRichIr for Pattern {
    fn build_rich_ir(&self, builder: &mut RichIrBuilder) {
        match self {
            Pattern::Int(int) => {
                builder.push(format!("{int}"), TokenType::Int, EnumSet::empty());
            }
            Pattern::Text(text) => {
                builder.push(format!(r#""{text:?}\""#), TokenType::Text, EnumSet::empty());
            }
            Pattern::NewIdentifier(reference) => reference.build_rich_ir(builder),
            Pattern::Tag { symbol, value } => {
                builder.push(symbol, TokenType::Symbol, EnumSet::empty());
                if let Some(value) = value {
                    builder.push(" (", None, EnumSet::empty());
                    value.build_rich_ir(builder);
                    builder.push(")", None, EnumSet::empty());
                }
            }
            Pattern::List(items) => {
                builder.push("(", None, EnumSet::empty());
                builder.push_children(items, ", ");
                if items.len() <= 1 {
                    builder.push(",", None, EnumSet::empty());
                }
                builder.push(")", None, EnumSet::empty());
            }
            Pattern::Struct(entries) => {
                builder.push("[", None, EnumSet::empty());
                builder.push_children_custom(
                    entries,
                    |builder, (key, value)| {
                        key.build_rich_ir(builder);
                        builder.push(": ", None, EnumSet::empty());
                        value.build_rich_ir(builder);
                    },
                    ", ",
                );
                builder.push("]", None, EnumSet::empty());
            }
            Pattern::Or(patterns) => builder.push_children(patterns, " | "),
            Pattern::Error { child, errors } => {
                build_errors_rich_ir(builder, errors, child);
            }
        }
    }
}
fn build_errors_rich_ir<C: ToRichIr>(
    builder: &mut RichIrBuilder,
    errors: &[CompilerError],
    child: &Option<C>,
) {
    builder.push(
        if errors.len() == 1 { "error" } else { "errors" },
        None,
        EnumSet::empty(),
    );
    builder.push_foldable(|builder| {
        builder.push_children_multiline(errors);
        if let Some(child) = child {
            builder.indent();
            builder.push_newline();
            builder.push("fallback: ", None, EnumSet::empty());
            child.build_rich_ir(builder);
            builder.dedent();
        }
    });
}
impl ToRichIr for Function {
    fn build_rich_ir(&self, builder: &mut RichIrBuilder) {
        for parameter in &self.parameters {
            let range = builder.push(
                parameter.to_short_debug_string(),
                TokenType::Parameter,
                EnumSet::empty(),
            );
            builder.push_definition(parameter.to_owned(), range);
            builder.push(" ", None, EnumSet::empty());
        }
        builder.push("->", None, EnumSet::empty());
        builder.push_foldable(|builder| {
            builder.indent();
            builder.push_newline();
            self.body.build_rich_ir(builder);
            builder.dedent();
            builder.push_newline();
        });
    }
}
impl ToRichIr for Body {
    fn build_rich_ir(&self, builder: &mut RichIrBuilder) {
        fn push(builder: &mut RichIrBuilder, id: &Id, expression: &Expression) {
            let range = builder.push(
                id.to_short_debug_string(),
                TokenType::Variable,
                EnumSet::empty(),
            );
            builder.push_definition(id.to_owned(), range);

            builder.push(" = ", None, EnumSet::empty());
            expression.build_rich_ir(builder);
        }

        let mut iterator = self.expressions.iter();
        if let Some((id, expression)) = iterator.next() {
            push(builder, id, expression);
        }
        for (id, expression) in iterator {
            builder.push_newline();
            push(builder, id, expression);
        }
    }
}

impl Expression {
    fn find(&self, id: &Id) -> Option<&Self> {
        match self {
            Expression::Int { .. } => None,
            Expression::Text { .. } => None,
            Expression::Reference { .. } => None,
            Expression::Symbol { .. } => None,
            Expression::List(_) => None,
            Expression::Struct(_) => None,
            Expression::Destructure { .. } => None,
            Expression::PatternIdentifierReference { .. } => None,
            // TODO: use binary search
            Expression::Match { cases, .. } => cases.iter().find_map(|(_, body)| body.find(id)),
            Expression::Function(Function { body, .. }) => body.find(id),
            Expression::Builtin(_) => None,
            Expression::Call { .. } => None,
            Expression::UseModule { .. } => None,
            Expression::Needs { .. } => None,
            Expression::Error { .. } => None,
        }
    }
}
impl Body {
    pub fn find(&self, id: &Id) -> Option<&Expression> {
        if let Some(expression) = self.expressions.get(id) {
            Some(expression)
        } else {
            self.expressions
                .iter()
                .filter(|(it, _)| it <= &id)
                .max_by_key(|(id, _)| id.keys.to_owned())?
                .1
                .find(id)
        }
    }
}

pub trait CollectErrors {
    fn collect_errors(&self, errors: &mut Vec<CompilerError>);
}
impl CollectErrors for Expression {
    fn collect_errors(&self, errors: &mut Vec<CompilerError>) {
        match self {
            Expression::Int(_)
            | Expression::Text(_)
            | Expression::Reference(_)
            | Expression::Symbol(_)
            | Expression::List(_)
            | Expression::Struct(_)
            | Expression::PatternIdentifierReference { .. } => {}
            Expression::Match { cases, .. } => {
                for (pattern, body) in cases {
                    pattern.collect_errors(errors);
                    body.collect_errors(errors);
                }
            }
            Expression::Builtin(_)
            | Expression::Call { .. }
            | Expression::UseModule { .. }
            | Expression::Needs { .. } => {}
            Expression::Function(function) => function.body.collect_errors(errors),
            Expression::Destructure { pattern, .. } => pattern.collect_errors(errors),
            Expression::Error {
                errors: the_errors, ..
            } => {
                errors.append(&mut the_errors.clone());
            }
        }
    }
}
impl CollectErrors for Pattern {
    fn collect_errors(&self, errors: &mut Vec<CompilerError>) {
        match self {
            Pattern::NewIdentifier(_) | Pattern::Int(_) | Pattern::Text(_) => {}
            Pattern::Tag { value, .. } => {
                if let Some(value) = value {
                    value.collect_errors(errors);
                }
            }
            Pattern::List(patterns) => {
                for item_pattern in patterns {
                    item_pattern.collect_errors(errors);
                }
            }
            Pattern::Struct(patterns) => {
                for (key_pattern, value_pattern) in patterns {
                    key_pattern.collect_errors(errors);
                    value_pattern.collect_errors(errors);
                }
            }
            Pattern::Or(patterns) => {
                for pattern in patterns {
                    pattern.collect_errors(errors);
                }
            }
            Pattern::Error {
                errors: the_errors, ..
            } => {
                errors.append(&mut the_errors.clone());
            }
        }
    }
}
impl CollectErrors for Body {
    fn collect_errors(&self, errors: &mut Vec<CompilerError>) {
        for (_id, expression) in &self.expressions {
            expression.collect_errors(errors);
        }
    }
}
