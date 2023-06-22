use std::{
    collections::hash_map::DefaultHasher,
    fmt,
    hash::{self, Hasher},
};

use itertools::Itertools;
use rustc_hash::{FxHashMap, FxHashSet};
use tracing::info;

use crate::{
    builtin_functions::BuiltinFunction,
    compiler::mir::{Body, Expression, Id, Mir, VisitorResult},
    utils::CountableId,
};

#[derive(PartialEq, Eq, Clone, Hash)]
enum FlowValue {
    /// We don't have any information about the value whatsoever.
    Any,

    /// We know exactly what the value is.
    Symbol(String),
    Builtin(BuiltinFunction),

    // We know the type of the value.
    Int,
    Text,
    List,
    Struct,
    Lambda {
        return_value: Box<FlowValue>,
    },

    /// The expression will never have a value because the program always panics
    /// before evaluating it.
    Never,
}
impl fmt::Debug for FlowValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Any => write!(f, "?"),
            Self::Symbol(symbol) => write!(f, "{symbol}"),
            Self::Builtin(builtin) => write!(f, "builtin{builtin:?}"),
            Self::Int => write!(f, "int"),
            Self::Text => write!(f, "text"),
            Self::List => write!(f, "list"),
            Self::Struct => write!(f, "struct"),
            Self::Lambda { return_value } => write!(f, "{{ {return_value:?} }}"),
            Self::Never => write!(f, "💥"),
        }
    }
}

#[derive(Default, Clone, Eq, PartialEq)]
struct Timeline {
    values: FxHashMap<Id, FlowValue>,
}
// #[allow(clippy::derive_hash_xor_eq)]
impl hash::Hash for Timeline {
    fn hash<H: hash::Hasher>(&self, state: &mut H) {
        let mut hash = 0u64;
        for (id, value) in &self.values {
            let mut state = DefaultHasher::default();
            id.hash(&mut state);
            value.hash(&mut state);
            hash ^= state.finish();
        }
        hash.hash(state);
    }
}
impl Timeline {
    fn set(&mut self, id: Id, value: FlowValue) {
        self.values.insert(id, value);
    }

    fn run(&self, id: Id, expression: &Expression) -> Vec<Timeline> {
        let values = match expression {
            Expression::Int(_) => vec![FlowValue::Int],
            Expression::Text(_) => vec![FlowValue::Text],
            Expression::Symbol(symbol) => vec![FlowValue::Symbol(symbol.to_string())],
            Expression::Builtin(builtin) => vec![FlowValue::Builtin(*builtin)],
            Expression::List(_) => vec![FlowValue::List],
            Expression::Struct(_) => vec![FlowValue::Struct],
            Expression::Reference(reference) => vec![self.values[reference].clone()],
            Expression::HirId(_) => vec![FlowValue::Any],
            Expression::Lambda { body, .. } => {
                // TODO: Properly handle panics inside of the body.
                vec![FlowValue::Lambda {
                    return_value: Box::new(self.values[&body.return_value()].clone()),
                }]
            }
            Expression::Parameter => vec![FlowValue::Any],
            Expression::Call {
                function,
                arguments,
                ..
            } => {
                if let FlowValue::Builtin(builtin) = self.values[function] {
                    let arguments = arguments.iter().map(|arg| &self.values[arg]).collect_vec();
                    Self::run_builtin(builtin, arguments)
                } else if let FlowValue::Lambda { return_value } = &self.values[function] {
                    vec![*return_value.clone()]
                } else {
                    vec![FlowValue::Any, FlowValue::Never]
                }
            }
            Expression::UseModule { .. } => {
                // Either an asset or code module, or the module can't be
                // resolved or is circular.
                vec![FlowValue::List, FlowValue::Struct, FlowValue::Never]
            }
            Expression::Panic { .. } => vec![FlowValue::Never],
            Expression::Multiple(multiple) => {
                // TODO: Properly handle panics inside the body.
                vec![self.values[&multiple.return_value()].clone()]
            }
            // These expressions are lowered to instructions that don't actually
            // put anything on the stack. In the MIR, the result of these is
            // guaranteed to never be used afterwards.
            Expression::ModuleStarts { .. }
            | Expression::ModuleEnds
            | Expression::TraceCallStarts { .. }
            | Expression::TraceCallEnds { .. }
            | Expression::TraceExpressionEvaluated { .. }
            | Expression::TraceFoundFuzzableClosure { .. } => vec![FlowValue::Any],
        };
        let mut new_timelines = vec![];
        for value in values {
            let mut timeline = self.clone();
            timeline.set(id, value);
            new_timelines.push(timeline);
        }
        new_timelines
    }

    fn run_builtin(builtin: BuiltinFunction, arguments: Vec<&FlowValue>) -> Vec<FlowValue> {
        if builtin.num_parameters() != arguments.len() {
            return vec![FlowValue::Never];
        }
        match builtin {
            BuiltinFunction::ChannelCreate => vec![FlowValue::Any, FlowValue::Never],
            BuiltinFunction::ChannelSend => vec![FlowValue::Any, FlowValue::Never],
            BuiltinFunction::ChannelReceive => vec![FlowValue::Any, FlowValue::Never],
            BuiltinFunction::Equals => match (arguments[0], arguments[1]) {
                (FlowValue::Symbol(a), FlowValue::Symbol(b)) => {
                    vec![FlowValue::Symbol(
                        if a == b { "True" } else { "False" }.to_string(),
                    )]
                }
                _ => vec![
                    FlowValue::Symbol("True".to_string()),
                    FlowValue::Symbol("False".to_string()),
                ],
            },
            BuiltinFunction::FunctionRun => vec![FlowValue::Any, FlowValue::Never],
            BuiltinFunction::GetArgumentCount => vec![FlowValue::Int, FlowValue::Never],
            BuiltinFunction::IfElse => match arguments[0] {
                FlowValue::Any => {
                    if let (
                        FlowValue::Lambda { return_value: a },
                        FlowValue::Lambda { return_value: b },
                    ) = (arguments[1], arguments[2])
                    {
                        vec![*a.clone(), *b.clone()]
                    } else {
                        vec![FlowValue::Any, FlowValue::Never]
                    }
                }
                FlowValue::Symbol(symbol) => {
                    let executed_body = match symbol.as_str() {
                        "True" => arguments[1],
                        "False" => arguments[2],
                        _ => return vec![FlowValue::Never],
                    };
                    if let FlowValue::Lambda { return_value } = executed_body {
                        vec![*return_value.clone()]
                    } else {
                        vec![FlowValue::Any, FlowValue::Never]
                    }
                }
                FlowValue::Builtin(_)
                | FlowValue::Int
                | FlowValue::Text
                | FlowValue::List
                | FlowValue::Struct
                | FlowValue::Lambda { .. }
                | FlowValue::Never => vec![FlowValue::Never],
            },
            BuiltinFunction::IntAdd => vec![FlowValue::Int, FlowValue::Never],
            BuiltinFunction::IntBitLength => vec![FlowValue::Int, FlowValue::Never],
            BuiltinFunction::IntBitwiseAnd => vec![FlowValue::Int, FlowValue::Never],
            BuiltinFunction::IntBitwiseOr => vec![FlowValue::Int, FlowValue::Never],
            BuiltinFunction::IntBitwiseXor => vec![FlowValue::Int, FlowValue::Never],
            BuiltinFunction::IntCompareTo => {
                vec![
                    FlowValue::Symbol("Less".to_string()),
                    FlowValue::Symbol("Equal".to_string()),
                    FlowValue::Symbol("Greater".to_string()),
                    FlowValue::Never,
                ]
            }
            BuiltinFunction::IntDivideTruncating => vec![FlowValue::Int, FlowValue::Never],
            BuiltinFunction::IntModulo => vec![FlowValue::Int, FlowValue::Never],
            BuiltinFunction::IntMultiply => vec![FlowValue::Int, FlowValue::Never],
            BuiltinFunction::IntParse => vec![FlowValue::Struct, FlowValue::Never],
            BuiltinFunction::IntRemainder => vec![FlowValue::Int, FlowValue::Never],
            BuiltinFunction::IntShiftLeft => vec![FlowValue::Int, FlowValue::Never],
            BuiltinFunction::IntShiftRight => vec![FlowValue::Int, FlowValue::Never],
            BuiltinFunction::IntSubtract => vec![FlowValue::Int, FlowValue::Never],
            BuiltinFunction::ListFilled => vec![FlowValue::List, FlowValue::Never],
            BuiltinFunction::ListGet => vec![FlowValue::Any, FlowValue::Never],
            BuiltinFunction::ListInsert => vec![FlowValue::List, FlowValue::Never],
            BuiltinFunction::ListLength => vec![FlowValue::Int, FlowValue::Never],
            BuiltinFunction::ListRemoveAt => vec![FlowValue::Any, FlowValue::Never],
            BuiltinFunction::ListReplace => vec![FlowValue::List, FlowValue::Never],
            BuiltinFunction::Parallel => vec![FlowValue::Any, FlowValue::Never],
            BuiltinFunction::Print => {
                vec![FlowValue::Symbol("Nothing".to_string()), FlowValue::Never]
            }
            BuiltinFunction::StructGet => vec![FlowValue::Any, FlowValue::Never],
            BuiltinFunction::StructGetKeys => vec![FlowValue::Any, FlowValue::Never],
            BuiltinFunction::StructHasKey => vec![
                FlowValue::Symbol("True".to_string()),
                FlowValue::Symbol("False".to_string()),
                FlowValue::Never,
            ],
            BuiltinFunction::TextCharacters => vec![FlowValue::Any, FlowValue::Never],
            BuiltinFunction::TextConcatenate => vec![FlowValue::Text, FlowValue::Never],
            BuiltinFunction::TextContains => vec![
                FlowValue::Symbol("True".to_string()),
                FlowValue::Symbol("False".to_string()),
                FlowValue::Never,
            ],
            BuiltinFunction::TextEndsWith => vec![
                FlowValue::Symbol("True".to_string()),
                FlowValue::Symbol("False".to_string()),
                FlowValue::Never,
            ],
            BuiltinFunction::TextFromUtf8 => vec![FlowValue::Text, FlowValue::Never],
            BuiltinFunction::TextGetRange => vec![FlowValue::Text, FlowValue::Never],
            BuiltinFunction::TextIsEmpty => vec![
                FlowValue::Symbol("True".to_string()),
                FlowValue::Symbol("False".to_string()),
                FlowValue::Never,
            ],
            BuiltinFunction::TextLength => vec![FlowValue::Int, FlowValue::Never],
            BuiltinFunction::TextStartsWith => vec![
                FlowValue::Symbol("True".to_string()),
                FlowValue::Symbol("False".to_string()),
                FlowValue::Never,
            ],
            BuiltinFunction::TextTrimEnd => vec![FlowValue::Text, FlowValue::Never],
            BuiltinFunction::TextTrimStart => vec![FlowValue::Text, FlowValue::Never],
            BuiltinFunction::ToDebugText => vec![FlowValue::Text, FlowValue::Never],
            BuiltinFunction::Try => vec![FlowValue::Any, FlowValue::Never],
            BuiltinFunction::TypeOf => vec![
                FlowValue::Symbol("Int".to_string()),
                FlowValue::Symbol("Text".to_string()),
                FlowValue::Symbol("Symbol".to_string()),
                FlowValue::Symbol("List".to_string()),
                FlowValue::Symbol("Struct".to_string()),
                FlowValue::Symbol("Function".to_string()),
                FlowValue::Symbol("Builtin".to_string()),
                FlowValue::Symbol("SendPort".to_string()),
                FlowValue::Symbol("ReceivePort".to_string()),
            ],
        }
    }
}

struct Multiverse {
    timelines: Vec<Timeline>,
}
impl Multiverse {
    fn big_bang() -> Self {
        Self {
            timelines: vec![Timeline::default()],
        }
    }

    fn run(&mut self, id: Id, expression: &Expression) {
        let mut new_timelines = vec![];
        for timeline in &self.timelines {
            new_timelines.extend(timeline.run(id, expression));
        }
        self.timelines = new_timelines;
    }

    fn possible_values_for(&self, id: Id) -> FxHashSet<FlowValue> {
        self.timelines
            .iter()
            .fold(FxHashSet::default(), |mut set, timeline| {
                set.insert(timeline.values[&id].clone());
                set
            })
    }

    fn purge(&mut self, ids_to_purge: &[Id]) {
        for timeline in &mut self.timelines {
            for purged in ids_to_purge {
                timeline.values.remove(purged);
            }
        }

        let new_timelines = self
            .timelines
            .drain(..)
            .collect::<FxHashSet<_>>()
            .into_iter()
            .collect();
        self.timelines = new_timelines;
    }
}

impl Mir {
    pub fn gather_data_flow_insights(&self) {
        println!("Gathering data flow insights");

        let mut id_to_its_last_usage = FxHashMap::default();
        self.body.visit_in_analysis_order(&mut |id, expression| {
            id_to_its_last_usage.insert(id, id);
            for referenced in expression.ids_used_for_analysis() {
                id_to_its_last_usage.insert(referenced, id);
            }
        });

        let mut id_to_last_usages_it_does: FxHashMap<Id, Vec<Id>> = FxHashMap::default();
        for (used_id, usage) in id_to_its_last_usage {
            id_to_last_usages_it_does
                .entry(usage)
                .or_default()
                .push(used_id);
        }

        let mut insights: FxHashMap<Id, FxHashSet<FlowValue>> = FxHashMap::default();
        let mut multiverse = Multiverse::big_bang();
        self.body.visit_in_analysis_order(&mut |id, expression| {
            multiverse.run(id, expression);
            insights.insert(id, multiverse.possible_values_for(id));
            multiverse.purge(id_to_last_usages_it_does.get(&id).unwrap_or(&vec![]));

            info!(
                "{id:?}: {:30}  ({} timelines)",
                insights[&id]
                    .iter()
                    .map(|value| format!("{value:?}"))
                    .join(" | "),
                multiverse.timelines.len(),
            );
        });

        // TODO: Do something with the insights.
    }
}

impl Body {
    fn visit_in_analysis_order(&self, visitor: &mut dyn FnMut(Id, &Expression)) {
        for (id, expression) in self.iter() {
            expression.visit_in_analysis_order(id, visitor);
        }
    }
}
impl Expression {
    fn visit_in_analysis_order(&self, id: Id, visitor: &mut dyn FnMut(Id, &Expression)) {
        match self {
            Expression::Lambda {
                parameters,
                responsible_parameter,
                body,
            } => {
                for parameter in parameters {
                    visitor(*parameter, &Expression::Parameter);
                }
                visitor(*responsible_parameter, &Expression::Parameter);
                body.visit_in_analysis_order(visitor)
            }
            Expression::Multiple(multiple) => multiple.visit_in_analysis_order(visitor),
            Expression::Int(_)
            | Expression::Text(_)
            | Expression::Symbol(_)
            | Expression::Builtin(_)
            | Expression::List(_)
            | Expression::Struct(_)
            | Expression::Reference(_)
            | Expression::HirId(_)
            | Expression::Parameter
            | Expression::Call { .. }
            | Expression::UseModule { .. }
            | Expression::Panic { .. }
            | Expression::ModuleStarts { .. }
            | Expression::ModuleEnds
            | Expression::TraceCallStarts { .. }
            | Expression::TraceCallEnds { .. }
            | Expression::TraceExpressionEvaluated { .. }
            | Expression::TraceFoundFuzzableClosure { .. } => {}
        }
        visitor(id, self);
    }
}

impl Expression {
    fn ids_used_for_analysis(&self) -> Vec<Id> {
        match self {
            Expression::Lambda { body, .. } | Expression::Multiple(body) => {
                vec![body.return_value()]
            }
            Expression::Int(_)
            | Expression::Text(_)
            | Expression::Symbol(_)
            | Expression::Builtin(_)
            | Expression::List(_)
            | Expression::Struct(_)
            | Expression::Reference(_)
            | Expression::HirId(_)
            | Expression::Parameter
            | Expression::Call { .. }
            | Expression::UseModule { .. }
            | Expression::Panic { .. }
            | Expression::ModuleStarts { .. }
            | Expression::ModuleEnds
            | Expression::TraceCallStarts { .. }
            | Expression::TraceCallEnds { .. }
            | Expression::TraceExpressionEvaluated { .. }
            | Expression::TraceFoundFuzzableClosure { .. } => {
                self.referenced_ids().into_iter().collect()
            }
        }
    }
}
