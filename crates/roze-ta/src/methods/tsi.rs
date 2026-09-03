// SPDX-License-Identifier: Apache-2.0
// Derived from Yata v0.7.0, src/methods/tsi.rs.
// Copyright 2020 AMvDev (amv-dev@protonmail.com).
// Modified 2026 by roze-ta contributors: native module integration, fixed types,
// unconditional serialization, safe indexing and Rust formatting.
// See THIRD-PARTY-NOTICES.md and docs/patches/yata-native-migration.md.

use crate::core::Method;
use crate::core::{Error, PeriodType, ValueType};
use crate::helpers::Peekable;
use crate::methods::EMA;

use serde::{Deserialize, Serialize};

/// [True Strength Index](https://en.wikipedia.org/wiki/True_strength_index) of specified `short period` and `long period` for timeseries of type [`ValueType`]
///
/// ```txt
///          EMA(EMA(momentum_1, long_period), short_period)
/// TSI = ------------------------------------------------------
///        EMA(EMA(ABS(momentum_1), long_period), short_period)
/// ```
///
/// # Parameters
///
/// Tuple of \(`short_length`, `long_length`\) \([`PeriodType`], [`PeriodType`]\)
///
/// # Input type
///
/// Input type is [`ValueType`]
///
/// # Output type
///
/// Output type is [`ValueType`]
///
/// # Examples
///
/// ```
/// use roze_ta::prelude::*;
/// use roze_ta::methods::TSI;
///
/// // TSI with short length=3, long length=10
/// let mut tsi = TSI::new(3, 10, &3.0).unwrap();
///
/// tsi.next(&3.0);
/// tsi.next(&6.0);
///
/// println!("{}", tsi.next(&9.0));
/// println!("{}", tsi.next(&12.0));
/// ```
///
/// # Performance
///
/// O\(1\)
///
/// [`ValueType`]: crate::core::ValueType
/// [`PeriodType`]: crate::core::PeriodType
#[derive(Debug, Clone, Copy)]
#[doc(alias = "TrueStrengthIndex")]
#[doc(alias = "True")]
#[doc(alias = "Strength")]
#[doc(alias = "Index")]
#[derive(Serialize, Deserialize)]
pub struct TSI {
    last_value: ValueType,
    ema11: EMA,
    ema12: EMA,
    ema21: EMA,
    ema22: EMA,
}

impl TSI {
    /// Creates new instance of `TSI`
    pub fn new(
        short_period: PeriodType,
        long_period: PeriodType,
        value: &ValueType,
    ) -> Result<Self, Error> {
        Method::new((short_period, long_period), value)
    }
}

impl Method for TSI {
    type Params = (PeriodType, PeriodType);
    type Input = ValueType;
    type Output = Self::Input;

    fn new(params: Self::Params, &value: &Self::Input) -> Result<Self, Error> {
        let (short_period, long_period) = params;

        let m = Self {
            last_value: value,
            ema11: EMA::new(long_period, &0.0)?,
            ema12: EMA::new(short_period, &0.0)?,
            ema21: EMA::new(long_period, &0.0)?,
            ema22: EMA::new(short_period, &0.0)?,
        };

        Ok(m)
    }

    #[inline]
    fn next(&mut self, &value: &Self::Input) -> Self::Output {
        let momentum = value - self.last_value;
        self.last_value = value;

        self.ema12.next(&self.ema11.next(&momentum));
        self.ema22.next(&self.ema21.next(&momentum.abs()));

        self.peek()
    }
}

impl Peekable<<Self as Method>::Output> for TSI {
    fn peek(&self) -> <Self as Method>::Output {
        let numerator = self.ema12.peek();
        let denominator = self.ema22.peek();

        if denominator > 0.0 {
            numerator / denominator
        } else {
            0.0
        }
    }
}
