use serde::{Deserialize, Serialize};

#[derive(PartialEq, Eq, Hash, Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TracingConfig {
    pub register_fuzzables: TracingMode,
    pub calls: TracingMode,
    pub evaluated_expressions: TracingMode,
}
impl TracingConfig {
    #[must_use]
    pub const fn off() -> Self {
        Self {
            register_fuzzables: TracingMode::Off,
            calls: TracingMode::Off,
            evaluated_expressions: TracingMode::Off,
        }
    }

    #[must_use]
    pub const fn for_child_module(&self) -> Self {
        Self {
            register_fuzzables: self.register_fuzzables.for_child_module(),
            calls: self.calls.for_child_module(),
            evaluated_expressions: self.evaluated_expressions.for_child_module(),
        }
    }
}

#[derive(PartialEq, Eq, Hash, Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TracingMode {
    Off,

    /// Traces the module that's the root of the compilation and no child
    /// modules.
    OnlyCurrent,

    All,
}
impl TracingMode {
    #[must_use]
    pub const fn all_or_off(should_trace: bool) -> Self {
        if should_trace {
            Self::All
        } else {
            Self::Off
        }
    }

    #[must_use]
    pub const fn only_current_or_off(should_trace: bool) -> Self {
        if should_trace {
            Self::OnlyCurrent
        } else {
            Self::Off
        }
    }

    #[must_use]
    pub const fn is_enabled(&self) -> bool {
        match self {
            Self::Off => false,
            Self::OnlyCurrent => true,
            Self::All => true,
        }
    }

    #[must_use]
    pub const fn for_child_module(&self) -> Self {
        match self {
            Self::Off => Self::Off,
            Self::OnlyCurrent => Self::Off,
            Self::All => Self::All,
        }
    }
}
