//! Selected Wickra Rust algorithms, retained under their original MIT license.
//! This low-level API preserves upstream neutral-value conventions. Use
//! `engine::Stream` for structured errors, strict undefined results, time
//! validation and versioned snapshots. See docs/contracts/reference-r1.md.
pub mod alma;
pub mod atr;
pub mod error;
pub mod ohlcv;
mod rolling_moments;
pub mod rsi;
pub mod stoch_rsi;
pub mod super_trend;
pub mod traits;
pub mod ulcer_index;
pub mod vortex;
pub use alma::Alma;
pub use atr::Atr;
pub use ohlcv::{Candle, Tick};
pub use rsi::Rsi;
pub use stoch_rsi::StochRsi;
pub use super_trend::{SuperTrend, SuperTrendOutput};
pub use traits::{BatchExt, Indicator};
pub use ulcer_index::UlcerIndex;
pub use vortex::{Vortex, VortexOutput};
