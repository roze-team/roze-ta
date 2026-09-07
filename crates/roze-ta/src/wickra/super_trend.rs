// SPDX-License-Identifier: MIT
// Copyright (c) 2026 kingchenc and the Wickra contributors
// Derived from wickra-lib/wickra; original files, license and Git blob IDs:
// vendor/wickra-r1/ and docs/evidence/reference-r1-source.json.
// Local changes: module paths, serde state, bounded parameters, selected API subset.
//! `SuperTrend`.

use crate::wickra::atr::Atr;
use crate::wickra::error::{Error, Result};
use crate::wickra::ohlcv::Candle;
use crate::wickra::traits::Indicator;

/// `SuperTrend` output: the trailing-stop level and the trend direction.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SuperTrendOutput {
    /// The `SuperTrend` line — the active trailing-stop level for this bar.
    pub value: f64,
    /// Trend direction: `+1.0` in an uptrend (the line sits below price),
    /// `-1.0` in a downtrend (the line sits above price).
    pub direction: f64,
}

/// Previous-bar state carried forward by the `SuperTrend` recurrence.
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub(crate) struct PrevState {
    pub(crate) final_upper: f64,
    pub(crate) final_lower: f64,
    pub(crate) close: f64,
    pub(crate) direction: f64,
}

/// `SuperTrend` — an ATR-banded trailing stop that flips sides on a close
/// through the band.
///
/// ```text
/// hl2          = (high + low) / 2
/// basic_upper  = hl2 + multiplier · ATR
/// basic_lower  = hl2 − multiplier · ATR
///
/// final_upper  = basic_upper  if basic_upper < prev_final_upper or prev_close > prev_final_upper
///                else prev_final_upper
/// final_lower  = basic_lower  if basic_lower > prev_final_lower or prev_close < prev_final_lower
///                else prev_final_lower
///
/// in a downtrend: stay down while close <= final_upper, else flip up
/// in an uptrend:  stay up   while close >= final_lower, else flip down
/// SuperTrend   = final_lower in an uptrend, final_upper in a downtrend
/// ```
///
/// The final bands ratchet — the upper band only moves down (and the lower
/// band only moves up) until price closes through it, which flips the trend
/// and hands the role of trailing stop to the opposite band. The first
/// ATR-ready bar seeds the trend as up. Wilder's classic configuration is
/// `ATR(10)` with a `3.0` multiplier.
///
/// # Example
///
/// ```
/// use roze_ta::wickra::{Candle, Indicator, SuperTrend};
///
/// let mut indicator = SuperTrend::classic();
/// let mut last = None;
/// for i in 0..80 {
///     let base = 100.0 + f64::from(i);
///     let candle =
///         Candle::new(base, base + 2.0, base - 2.0, base + 1.0, 10.0, i64::from(i)).unwrap();
///     last = indicator.update(candle);
/// }
/// assert!(last.is_some());
/// ```
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SuperTrend {
    pub(crate) atr: Atr,
    pub(crate) multiplier: f64,
    pub(crate) atr_period: usize,
    pub(crate) prev: Option<PrevState>,
}

impl SuperTrend {
    /// Construct a `SuperTrend` with an explicit ATR period and band multiplier.
    ///
    /// # Errors
    /// Returns [`Error::PeriodZero`] if `atr_period == 0` and
    /// [`Error::NonPositiveMultiplier`] if `multiplier` is not strictly
    /// positive and finite.
    pub fn new(atr_period: usize, multiplier: f64) -> Result<Self> {
        if !multiplier.is_finite() || multiplier <= 0.0 {
            return Err(Error::NonPositiveMultiplier);
        }
        Ok(Self {
            atr: Atr::new(atr_period)?,
            multiplier,
            atr_period,
            prev: None,
        })
    }

    /// Wilder's classic configuration: `ATR(10)` with a `3.0` multiplier.
    pub fn classic() -> Self {
        Self::new(10, 3.0).expect("classic SuperTrend params are valid")
    }

    /// Configured `(atr_period, multiplier)`.
    pub const fn params(&self) -> (usize, f64) {
        (self.atr_period, self.multiplier)
    }
}

impl Indicator for SuperTrend {
    type Input = Candle;
    type Output = SuperTrendOutput;

    fn update(&mut self, candle: Candle) -> Option<SuperTrendOutput> {
        let atr = self.atr.update(candle)?;
        let hl2 = f64::midpoint(candle.high, candle.low);
        let basic_upper = hl2 + self.multiplier * atr;
        let basic_lower = hl2 - self.multiplier * atr;

        let (final_upper, final_lower, direction) = match self.prev {
            None => {
                // First ATR-ready bar: no prior bands, seed the trend as up.
                (basic_upper, basic_lower, 1.0)
            }
            Some(p) => {
                let final_upper = if basic_upper < p.final_upper || p.close > p.final_upper {
                    basic_upper
                } else {
                    p.final_upper
                };
                let final_lower = if basic_lower > p.final_lower || p.close < p.final_lower {
                    basic_lower
                } else {
                    p.final_lower
                };
                let direction = if p.direction < 0.0 {
                    // Previous downtrend — the line was the upper band.
                    if candle.close <= final_upper {
                        -1.0
                    } else {
                        1.0
                    }
                } else {
                    // Previous uptrend — the line was the lower band.
                    if candle.close >= final_lower {
                        1.0
                    } else {
                        -1.0
                    }
                };
                (final_upper, final_lower, direction)
            }
        };

        let value = if direction > 0.0 {
            final_lower
        } else {
            final_upper
        };
        self.prev = Some(PrevState {
            final_upper,
            final_lower,
            close: candle.close,
            direction,
        });
        Some(SuperTrendOutput { value, direction })
    }

    fn reset(&mut self) {
        self.atr.reset();
        self.prev = None;
    }

    #[inline]
    fn warmup_period(&self) -> usize {
        self.atr_period
    }

    #[inline]
    fn is_ready(&self) -> bool {
        self.prev.is_some()
    }

    #[inline]
    fn name(&self) -> &'static str {
        "SuperTrend"
    }
}
