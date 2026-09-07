// SPDX-License-Identifier: MIT
// Copyright (c) 2026 kingchenc and the Wickra contributors
// Derived from wickra-lib/wickra; original files, license and Git blob IDs:
// vendor/wickra-r1/ and docs/evidence/reference-r1-source.json.
// Local changes: module paths, serde state, bounded parameters, selected API subset.
//! Vortex Indicator.

use std::collections::VecDeque;

use crate::wickra::error::{Error, Result};
use crate::wickra::ohlcv::Candle;
use crate::wickra::traits::Indicator;

/// Vortex Indicator output: the two directional movement lines.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct VortexOutput {
    /// `VI+` — strength of upward (positive) vortex movement.
    pub plus: f64,
    /// `VI−` — strength of downward (negative) vortex movement.
    pub minus: f64,
}

/// Vortex Indicator — Botes & Siepman's pair of oscillators (`VI+`, `VI−`) that
/// capture the relationship between two consecutive bars.
///
/// Two "vortex movements" measure how far price travelled against the opposite
/// extreme of the previous bar; each is normalised by the summed true range:
///
/// ```text
/// VM+_t = |high_t − low_{t−1}|
/// VM−_t = |low_t  − high_{t−1}|
/// VI+   = Σ VM+ over n / Σ TR over n
/// VI−   = Σ VM− over n / Σ TR over n
/// ```
///
/// `VI+` crossing above `VI−` is a bullish signal, the reverse a bearish one;
/// the wider the gap, the stronger the trend. A fully flat window (zero true
/// range) reports `(0, 0)`.
///
/// # Example
///
/// ```
/// use roze_ta::wickra::{Candle, Indicator, Vortex};
///
/// let mut indicator = Vortex::new(14).unwrap();
/// let mut last = None;
/// for i in 0..80 {
///     let base = 100.0 + i as f64;
///     let candle =
///         Candle::new(base, base + 2.0, base - 2.0, base + 1.0, 10.0, i64::from(i)).unwrap();
///     last = indicator.update(candle);
/// }
/// assert!(last.is_some());
/// ```
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Vortex {
    pub(crate) period: usize,
    pub(crate) prev: Option<Candle>,
    /// Rolling window of `(VM+, VM−, TR)` triples.
    pub(crate) window: VecDeque<(f64, f64, f64)>,
    pub(crate) sum_vm_plus: f64,
    pub(crate) sum_vm_minus: f64,
    pub(crate) sum_tr: f64,
    pub(crate) last: Option<VortexOutput>,
}

impl Vortex {
    /// Construct a new Vortex Indicator with the given period.
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
            prev: None,
            window: VecDeque::with_capacity(period),
            sum_vm_plus: 0.0,
            sum_vm_minus: 0.0,
            sum_tr: 0.0,
            last: None,
        })
    }

    /// Configured period.
    pub const fn period(&self) -> usize {
        self.period
    }

    /// Current value if available.
    pub const fn value(&self) -> Option<VortexOutput> {
        self.last
    }
}

impl Indicator for Vortex {
    type Input = Candle;
    type Output = VortexOutput;

    #[inline]
    fn update(&mut self, candle: Candle) -> Option<VortexOutput> {
        let Some(prev) = self.prev else {
            // The first bar has no predecessor to measure against.
            self.prev = Some(candle);
            return None;
        };
        let vm_plus = (candle.high - prev.low).abs();
        let vm_minus = (candle.low - prev.high).abs();
        let tr = candle.true_range(Some(prev.close));
        self.prev = Some(candle);

        if self.window.len() == self.period {
            let (old_p, old_m, old_tr) = self.window.pop_front().expect("window is non-empty");
            self.sum_vm_plus -= old_p;
            self.sum_vm_minus -= old_m;
            self.sum_tr -= old_tr;
        }
        self.window.push_back((vm_plus, vm_minus, tr));
        self.sum_vm_plus += vm_plus;
        self.sum_vm_minus += vm_minus;
        self.sum_tr += tr;

        // Local: bounded recomputation avoids stale residuals after all ranges expire.
        self.sum_vm_plus = self.window.iter().map(|v| v.0).sum();
        self.sum_vm_minus = self.window.iter().map(|v| v.1).sum();
        self.sum_tr = self.window.iter().map(|v| v.2).sum();
        if self.window.len() < self.period {
            return None;
        }
        let out = if self.sum_tr == 0.0 {
            // A perfectly flat window has no range to normalise against.
            VortexOutput {
                plus: 0.0,
                minus: 0.0,
            }
        } else {
            VortexOutput {
                plus: self.sum_vm_plus / self.sum_tr,
                minus: self.sum_vm_minus / self.sum_tr,
            }
        };
        self.last = Some(out);
        Some(out)
    }

    fn reset(&mut self) {
        self.prev = None;
        self.window.clear();
        self.sum_vm_plus = 0.0;
        self.sum_vm_minus = 0.0;
        self.sum_tr = 0.0;
        self.last = None;
    }

    #[inline]
    fn warmup_period(&self) -> usize {
        // The first VM/TR triple needs a previous bar, then the window fills.
        self.period + 1
    }

    #[inline]
    fn is_ready(&self) -> bool {
        self.last.is_some()
    }

    #[inline]
    fn name(&self) -> &'static str {
        "Vortex"
    }
}
