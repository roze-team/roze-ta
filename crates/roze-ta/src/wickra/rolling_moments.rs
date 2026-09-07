// SPDX-License-Identifier: MIT
// Copyright (c) 2026 kingchenc and the Wickra contributors
// Derived from wickra-lib/wickra; original files, license and Git blob IDs:
// vendor/wickra-r1/ and docs/evidence/reference-r1-source.json.
// Local changes: module paths, serde state, bounded parameters, selected API subset.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub(crate) struct RollingSum {
    /// Running total over the live window.
    pub(crate) total: f64,
    /// Pushes since the last rebuild.
    pub(crate) pushes_since_reseed: usize,
}

impl RollingSum {
    /// A fresh, empty accumulator.
    pub(crate) const fn new() -> Self {
        Self {
            total: 0.0,
            pushes_since_reseed: 0,
        }
    }

    /// Add `value` to the total.
    pub(crate) fn push(&mut self, value: f64) {
        self.total += value;
        self.pushes_since_reseed += 1;
    }

    /// Remove `value` from the total. It must be one previously pushed and not
    /// yet removed.
    pub(crate) fn evict(&mut self, value: f64) {
        self.total -= value;
    }

    /// The current total.
    pub(crate) const fn value(&self) -> f64 {
        self.total
    }

    /// Whether enough pushes have accumulated to justify a rebuild.
    pub(crate) const fn needs_reseed(&self, period: usize) -> bool {
        self.pushes_since_reseed >= period
    }

    /// Rebuild the total from the live window. `values` must yield exactly the
    /// values currently included.
    pub(crate) fn reseed<I>(&mut self, values: I)
    where
        I: IntoIterator<Item = f64>,
    {
        self.total = values.into_iter().sum();
        self.pushes_since_reseed = 0;
    }

    /// Drop the total and the rebuild counter.
    pub(crate) fn reset(&mut self) {
        self.total = 0.0;
        self.pushes_since_reseed = 0;
    }
}
