// SPDX-License-Identifier: Apache-2.0
// Derived from Yata v0.7.0, src/core/errors.rs.
// Copyright 2020 AMvDev (amv-dev@protonmail.com).
// Modified 2026 by roze-ta contributors: native module integration, fixed types,
// unconditional serialization, safe indexing and Rust formatting.
// See THIRD-PARTY-NOTICES.md and docs/patches/yata-native-migration.md.

/// Crate errors enum
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum Error {
    /// Error parsing string to [`Source`](crate::core::Source)
    SourceParse(String),

    /// Error parsing indicator parameter
    ParameterParse(String, String),

    /// Error parsing moving average
    MovingAverageParse,

    /// Invalid parameters for method creation
    WrongMethodParameters,

    /// Invalid indicator config error
    WrongConfig,

    /// Invalid candles error
    InvalidCandles,

    /// Any other error
    Other(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SourceParse(value) => write!(f, "Unable to parse value as Source: {value:?}"),
            Self::ParameterParse(name, value) => {
                write!(f, "Unable to parse into {name}: {value:?}")
            }
            Self::WrongMethodParameters => write!(f, "Wrong method parameters"),
            Self::WrongConfig => write!(f, "Wrong config"),
            Self::InvalidCandles => write!(f, "Invalid candles"),
            Self::Other(reason) => f.write_str(reason),
            Self::MovingAverageParse => write!(f, "Error parsing moving average type and length"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        None
    }
}
