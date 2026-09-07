// SPDX-License-Identifier: MIT
// Copyright (c) 2026 kingchenc and the Wickra contributors
// Derived from wickra-lib/wickra; original files, license and Git blob IDs:
// vendor/wickra-r1/ and docs/evidence/reference-r1-source.json.
// Local changes: module paths, serde state, bounded parameters, selected API subset.
//! Ulcer Index.

use std::collections::VecDeque;

use crate::wickra::error::{Error, Result};
use crate::wickra::rolling_moments::RollingSum;
use crate::wickra::traits::Indicator;

/// Ulcer Index — Peter Martin's downside-only volatility / risk measure.
///
/// Standard deviation punishes upside and downside moves equally; the Ulcer
/// Index measures only the **pain of drawdowns**. For each bar it computes the
/// percentage drop from the highest price of the trailing window, squares it,
/// and reports the root-mean-square over the window:
///
/// ```text
/// drawdown_t = 100 · (price_t − max(price, period)_t) / max(price, period)_t
/// UlcerIndex = √( mean( drawdown² over period ) )
/// ```
///
/// A pure up-trend never trades below its own running high, so its Ulcer Index
/// is `0`; the deeper and longer the drawdowns, the higher the reading. It is
/// the volatility measure of choice for risk-adjusted return ratios (the
/// "Martin ratio" / UPI).
///
/// Each `update` is amortised O(1): the trailing maximum is tracked with a
/// monotonically-decreasing deque of `(index, price)` pairs, so the indicator
/// honours the `Indicator` trait's O(1)-per-tick contract even for long
/// windows.
///
/// # Example
///
/// ```
/// use roze_ta::wickra::{Indicator, UlcerIndex};
///
/// let mut indicator = UlcerIndex::new(14).unwrap();
/// let mut last = None;
/// for i in 0..80 {
///     last = indicator.update(100.0 + (f64::from(i) * 0.3).sin() * 8.0);
/// }
/// assert!(last.is_some());
/// ```
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct UlcerIndex {
    pub(crate) period: usize,
    /// 1-based count of finite inputs seen so far; used as the monotonic index
    /// that expires entries from `max_dq`.
    pub(crate) count: u64,
    /// Monotonically-decreasing deque of `(index, price)` over the trailing
    /// `period` inputs. The front holds the current trailing maximum in O(1).
    pub(crate) max_dq: VecDeque<(u64, f64)>,
    /// Rolling window of the last `period` squared percentage drawdowns.
    pub(crate) drawdowns_sq: VecDeque<f64>,
    pub(crate) sum_sq: RollingSum,
    pub(crate) last: Option<f64>,
}

impl UlcerIndex {
    /// Construct a new Ulcer Index with the given period.
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
            count: 0,
            max_dq: VecDeque::with_capacity(period),
            drawdowns_sq: VecDeque::with_capacity(period),
            sum_sq: RollingSum::new(),
            last: None,
        })
    }

    /// Configured period.
    pub const fn period(&self) -> usize {
        self.period
    }

    /// Current value if available.
    pub const fn value(&self) -> Option<f64> {
        self.last
    }
}

impl Indicator for UlcerIndex {
    type Input = f64;
    type Output = f64;

    fn update(&mut self, input: f64) -> Option<f64> {
        if !input.is_finite() {
            // Non-finite input is ignored; state is left untouched.
            return None;
        }
        self.count += 1;
        // Drop tail entries that can never be the trailing max again — every
        // entry `≤ input` is dominated by `input` and at least as old.
        while let Some(&(_, back)) = self.max_dq.back() {
            if back <= input {
                self.max_dq.pop_back();
            } else {
                break;
            }
        }
        self.max_dq.push_back((self.count, input));
        // Expire the head once it falls out of the trailing `period`-window.
        let window_lo = self.count.saturating_sub(self.period as u64 - 1);
        while let Some(&(idx, _)) = self.max_dq.front() {
            if idx < window_lo {
                self.max_dq.pop_front();
            } else {
                break;
            }
        }
        if self.count < self.period as u64 {
            return None;
        }
        // Front is the trailing max in O(1).
        let max_price = self.max_dq.front().expect("non-empty").1;
        let drawdown = if max_price == 0.0 {
            0.0
        } else {
            100.0 * (input - max_price) / max_price
        };
        let sq = drawdown * drawdown;

        if self.drawdowns_sq.len() == self.period {
            let oldest = self.drawdowns_sq.pop_front().expect("window is non-empty");
            self.sum_sq.evict(oldest);
        }
        self.drawdowns_sq.push_back(sq);
        self.sum_sq.push(sq);
        if self.sum_sq.needs_reseed(1) {
            self.sum_sq.reseed(self.drawdowns_sq.iter().copied());
        }
        if self.drawdowns_sq.len() < self.period {
            return None;
        }
        let ui = (self.sum_sq.value() / self.period as f64).sqrt();
        self.last = Some(ui);
        Some(ui)
    }

    fn reset(&mut self) {
        self.count = 0;
        self.max_dq.clear();
        self.drawdowns_sq.clear();
        self.sum_sq.reset();
        self.last = None;
    }

    #[inline]
    fn warmup_period(&self) -> usize {
        // `period` inputs fill the trailing-max window; the first drawdown is
        // computable on bar `period` (the window is full for the first time);
        // another `period - 1` drawdowns then fill the RMS window. The two
        // windows overlap by one bar, so `warmup_period() == 2 * period - 1`.
        2 * self.period - 1
    }

    #[inline]
    fn is_ready(&self) -> bool {
        self.last.is_some()
    }

    #[inline]
    fn name(&self) -> &'static str {
        "UlcerIndex"
    }
}
