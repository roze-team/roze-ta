// SPDX-License-Identifier: MIT
// Copyright (c) 2026 kingchenc and the Wickra contributors
// Derived from wickra-lib/wickra; original files, license and Git blob IDs:
// vendor/wickra-r1/ and docs/evidence/reference-r1-source.json.
// Local changes: module paths, serde state, bounded parameters, selected API subset.
//! OHLCV value types: candles and ticks.

use crate::wickra::error::{Error, Result};

/// A single OHLCV bar.
///
/// Timestamps are unitless `i64` values so callers can use whatever epoch resolution
/// they prefer (milliseconds, microseconds, seconds…). Wickra never inspects them
/// numerically beyond passing them through.
///
/// # Construction and the limits of its guarantee
///
/// The struct is `#[non_exhaustive]`, so code outside this crate cannot build
/// one from a field literal and must go through [`new`](Self::new), which
/// validates, or [`new_unchecked`](Self::new_unchecked), which is an explicit
/// opt-out for values already known to be sound.
///
/// The fields stay public because reading them is by far the common operation
/// and an accessor on each would buy nothing. That does mean a validated value
/// can still be *written* into an invalid state afterwards, and nothing detects
/// it: the indicators that consume this type rely on the constructor's
/// guarantee rather than re-checking every bar. Treat a mutation the way you
/// would treat `new_unchecked` — you are asserting the invariants still hold.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
#[non_exhaustive]
pub struct Candle {
    /// Bar open price.
    pub open: f64,
    /// Bar high price.
    pub high: f64,
    /// Bar low price.
    pub low: f64,
    /// Bar close price.
    pub close: f64,
    /// Bar volume.
    pub volume: f64,
    /// Bar timestamp (caller-defined epoch / resolution).
    pub timestamp: i64,
}

impl Candle {
    /// Construct a new candle, validating the OHLC relationships and finiteness.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidCandle`] if any of these invariants are violated:
    /// - `high >= max(open, close, low)`
    /// - `low  <= min(open, close, high)`
    /// - all of `open`, `high`, `low`, `close`, `volume` are finite
    /// - `volume >= 0`
    pub fn new(
        open: f64,
        high: f64,
        low: f64,
        close: f64,
        volume: f64,
        timestamp: i64,
    ) -> Result<Self> {
        if !(open.is_finite() && high.is_finite() && low.is_finite() && close.is_finite()) {
            return Err(Error::InvalidCandle {
                message: "open, high, low, close must all be finite",
            });
        }
        if !volume.is_finite() {
            return Err(Error::InvalidCandle {
                message: "volume must be finite",
            });
        }
        if volume < 0.0 {
            return Err(Error::InvalidCandle {
                message: "volume must be non-negative",
            });
        }
        if high < low {
            return Err(Error::InvalidCandle {
                message: "high must be >= low",
            });
        }
        if high < open || high < close {
            return Err(Error::InvalidCandle {
                message: "high must be >= open and >= close",
            });
        }
        if low > open || low > close {
            return Err(Error::InvalidCandle {
                message: "low must be <= open and <= close",
            });
        }
        Ok(Self {
            open,
            high,
            low,
            close,
            volume,
            timestamp,
        })
    }

    /// Construct a candle without validation. The caller asserts that all OHLC
    /// invariants hold and that no field is NaN or infinite.
    pub const fn new_unchecked(
        open: f64,
        high: f64,
        low: f64,
        close: f64,
        volume: f64,
        timestamp: i64,
    ) -> Self {
        Self {
            open,
            high,
            low,
            close,
            volume,
            timestamp,
        }
    }

    /// The typical price `(high + low + close) / 3`. Used by CCI, MFI, VWAP, etc.
    #[inline]
    pub fn typical_price(&self) -> f64 {
        (self.high + self.low + self.close) / 3.0
    }

    /// The mid price `(high + low) / 2`.
    #[inline]
    pub fn median_price(&self) -> f64 {
        f64::midpoint(self.high, self.low)
    }

    /// The weighted close `(high + low + 2*close) / 4`.
    #[inline]
    pub fn weighted_close(&self) -> f64 {
        (self.high + self.low + 2.0 * self.close) / 4.0
    }

    /// The average price `(open + high + low + close) / 4`.
    #[inline]
    pub fn avg_price(&self) -> f64 {
        (self.open + self.high + self.low + self.close) / 4.0
    }

    /// True range of this candle relative to a previous close: `max(H-L, |H-prev|, |L-prev|)`.
    /// If no previous close is supplied, falls back to `high - low`.
    #[inline]
    pub fn true_range(&self, prev_close: Option<f64>) -> f64 {
        let hl = self.high - self.low;
        match prev_close {
            Some(prev) => {
                let hp = (self.high - prev).abs();
                let lp = (self.low - prev).abs();
                hl.max(hp).max(lp)
            }
            None => hl,
        }
    }
}

/// A single trade tick.
///
/// # Construction and the limits of its guarantee
///
/// The struct is `#[non_exhaustive]`, so code outside this crate cannot build
/// one from a field literal and must go through [`new`](Self::new), which
/// validates. A tick has no unchecked constructor.
///
/// The fields stay public because reading them is by far the common operation
/// and an accessor on each would buy nothing. That does mean a validated value
/// can still be *written* into an invalid state afterwards, and nothing detects
/// it: the code that consumes this type relies on the constructor's guarantee
/// rather than re-checking. A mutation is an assertion that the invariants
/// still hold.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
#[non_exhaustive]
pub struct Tick {
    /// Trade price.
    pub price: f64,
    /// Trade size.
    pub volume: f64,
    /// Trade timestamp (caller-defined epoch / resolution).
    pub timestamp: i64,
}

impl Tick {
    /// Construct a new tick, validating finiteness and non-negativity of volume.
    ///
    /// # Errors
    ///
    /// Returns [`Error::NonFiniteInput`] if `price` or `volume` is NaN or infinite,
    /// or [`Error::InvalidTick`] for `volume < 0`. (Audit finding R14 — previously
    /// returned [`Error::InvalidCandle`], which is semantically wrong for a tick.)
    pub fn new(price: f64, volume: f64, timestamp: i64) -> Result<Self> {
        if !price.is_finite() || !volume.is_finite() {
            return Err(Error::NonFiniteInput);
        }
        if volume < 0.0 {
            return Err(Error::InvalidTick {
                message: "tick volume must be non-negative",
            });
        }
        Ok(Self {
            price,
            volume,
            timestamp,
        })
    }
}
