// SPDX-License-Identifier: MIT
// Copyright (c) 2026 kingchenc and the Wickra contributors
// Derived from wickra-lib/wickra; original files, license and Git blob IDs:
// vendor/wickra-r1/ and docs/evidence/reference-r1-source.json.
// Local changes: module paths, serde state, bounded parameters, selected API subset.
//! Average True Range (Wilder).

use crate::wickra::error::{Error, Result};
use crate::wickra::ohlcv::Candle;
use crate::wickra::traits::Indicator;

/// Average True Range with Wilder smoothing.
///
/// The first emitted value, by convention, appears after `period` candles: the
/// first `period − 1` true-range values seed the Wilder average alongside the
/// `period`-th, then the smoothed update begins.
///
/// # Example
///
/// ```
/// use roze_ta::wickra::{Candle, Indicator, Atr};
///
/// let mut indicator = Atr::new(5).unwrap();
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
pub struct Atr {
    pub(crate) period: usize,
    /// `period - 1` as `f64`, precomputed for the Wilder smoothing step.
    pub(crate) n_minus_1: f64,
    /// `1 / period`, precomputed so the per-tick smoothing multiplies instead of
    /// divides.
    pub(crate) inv_period: f64,
    pub(crate) prev_close: Option<f64>,
    pub(crate) seed_buf: Vec<f64>,
    /// Smoothed ATR, valid once `seeded` is set. Bare `f64` + flag rather than
    /// `Option<f64>` so the hot recurrence avoids an enum-tag read per tick.
    pub(crate) avg: f64,
    pub(crate) seeded: bool,
}

impl Atr {
    /// Construct an ATR with the given Wilder period.
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
            prev_close: None,
            seed_buf: Vec::with_capacity(period),
            avg: 0.0,
            seeded: false,
        })
    }

    /// Configured period.
    pub const fn period(&self) -> usize {
        self.period
    }

    /// Current value if available.
    pub const fn value(&self) -> Option<f64> {
        if self.seeded {
            Some(self.avg)
        } else {
            None
        }
    }
}

impl Indicator for Atr {
    type Input = Candle;
    type Output = f64;

    #[inline]
    fn update(&mut self, candle: Candle) -> Option<f64> {
        let tr = candle.true_range(self.prev_close);
        self.prev_close = Some(candle.close);

        if self.seeded {
            // Wilder smoothing with the reciprocal hoisted out of the hot path.
            let new_avg = self.avg.mul_add(self.n_minus_1, tr) * self.inv_period;
            self.avg = new_avg;
            return Some(new_avg);
        }

        self.seed_buf.push(tr);
        if self.seed_buf.len() == self.period {
            let seed = self.seed_buf.iter().copied().sum::<f64>() / self.period as f64;
            self.avg = seed;
            self.seeded = true;
            return Some(seed);
        }
        None
    }

    fn reset(&mut self) {
        self.prev_close = None;
        self.seed_buf.clear();
        self.avg = 0.0;
        self.seeded = false;
    }

    #[inline]
    fn warmup_period(&self) -> usize {
        self.period
    }

    #[inline]
    fn is_ready(&self) -> bool {
        self.seeded
    }

    #[inline]
    fn name(&self) -> &'static str {
        "ATR"
    }
}
