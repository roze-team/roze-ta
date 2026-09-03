// SPDX-License-Identifier: Apache-2.0
// Derived from Yata v0.7.0, src/lib.rs.
// Copyright 2020 AMvDev (amv-dev@protonmail.com).
// Modified 2026 by roze-ta contributors: extracted the native prelude module.
// See THIRD-PARTY-NOTICES.md and docs/patches/yata-native-migration.md.

//! Traits used to construct and update native indicators and methods.
//!
//! ## Method usage example
//!
//! ```
//! use roze_ta::prelude::*;
//! use roze_ta::methods::EMA;
//!
//! // EMA of length=3
//! let mut ema = EMA::new(3, &3.0).unwrap();
//!
//! ema.next(&3.0);
//! ema.next(&6.0);
//!
//! assert_eq!(ema.next(&9.0), 6.75);
//! assert_eq!(ema.next(&12.0), 9.375);
//! ```
//!
//! ## Indicator usage example
//!
//! ```
//! use roze_ta::helpers::{RandomCandles, MA};
//! use roze_ta::indicators::MACD;
//! use roze_ta::prelude::*;
//!
//! let mut candles = RandomCandles::new();
//! let mut macd = MACD::default();
//!
//! macd.ma1 = "sma-4".parse().unwrap(); // one way of defining methods inside indicators
//!
//! macd.signal = MA::TEMA(5); // another way of defining methods inside indicators
//!
//! let mut macd = macd.init(&candles.first()).unwrap();
//!
//! for candle in candles.take(10) {
//!     let result = macd.next(&candle);
//!
//!     println!("{:?}", result);
//! }
//! ```
//!

pub use super::core::{Candle, Error, IndicatorConfig, IndicatorInstance, Method, Sequence, OHLCV};

pub use super::helpers::{Buffered, Peekable};

/// Dynamically dispatchable traits for indicators creation
pub mod dd {
    pub use crate::core::{IndicatorConfigDyn, IndicatorInstanceDyn};
}
