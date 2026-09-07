// SPDX-License-Identifier: MIT
// Copyright (c) 2026 kingchenc and the Wickra contributors
// Derived from wickra-lib/wickra; original files, license and Git blob IDs:
// vendor/wickra-r1/ and docs/evidence/reference-r1-source.json.
// Local changes: module paths, serde state, bounded parameters, selected API subset.
//! Relative Strength Index using Wilder's smoothing.

use crate::wickra::error::{Error, Result};
use crate::wickra::traits::Indicator;

/// Relative Strength Index (Wilder, 1978).
///
/// Uses Wilder's smoothing (an EMA with `alpha = 1 / period`). The first output
/// is produced after `period + 1` inputs: the seed averages the first `period`
/// gains and losses, and the first emitted RSI corresponds to the input at
/// index `period`.
///
/// # Example
///
/// ```
/// use roze_ta::wickra::{Indicator, Rsi};
///
/// let mut indicator = Rsi::new(3).unwrap();
/// let mut last = None;
/// for i in 0..80 {
///     last = indicator.update(100.0 + f64::from(i));
/// }
/// assert!(last.is_some());
/// ```
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Rsi {
    pub(crate) period: usize,
    /// `period - 1` as `f64`, precomputed for the Wilder smoothing step.
    pub(crate) n_minus_1: f64,
    /// `1 / period`, precomputed so the per-tick smoothing multiplies instead of
    /// divides (a reciprocal is hoisted out of the hot path).
    pub(crate) inv_period: f64,
    /// Previous close, valid once `has_prev` is set. Bare `f64` + flag instead of
    /// `Option<f64>` to avoid an enum-tag read on every tick.
    pub(crate) prev_close: f64,
    pub(crate) has_prev: bool,
    // Wilder seeds with the simple average of the first `period` gains/losses,
    // then transitions to recursive smoothing.
    pub(crate) seed_buf_gains: Vec<f64>,
    pub(crate) seed_buf_losses: Vec<f64>,
    /// Smoothed average gain / loss, valid once `avgs_seeded` is set. Bare `f64`s
    /// + flag so the hot recurrence avoids reading two `Option<f64>` tags per tick.
    pub(crate) avg_gain: f64,
    pub(crate) avg_loss: f64,
    pub(crate) avgs_seeded: bool,
    pub(crate) last_value: Option<f64>,
}

impl Rsi {
    /// Construct an RSI with the given Wilder period.
    ///
    /// # Errors
    ///
    /// Returns [`Error::PeriodZero`] if `period == 0`.
    pub fn new(period: usize) -> Result<Self> {
        if period == 0 {
            return Err(Error::PeriodZero);
        }
        if period > crate::wickra::error::MAX_PERIOD {
            return Err(Error::InvalidPeriod {
                message: crate::wickra::error::PERIOD_ABOVE_MAX,
            });
        }
        Ok(Self {
            period,
            n_minus_1: (period - 1) as f64,
            inv_period: 1.0 / period as f64,
            prev_close: 0.0,
            has_prev: false,
            seed_buf_gains: Vec::with_capacity(period),
            seed_buf_losses: Vec::with_capacity(period),
            avg_gain: 0.0,
            avg_loss: 0.0,
            avgs_seeded: false,
            last_value: None,
        })
    }

    /// Configured period.
    pub const fn period(&self) -> usize {
        self.period
    }

    /// Current value if available.
    pub const fn value(&self) -> Option<f64> {
        self.last_value
    }

    fn rsi_from_avgs(avg_gain: f64, avg_loss: f64) -> f64 {
        // Algebraically `100 - 100/(1 + ag/al)` collapses to `100·ag/(ag+al)`,
        // which needs a single division instead of two and removes the separate
        // `rs` step. Edge cases stay exact: `al == 0, ag > 0` gives `100·ag/ag =
        // 100`; `ag == 0, al > 0` gives `0`; both zero (no movement) is the
        // undefined case and returns the neutral 50.
        let denom = avg_gain + avg_loss;
        if denom == 0.0 {
            50.0
        } else {
            100.0 * avg_gain / denom
        }
    }
}

impl Indicator for Rsi {
    type Input = f64;
    type Output = f64;

    fn update(&mut self, input: f64) -> Option<f64> {
        if !input.is_finite() {
            return None;
        }

        if !self.has_prev {
            self.prev_close = input;
            self.has_prev = true;
            return None;
        }
        let prev = self.prev_close;
        self.prev_close = input;

        let diff = input - prev;
        let gain = if diff > 0.0 { diff } else { 0.0 };
        let loss = if diff < 0.0 { -diff } else { 0.0 };

        if self.avgs_seeded {
            // Wilder smoothing `(prev·(n-1) + x) / n` with the reciprocal hoisted:
            // a fused multiply-add then a multiply by `1/n`, no per-tick division.
            let new_ag = self.avg_gain.mul_add(self.n_minus_1, gain) * self.inv_period;
            let new_al = self.avg_loss.mul_add(self.n_minus_1, loss) * self.inv_period;
            self.avg_gain = new_ag;
            self.avg_loss = new_al;
            let v = Self::rsi_from_avgs(new_ag, new_al);
            self.last_value = Some(v);
            return Some(v);
        }

        self.seed_buf_gains.push(gain);
        self.seed_buf_losses.push(loss);
        if self.seed_buf_gains.len() == self.period {
            let ag = self.seed_buf_gains.iter().sum::<f64>() / self.period as f64;
            let al = self.seed_buf_losses.iter().sum::<f64>() / self.period as f64;
            self.avg_gain = ag;
            self.avg_loss = al;
            self.avgs_seeded = true;
            let v = Self::rsi_from_avgs(ag, al);
            self.last_value = Some(v);
            return Some(v);
        }
        None
    }

    fn reset(&mut self) {
        self.prev_close = 0.0;
        self.has_prev = false;
        self.seed_buf_gains.clear();
        self.seed_buf_losses.clear();
        self.avg_gain = 0.0;
        self.avg_loss = 0.0;
        self.avgs_seeded = false;
        self.last_value = None;
    }

    #[inline]
    fn warmup_period(&self) -> usize {
        self.period + 1
    }

    #[inline]
    fn is_ready(&self) -> bool {
        self.last_value.is_some()
    }

    #[inline]
    fn name(&self) -> &'static str {
        "RSI"
    }
}
