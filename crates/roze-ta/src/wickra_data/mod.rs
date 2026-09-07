//! Pure, in-memory Wickra tick aggregation and candle resampling.
//! Source time units and explicit gap-fill behavior are preserved.
//! `flush` returns the open bar; it does not establish market close or as-of availability.
pub mod aggregator;
pub mod resample;

pub mod error {
    /// Recoverable errors from the pure data transformations.
    #[derive(Debug, thiserror::Error)]
    #[non_exhaustive]
    pub enum Error {
        #[error("invalid timeframe: {0}")]
        InvalidTimeframe(String),
        #[error("indicator-core error: {0}")]
        Core(#[from] crate::wickra_all::Error),
        #[error("malformed input: {0}")]
        Malformed(String),
    }
    pub type Result<T> = core::result::Result<T, Error>;
}
pub use error::{Error, Result};
