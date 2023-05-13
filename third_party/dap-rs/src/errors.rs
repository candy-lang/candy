use std::fmt::Debug;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum DeserializationError {
    #[error("could not parse value '{value}' to enum variant of '{enum_name}'")]
    StringToEnumParseError { enum_name: String, value: String },
    #[error("Error while deserializing")]
    SerdeError(#[from] serde_json::Error),
}

#[derive(Debug, Error)]
pub enum ServerError {
    #[error("I/O error")]
    IoError(#[from] std::io::Error),

    #[error("Unknown header: {header}")]
    UnknownHeader { header: String },

    #[error("Parse error")]
    ParseError(#[from] DeserializationError),

    #[error("Could not parse header line '{line}'")]
    HeaderParseError { line: String },

    #[error("Network error. {reason}")]
    NetworkError { reason: String },

    #[error("Protocol error. {reason}")]
    ProtocolError { reason: String },
}
