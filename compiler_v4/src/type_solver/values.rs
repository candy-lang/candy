use crate::{
    hir::{NamedType, ParameterType, Type, TypeParameterId},
    id::CountableId,
};
use enum_dispatch::enum_dispatch;
use itertools::Itertools;
use rustc_hash::FxHashMap;
use std::{
    collections::VecDeque,
    fmt::{self, Display, Formatter},
};

pub fn canonical_variable(n: usize) -> ParameterType {
    ParameterType {
        name: format!("canonical${n}").into_boxed_str(),
        id: TypeParameterId::from_usize(usize::MAX - n - 1),
    }
}

/// Either a `SolverVariable` or a `SolverValue`.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
#[enum_dispatch(SolverTypeTrait)]
pub enum SolverType {
    Variable(SolverVariable),
    Value(SolverValue),
}
impl SolverType {
    /// Unifies `this` type with the `other`. If unification succeeds, returns a map from variables
    /// to substitutions. Unification may fail if no variable substitutions exist that makes both
    /// types the same.
    ///
    /// Examples:
    ///
    /// * Unifying `String` with `String` succeeds. No substitutions necessary.
    /// * Unifying `Int` with `String` fails.
    /// * Unifying `Int` with `?A` succeeds with the substitution `?A = Int`.
    /// * Unifying `List<?A>` with `List<Set<?B>>` succeeds with the substitution `?A = Set<?B>`.
    /// * Unifying `Map<?A, ?A>` with `Map<Int, String>` fails.
    /// * Unifying `Map<?A, ?A>` with `Map<Int, Int>` succeeds with the subsitution `?A = Int`.
    /// * Unifying `Map<?A, ?A>` with `Map<?B, Int>` succeeds with the substitutions `?A = Int` and
    ///   `?B = Int`.
    /// * Unifying `Set<?A>` with `Set<?B>` succeeds with the substitution `?A = ?B`.
    /// * Unifying `?Batman` with `Na<?Batman>` fails because repeated substitution would lead to
    ///   types of infinite size:
    ///   `?Batman = Na<?Batman> = Na<Na<?Batman>> = ... = Na<Na<Na<Na<Na<Na<?Batman>>>>>> = ...`
    /// * Unifying `Map<?A, ?B>` with `Map<Ping<?B>, Pong<?A>>` fails because repeated substitution
    ///   would lead to types of infinite size:
    ///   `?A = Ping<?B> = Ping<Pong<?A>> = Ping<Pong<Ping<?B>>> = Ping<Pong<Ping<Pong<?A>>>> = ...`
    // TODO(never, marcelgarus): Write property-based tests for this. For example, generate random types
    // and then check that the order of unifying doesn't matter and that applying the substitutions
    // actually results in the same type.
    pub fn unify(&self, other: &Self) -> Option<FxHashMap<SolverVariable, Self>> {
        match (&self, &other) {
            (Self::Variable(self_variable), Self::Variable(other_variable)) => {
                // If both are variables, we return a substitution with the lexicographically first one being
                // the one subsituted. For example, unifying `?B` with `?A` would yield the substitution
                // `?A = ?B`.

                if self_variable.type_ == other_variable.type_ {
                    // Being equal to itself is no bound, so no substitution `?A = ?A` is necessary.
                    return Some(FxHashMap::default());
                }
                let entry = if let Some(self_type) = &self_variable.type_
                    && let Some(other_type) = &other_variable.type_
                    && self_type.name < other_type.name
                {
                    (self_variable.clone(), other.clone())
                } else if self_variable.type_.is_some() {
                    (other_variable.clone(), self.clone())
                } else {
                    (self_variable.clone(), other.clone())
                };
                Some(FxHashMap::from_iter([entry]))
            }
            (Self::Value(_), Self::Variable(_)) => other.unify(self),
            (Self::Variable(self_variable), other @ Self::Value(_)) => {
                if other.contains_variable(self_variable) {
                    // This is an infinitely growing type. That's not allowed.
                    return None;
                }
                Some(FxHashMap::from_iter([(
                    self_variable.clone(),
                    (*other).clone(),
                )]))
            }

            (Self::Value(self_value), Self::Value(other_value)) => {
                if self_value.type_ != other_value.type_
                    || self_value.parameters.len() != other_value.parameters.len()
                {
                    return None;
                }

                // To unify all types, we have a queue of substitutions that we try to bring into a
                // common solution space one by one.
                let mut solution_space = FxHashMap::<SolverVariable, SolverType>::default();
                let mut queue = self_value
                    .parameters
                    .iter()
                    .zip(other_value.parameters.iter())
                    .map(|(a, b)| a.unify(b))
                    .collect::<Option<Vec<_>>>()?
                    .into_iter()
                    .flatten()
                    .collect::<VecDeque<_>>();

                while let Some(substitution) = queue.pop_front() {
                    if let Some(existing_substitution) = solution_space.get(&substitution.0) {
                        queue.extend(existing_substitution.unify(&substitution.1)?.into_iter());
                    } else {
                        solution_space.insert(substitution.0.clone(), substitution.1.clone());
                        for type_ in solution_space.values_mut() {
                            *type_ =
                                type_.substitute(substitution.0.clone(), substitution.1.clone());
                        }
                        for (_, type_) in &mut queue {
                            *type_ =
                                type_.substitute(substitution.0.clone(), substitution.1.clone());
                        }
                        if queue
                            .iter()
                            .any(|(variable, type_)| type_.contains_variable(variable))
                        {
                            return None;
                        }
                    }
                }

                Some(solution_space)
            }
        }
    }
}
#[enum_dispatch]
pub trait SolverTypeTrait: Sized {
    fn contains_variable(&self, variable: &SolverVariable) -> bool;

    fn substitute(&self, variable: SolverVariable, substitution: SolverType) -> SolverType {
        self.substitute_all(&FxHashMap::from_iter([(variable, substitution)]))
    }
    fn substitute_all(&self, substitutions: &FxHashMap<SolverVariable, SolverType>) -> SolverType;

    fn canonicalize(&self) -> Self {
        self.canonicalize_internal(&mut FxHashMap::default())
    }
    fn canonicalize_internal(
        &self,
        mapping: &mut FxHashMap<SolverVariable, SolverVariable>,
    ) -> Self;
}
impl TryFrom<Type> for SolverType {
    type Error = ();

    fn try_from(type_: Type) -> Result<Self, Self::Error> {
        match type_ {
            Type::Named(named_type) => SolverValue::try_from(named_type).map(SolverType::Value),
            Type::Parameter(parameter_type) => Ok(SolverVariable::from(parameter_type).into()),
            Type::Self_ { .. } => todo!(),
            Type::Error => Err(()),
        }
    }
}
impl From<SolverType> for Type {
    fn from(type_: SolverType) -> Self {
        match type_ {
            SolverType::Variable(variable) => Type::Parameter(variable.type_.unwrap()),
            SolverType::Value(value) => Type::Named(NamedType {
                name: value.type_,
                type_arguments: value
                    .parameters
                    .into_vec()
                    .into_iter()
                    .map(Type::from)
                    .collect(),
            }),
        }
    }
}
impl Display for SolverType {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match self {
            Self::Variable(variable) => variable.fmt(f),
            Self::Value(value) => value.fmt(f),
        }
    }
}

/// A type variable that needs to be substituted.
///
/// These map to the type parameters in Candy. In the context of the solver,
/// they are displayed with a leading question mark: `?name`.
///
/// Examples:
///
/// * `?T`
/// * `?0`
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct SolverVariable {
    /// None stands for an error type.
    pub type_: Option<ParameterType>,
}
impl SolverVariable {
    #[must_use]
    pub fn self_() -> Self {
        Self {
            type_: Some(ParameterType {
                name: "Self".into(),
                id: TypeParameterId::SELF_TYPE,
            }),
        }
    }
    #[must_use]
    pub const fn new(type_: ParameterType) -> Self {
        Self { type_: Some(type_) }
    }
    #[must_use]
    pub const fn error() -> Self {
        Self { type_: None }
    }
}
impl SolverTypeTrait for SolverVariable {
    #[must_use]
    fn contains_variable(&self, variable: &SolverVariable) -> bool {
        self == variable
    }

    #[must_use]
    fn substitute_all(&self, substitutions: &FxHashMap<SolverVariable, SolverType>) -> SolverType {
        substitutions
            .get(self)
            .cloned()
            .unwrap_or_else(|| SolverType::Variable(self.clone()))
    }

    #[must_use]
    fn canonicalize_internal(
        &self,
        mapping: &mut FxHashMap<SolverVariable, SolverVariable>,
    ) -> Self {
        let mapping_len = mapping.len();
        mapping
            .entry(self.clone())
            .or_insert_with(|| Self {
                type_: Some(canonical_variable(mapping_len)),
            })
            .clone()
    }
}
impl From<ParameterType> for SolverVariable {
    fn from(type_: ParameterType) -> Self {
        Self::new(type_)
    }
}
impl Display for SolverVariable {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(
            f,
            "?{}",
            self.type_
                .as_ref()
                .map_or_else(|| "<error>".to_string(), ToString::to_string)
        )
    }
}

/// A concrete type. May contain `SolverVariable`s inside its `parameters`.
///
/// Examples:
///
/// * `List<?T>`
/// * `String`
/// * `Set<Int>`
/// * `Tuple<Int, String>`
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct SolverValue {
    pub type_: Box<str>,
    pub parameters: Box<[SolverType]>,
}
impl SolverTypeTrait for SolverValue {
    fn contains_variable(&self, variable: &SolverVariable) -> bool {
        self.parameters
            .iter()
            .any(|parameter| parameter.contains_variable(variable))
    }

    fn substitute_all(&self, substitutions: &FxHashMap<SolverVariable, SolverType>) -> SolverType {
        SolverType::Value(Self {
            type_: self.type_.clone(),
            parameters: self
                .parameters
                .iter()
                .map(|parameter| parameter.substitute_all(substitutions))
                .collect(),
        })
    }

    fn canonicalize_internal(
        &self,
        mapping: &mut FxHashMap<SolverVariable, SolverVariable>,
    ) -> Self {
        Self {
            type_: self.type_.clone(),
            parameters: self
                .parameters
                .iter()
                .map(|parameter| parameter.canonicalize_internal(mapping))
                .collect(),
        }
    }
}
impl TryFrom<NamedType> for SolverValue {
    type Error = ();

    fn try_from(type_: NamedType) -> Result<Self, Self::Error> {
        Ok(Self {
            type_: type_.name,
            parameters: type_
                .type_arguments
                .into_vec()
                .into_iter()
                .map(SolverType::try_from)
                .collect::<Result<Box<[SolverType]>, _>>()?,
        })
    }
}
impl Display for SolverValue {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "{}", self.type_)?;
        if !self.parameters.is_empty() {
            write!(f, "<{}>", self.parameters.iter().join(", "))?;
        }
        Ok(())
    }
}
