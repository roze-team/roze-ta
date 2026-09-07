// SPDX-License-Identifier: MIT
// Copyright (c) 2026 kingchenc and the Wickra contributors
// Derived from wickra-lib/wickra; original files, license and Git blob IDs:
// vendor/wickra-r1/ and docs/evidence/reference-r1-source.json.
// Local changes: module paths, serde state, bounded parameters, selected API subset.
//! Stochastic RSI.

use std::collections::VecDeque;

use crate::wickra::error::{Error, Result};
use crate::wickra::traits::Indicator;

use super::Rsi;

/// Stochastic RSI — the Stochastic Oscillator formula applied to the RSI series
/// instead of to price.
///
/// RSI itself rarely reaches its `[0, 100]` extremes, so it spends most of its
/// life bunched in the middle of the range. `StochRSI` re-scales it: it reports
/// where the *current* RSI sits within its own high/low range over the last
/// `stoch_period` bars, which makes overbought/oversold turns far easier to
/// see.
///
/// ```text
/// StochRSI = 100 · (RSI − min(RSI, stoch_period)) / (max(RSI, …) − min(RSI, …))
/// ```
///
/// The output is bounded in `[0, 100]`. A flat RSI window (zero range) is
/// reported as the neutral `50.0`, matching the [`Stochastic`](crate::wickra::StochRsi)
/// convention.
///
/// # Example
///
/// ```
/// use roze_ta::wickra::{Indicator, StochRsi};
///
/// let mut indicator = StochRsi::new(14, 14).unwrap();
/// let mut last = None;
/// for i in 0..80 {
///     last = indicator.update(100.0 + (f64::from(i) * 0.5).sin() * 10.0);
/// }
/// assert!(last.is_some());
/// ```
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct StochRsi {
    pub(crate) rsi_period: usize,
    pub(crate) stoch_period: usize,
    pub(crate) rsi: Rsi,
    /// Rolling window of the last `stoch_period` RSI values.
    pub(crate) window: VecDeque<f64>,
    pub(crate) last: Option<f64>,
}

impl StochRsi {
    /// Construct a new `StochRSI` with the RSI period and the stochastic lookback.
    ///
    /// # Errors
    ///
    /// Returns [`Error::PeriodZero`] if either period is `0`.
    pub fn new(rsi_period: usize, stoch_period: usize) -> Result<Self> {
        if rsi_period == 0 || stoch_period == 0 {
            return Err(Error::PeriodZero);
        }
        if stoch_period > crate::wickra::error::MAX_PERIOD {
            return Err(Error::InvalidPeriod {
                message: crate::wickra::error::PERIOD_ABOVE_MAX,
            });
        }
        Ok(Self {
            rsi_period,
            stoch_period,
            rsi: Rsi::new(rsi_period)?,
            window: VecDeque::with_capacity(stoch_period),
            last: None,
        })
    }

    /// The `(rsi_period, stoch_period)` pair.
    pub const fn periods(&self) -> (usize, usize) {
        (self.rsi_period, self.stoch_period)
    }

    /// Current value if available.
    pub const fn value(&self) -> Option<f64> {
        self.last
    }
}

impl Indicator for StochRsi {
    type Input = f64;
    type Output = f64;

    #[inline]
    fn update(&mut self, input: f64) -> Option<f64> {
        if !input.is_finite() {
            // Non-finite input is ignored; state is left untouched.
            return None;
        }
        let rsi_value = self.rsi.update(input)?;

        if self.window.len() == self.stoch_period {
            self.window.pop_front();
        }
        self.window.push_back(rsi_value);
        if self.window.len() < self.stoch_period {
            return None;
        }

        let max = self
            .window
            .iter()
            .copied()
            .fold(f64::NEG_INFINITY, f64::max);
        let min = self.window.iter().copied().fold(f64::INFINITY, f64::min);
        let range = max - min;
        let stoch = if range == 0.0 {
            // Flat RSI window: report the neutral midpoint.
            50.0
        } else {
            100.0 * (rsi_value - min) / range
        };
        self.last = Some(stoch);
        Some(stoch)
    }

    fn reset(&mut self) {
        self.rsi.reset();
        self.window.clear();
        self.last = None;
    }

    #[inline]
    fn warmup_period(&self) -> usize {
        // RSI emits its first value at input `rsi_period + 1`; the stochastic
        // window then needs `stoch_period` RSI values.
        self.rsi_period + self.stoch_period
    }

    #[inline]
    fn is_ready(&self) -> bool {
        self.last.is_some()
    }

    #[inline]
    fn name(&self) -> &'static str {
        "StochRSI"
    }
}
