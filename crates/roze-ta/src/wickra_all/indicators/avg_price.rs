// Copyright (c) 2026 kingchenc and Wickra contributors.
// SPDX-License-Identifier: MIT
// Derived from the pinned Wickra baseline; see docs/patches/wickra-full-migration.md.
// Local changes: module paths, output serialization and dependency/test integration.

//! Average Price (AVGPRICE).

use crate::wickra_all::ohlcv::Candle;
use crate::wickra_all::traits::Indicator;

/// Average Price (`AVGPRICE`) — the bar's `(open + high + low + close) / 4`.
///
/// A per-bar price aggregate that, unlike [`TypicalPrice`](crate::wickra_all::TypicalPrice)
/// and [`WeightedClose`](crate::wickra_all::WeightedClose), folds in the open as well as the
/// high, low and close. As a stateless transform it emits a value from the very
/// first candle.
///
/// # Example
///
/// ```
/// use roze_ta::wickra_all::{Candle, Indicator, AvgPrice};
///
/// let mut indicator = AvgPrice::new();
/// let mut last = None;
/// for i in 0..80 {
///     let base = 100.0 + f64::from(i);
///     let candle =
///         Candle::new(base, base + 2.0, base - 2.0, base + 1.0, 10.0, i64::from(i)).unwrap();
///     last = indicator.update(candle);
/// }
/// assert!(last.is_some());
/// ```
#[derive(Debug, Clone, Default)]
pub struct AvgPrice {
    has_emitted: bool,
}

impl AvgPrice {
    /// Construct a new Average Price transform.
    pub const fn new() -> Self {
        Self { has_emitted: false }
    }
}

impl Indicator for AvgPrice {
    type Input = Candle;
    type Output = f64;

    #[inline]
    fn update(&mut self, candle: Candle) -> Option<f64> {
        self.has_emitted = true;
        Some(candle.avg_price())
    }

    fn reset(&mut self) {
        self.has_emitted = false;
    }

    #[inline]
    fn warmup_period(&self) -> usize {
        1
    }

    #[inline]
    fn is_ready(&self) -> bool {
        self.has_emitted
    }

    #[inline]
    fn name(&self) -> &'static str {
        "AVGPRICE"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;

    #[test]
    fn averages_the_four_prices() {
        // (open + high + low + close) / 4 = (10 + 14 + 6 + 12) / 4 = 10.5.
        let candle = Candle::new(10.0, 14.0, 6.0, 12.0, 1.0, 0).unwrap();
        let mut ap = AvgPrice::new();
        assert!(!ap.is_ready());
        assert_relative_eq!(ap.update(candle).unwrap(), 10.5, epsilon = 1e-12);
        assert!(ap.is_ready());
    }

    #[test]
    fn accessors_and_reset() {
        let mut ap = AvgPrice::new();
        assert_eq!(ap.name(), "AVGPRICE");
        assert_eq!(ap.warmup_period(), 1);
        let candle = Candle::new(10.0, 14.0, 6.0, 12.0, 1.0, 0).unwrap();
        let _ = ap.update(candle);
        assert!(ap.is_ready());
        ap.reset();
        assert!(!ap.is_ready());
    }
}
