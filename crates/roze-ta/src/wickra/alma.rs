// SPDX-License-Identifier: MIT
// Copyright (c) 2026 kingchenc and the Wickra contributors
// Derived from wickra-lib/wickra; original files, license and Git blob IDs:
// vendor/wickra-r1/ and docs/evidence/reference-r1-source.json.
// Local changes: module paths, serde state, bounded parameters, selected API subset.
//! Arnaud Legoux Moving Average (ALMA).

use std::collections::VecDeque;

use crate::wickra::error::{Error, Result};
use crate::wickra::traits::Indicator;

/// Arnaud Legoux Moving Average — a Gaussian-weighted moving average.
///
/// Each output is a weighted sum of the last `period` inputs:
///
/// ```text
/// w[i] = exp(-(i - m)^2 / (2 * s^2))   for i in 0..period
/// m    = offset * (period - 1)
/// s    = period / sigma
/// ALMA = sum(price[i] * w[i]) / sum(w[i])
/// ```
///
/// The Gaussian is centred on the relative index `offset * (period - 1)`, so
/// `offset = 0.85` puts the peak near the newest sample (responsive), while
/// `offset = 0.5` centres the peak in the middle of the window (smooth).
/// `sigma` controls how concentrated the Gaussian is: larger `sigma` ->
/// narrower kernel, smaller `sigma` -> broader (closer to SMA).
///
/// Reference: Arnaud Legoux and Dimitrios Kouzis-Loukas, 2009.
///
/// # Defaults
///
/// The community-standard parameters are `period = 9`, `offset = 0.85`,
/// `sigma = 6.0`. The first output lands after exactly `period` inputs.
///
/// # Example
///
/// ```
/// use roze_ta::wickra::{Alma, Indicator};
///
/// let mut alma = Alma::new(9, 0.85, 6.0).unwrap();
/// let mut last = None;
/// for i in 0..40 {
///     last = alma.update(100.0 + f64::from(i));
/// }
/// assert!(last.is_some());
/// ```
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Alma {
    pub(crate) period: usize,
    pub(crate) offset: f64,
    pub(crate) sigma: f64,
    /// Pre-computed, normalised weights (sum to 1). `weights[0]` is the oldest
    /// sample in the window, `weights[period - 1]` the newest.
    pub(crate) weights: Vec<f64>,
    pub(crate) window: VecDeque<f64>,
    pub(crate) current: Option<f64>,
}

impl Alma {
    /// Construct a new ALMA with the given period, offset and sigma.
    ///
    /// # Errors
    ///
    /// - [`Error::PeriodZero`] if `period == 0`.
    /// - [`Error::InvalidPeriod`] if `offset` is outside `[0.0, 1.0]` or
    ///   `sigma <= 0.0` or either of `offset` / `sigma` is non-finite.
    pub fn new(period: usize, offset: f64, sigma: f64) -> Result<Self> {
        if period == 0 {
            return Err(Error::PeriodZero);
        }
        if period > crate::wickra::error::MAX_PERIOD {
            return Err(Error::InvalidPeriod {
                message: crate::wickra::error::PERIOD_ABOVE_MAX,
            });
        }
        if !offset.is_finite() || !(0.0..=1.0).contains(&offset) {
            return Err(Error::InvalidPeriod {
                message: "ALMA offset must be a finite value in [0, 1]",
            });
        }
        if !sigma.is_finite() || sigma <= 0.0 {
            return Err(Error::InvalidPeriod {
                message: "ALMA sigma must be a finite positive value",
            });
        }
        let m = offset * (period as f64 - 1.0);
        let s = period as f64 / sigma;
        let denom = 2.0 * s * s;
        // The raw Gaussian weights sum to a strictly positive value because
        // every term is `exp(_) > 0`, so the normalisation below cannot divide
        // by zero.
        let mut raw: Vec<f64> = (0..period)
            .map(|i| (-((i as f64 - m).powi(2)) / denom).exp())
            .collect();
        let sum: f64 = raw.iter().sum();
        if !sum.is_finite() || sum <= 0.0 {
            return Err(Error::InvalidPeriod {
                message: "ALMA weights are not representable",
            });
        }
        for w in &mut raw {
            *w /= sum;
        }
        Ok(Self {
            period,
            offset,
            sigma,
            weights: raw,
            window: VecDeque::with_capacity(period),
            current: None,
        })
    }

    /// Construct ALMA with the community-standard parameters
    /// `(period = 9, offset = 0.85, sigma = 6.0)`.
    pub fn classic() -> Self {
        Self::new(9, 0.85, 6.0).expect("classic ALMA parameters are valid")
    }

    /// Configured period.
    pub const fn period(&self) -> usize {
        self.period
    }

    /// Configured offset.
    pub const fn offset(&self) -> f64 {
        self.offset
    }

    /// Configured sigma.
    pub const fn sigma(&self) -> f64 {
        self.sigma
    }
}

impl Indicator for Alma {
    type Input = f64;
    type Output = f64;

    #[inline]
    fn update(&mut self, input: f64) -> Option<f64> {
        if !input.is_finite() {
            return None;
        }
        if self.window.len() == self.period {
            self.window.pop_front();
        }
        self.window.push_back(input);
        if self.window.len() < self.period {
            return None;
        }
        let mut acc = 0.0;
        for (w, p) in self.weights.iter().zip(self.window.iter()) {
            acc += w * p;
        }
        self.current = Some(acc);
        Some(acc)
    }

    fn reset(&mut self) {
        self.window.clear();
        self.current = None;
    }

    #[inline]
    fn warmup_period(&self) -> usize {
        self.period
    }

    #[inline]
    fn is_ready(&self) -> bool {
        self.current.is_some()
    }

    #[inline]
    fn name(&self) -> &'static str {
        "ALMA"
    }
}
