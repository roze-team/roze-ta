//! Stable domain errors for the versioned engine and protocol adapters.
use serde::{Deserialize, Serialize};
use std::{error::Error, fmt};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    InvalidIdentity,
    InvalidBar,
    InvalidTime,
    DuplicateOrUnorderedBar,
    LimitExceeded,
    UnsupportedProfile,
    DuplicateProfile,
    IncompatibleSnapshot,
    CorruptSnapshot,
    EncodingFailed,
    UpstreamFailure,
    NotReady,
    UndefinedResult,
    Cancelled,
    TimedOut,
    InvalidParameter,
    InvalidSample,
    DuplicateSample,
    InsufficientData,
    NumericalFailure,
    IncompatibleArtifact,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct TaError {
    pub code: ErrorCode,
    pub message: String,
}
impl TaError {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}
impl fmt::Display for TaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}
impl Error for TaError {}
