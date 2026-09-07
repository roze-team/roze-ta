// Copyright (c) 2026 kingchenc and Wickra contributors.
// SPDX-License-Identifier: MIT
// Derived from the pinned Wickra baseline; see docs/patches/wickra-full-migration.md.
// Local changes: module paths, output serialization and dependency/test integration.

//! TICK Index — instantaneous net advancing-minus-declining issues.

use crate::wickra_all::cross_section::CrossSection;
use crate::wickra_all::traits::Indicator;

/// TICK Index — the instantaneous net of advancing minus declining issues across
/// a universe, `advancers - decliners`.
///
/// Unlike the cumulative [`AdvanceDecline`](crate::wickra_all::AdvanceDecline) line, the TICK
/// is *not* accumulated: each tick reports the breadth of that snapshot alone. It
/// oscillates around zero — strongly positive readings mean a broad surge of
/// upticks (often an intraday overbought extreme), strongly negative readings a
/// broad flush. Traders fade extremes and watch the zero line for intraday bias.
///
/// `Input = CrossSection`, `Output = f64`, `warmup_period == 1`.
///
/// # Example
///
/// ```
/// use roze_ta::wickra_all::{CrossSection, Indicator, Member, TickIndex};
///
/// let mut tick = TickIndex::new();
/// // 2 advancers, 5 decliners -> net -3.
/// let snapshot = CrossSection::new(
///     vec![
///         Member::new(1.0, 10.0, false, false),
///         Member::new(1.0, 10.0, false, false),
///         Member::new(-1.0, 10.0, false, false),
///         Member::new(-1.0, 10.0, false, false),
///         Member::new(-1.0, 10.0, false, false),
///         Member::new(-1.0, 10.0, false, false),
///         Member::new(-1.0, 10.0, false, false),
///     ],
///     0,
/// )
/// .unwrap();
/// assert_eq!(tick.update(snapshot), Some(-3.0));
/// ```
#[derive(Debug, Clone, Default)]
pub struct TickIndex {
    has_emitted: bool,
}

impl TickIndex {
    /// Construct a new TICK Index indicator.
    #[must_use]
    pub const fn new() -> Self {
        Self { has_emitted: false }
    }
}

impl Indicator for TickIndex {
    type Input = CrossSection;
    type Output = f64;

    #[inline]
    fn update(&mut self, section: CrossSection) -> Option<f64> {
        let net = section.advancers() as f64 - section.decliners() as f64;
        self.has_emitted = true;
        Some(net)
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
        "TickIndex"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wickra_all::cross_section::Member;
    use crate::wickra_all::traits::BatchExt;

    fn section(up: usize, down: usize) -> CrossSection {
        let mut members = Vec::new();
        for _ in 0..up {
            members.push(Member::new(1.0, 10.0, false, false));
        }
        for _ in 0..down {
            members.push(Member::new(-1.0, 10.0, false, false));
        }
        members.push(Member::new(0.0, 10.0, false, false));
        CrossSection::new(members, 0).unwrap()
    }

    #[test]
    fn accessors_and_metadata() {
        let tick = TickIndex::new();
        assert_eq!(tick.name(), "TickIndex");
        assert_eq!(tick.warmup_period(), 1);
        assert!(!tick.is_ready());
    }

    #[test]
    fn positive_when_advancers_lead() {
        let mut tick = TickIndex::new();
        assert_eq!(tick.update(section(5, 2)), Some(3.0));
        assert!(tick.is_ready());
    }

    #[test]
    fn negative_when_decliners_lead() {
        let mut tick = TickIndex::new();
        assert_eq!(tick.update(section(2, 5)), Some(-3.0));
    }

    #[test]
    fn does_not_accumulate() {
        let mut tick = TickIndex::new();
        // Each tick is independent — the second reading does not carry the first.
        assert_eq!(tick.update(section(3, 0)), Some(3.0));
        assert_eq!(tick.update(section(0, 1)), Some(-1.0));
    }

    #[test]
    fn reset_clears_state() {
        let mut tick = TickIndex::new();
        tick.update(section(3, 0));
        assert!(tick.is_ready());
        tick.reset();
        assert!(!tick.is_ready());
    }

    #[test]
    fn batch_equals_streaming() {
        let sections = vec![section(5, 2), section(2, 5), section(3, 0), section(0, 1)];
        let mut a = TickIndex::new();
        let mut b = TickIndex::new();
        assert_eq!(
            a.batch(&sections),
            sections
                .iter()
                .map(|s| b.update(s.clone()))
                .collect::<Vec<_>>()
        );
    }
}
