//! Streaming technical indicators backed by `yata`.
//!
//! The versioned [engine] validates completed bars and provides streaming,
//! batch replay and lossless snapshots for every registered profile.
//! State size depends on each indicator's windows; it is not universally O(1).

#![forbid(unsafe_code)]

pub mod analysis;
pub mod catalog;
pub mod engine;
pub mod error;
pub mod fingerprint;

/// Access all upstream indicators and methods, including those not yet registered in MCP.
pub use yata;
pub use yata::{core, helpers, indicators, methods, prelude};

use serde::{Deserialize, Serialize};
use yata::{
    methods::{EMA, RMA, TR},
    prelude::Method,
};

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Bar {
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,
}

impl Bar {
    fn valid(self) -> bool {
        [self.open, self.high, self.low, self.close, self.volume]
            .iter()
            .all(|v| v.is_finite() && v.abs() <= 1e100)
            && self.high >= self.low
            && self.open >= self.low
            && self.open <= self.high
            && self.close >= self.low
            && self.close <= self.high
            && self.volume >= 0.0
    }
    fn yata(self) -> [f64; 5] {
        [self.open, self.high, self.low, self.close, self.volume]
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AtrState {
    period: u8,
    true_range: Option<TR>,
    average: Option<RMA>,
    samples: u64,
    value: Option<f64>,
}

impl AtrState {
    pub fn new(period: usize) -> Option<Self> {
        let period = u8::try_from(period).ok().filter(|value| *value > 0)?;
        Some(Self {
            period,
            true_range: None,
            average: None,
            samples: 0,
            value: None,
        })
    }

    pub fn next(&mut self, bar: Bar) -> Option<f64> {
        if !bar.valid() {
            return None;
        }
        let candle = bar.yata();
        let tr = match &mut self.true_range {
            Some(method) => method.next(&candle),
            None => {
                let mut method = TR::new(&candle).ok()?;
                let value = method.next(&candle);
                self.true_range = Some(method);
                value
            }
        };
        let value = match &mut self.average {
            Some(method) => method.next(&tr),
            None => {
                self.average = Some(RMA::new(self.period, &tr).ok()?);
                tr
            }
        };
        self.samples += 1;
        self.value = Some(value);
        self.value
    }

    pub fn value(&self) -> Option<f64> {
        self.value
    }
    pub fn samples(&self) -> u64 {
        self.samples
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EmaState {
    period: u8,
    method: Option<EMA>,
    value: Option<f64>,
}

impl EmaState {
    pub fn new(period: usize) -> Option<Self> {
        Some(Self {
            // Yata EMA computes period + 1 in u8; 255 overflows.
            period: u8::try_from(period)
                .ok()
                .filter(|value| *value > 0 && *value < u8::MAX)?,
            method: None,
            value: None,
        })
    }
    pub fn next(&mut self, value: f64) -> Option<f64> {
        if !value.is_finite() || value.abs() > 1e100 {
            return None;
        }
        let output = match &mut self.method {
            Some(method) => method.next(&value),
            None => {
                self.method = Some(EMA::new(self.period, &value).ok()?);
                value
            }
        };
        self.value = Some(output);
        self.value
    }
    pub fn value(&self) -> Option<f64> {
        self.value
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RsiState {
    period: u8,
    previous: Option<f64>,
    gains: Option<RMA>,
    losses: Option<RMA>,
    value: Option<f64>,
}

impl RsiState {
    pub fn new(period: usize) -> Option<Self> {
        Some(Self {
            period: u8::try_from(period).ok().filter(|value| *value > 2)?,
            previous: None,
            gains: None,
            losses: None,
            value: None,
        })
    }
    pub fn next(&mut self, close: f64) -> Option<f64> {
        if !close.is_finite() || close.abs() > 1e100 {
            return None;
        }
        let previous = self.previous.replace(close)?;
        let change = close - previous;
        let gain = change.max(0.0);
        let loss = (-change).max(0.0);
        let avg_gain = match &mut self.gains {
            Some(v) => v.next(&gain),
            None => {
                self.gains = Some(RMA::new(self.period, &gain).ok()?);
                gain
            }
        };
        let avg_loss = match &mut self.losses {
            Some(v) => v.next(&loss),
            None => {
                self.losses = Some(RMA::new(self.period, &loss).ok()?);
                loss
            }
        };
        self.value = Some(if avg_gain == 0.0 && avg_loss == 0.0 {
            50.0
        } else {
            100.0 * avg_gain / (avg_gain + avg_loss)
        });
        self.value
    }
    pub fn value(&self) -> Option<f64> {
        self.value
    }
}

pub fn atr(bars: &[Bar], period: usize) -> Option<f64> {
    if bars.iter().any(|bar| !bar.valid()) {
        return None;
    }
    let mut state = AtrState::new(period)?;
    bars.iter()
        .copied()
        .filter_map(|bar| state.next(bar))
        .last()
}
pub fn ema(values: &[f64], period: usize) -> Option<f64> {
    if values.iter().any(|v| !v.is_finite() || v.abs() > 1e100) {
        return None;
    }
    let mut state = EmaState::new(period)?;
    values
        .iter()
        .copied()
        .filter_map(|value| state.next(value))
        .last()
}
pub fn rsi(values: &[f64], period: usize) -> Option<f64> {
    if values.iter().any(|v| !v.is_finite() || v.abs() > 1e100) {
        return None;
    }
    let mut state = RsiState::new(period)?;
    values
        .iter()
        .copied()
        .filter_map(|value| state.next(value))
        .last()
}

#[cfg(test)]
mod tests {
    use super::*;
    fn bar(high: f64, low: f64, close: f64) -> Bar {
        Bar {
            open: close,
            high,
            low,
            close,
            volume: 0.0,
        }
    }
    #[test]
    fn batch_and_streaming_atr_are_identical() {
        let bars = [
            bar(10.0, 8.0, 9.0),
            bar(12.0, 10.0, 11.0),
            bar(15.0, 13.0, 14.0),
        ];
        let mut state = AtrState::new(2).unwrap();
        let streamed = bars.into_iter().filter_map(|v| state.next(v)).last();
        assert_eq!(atr(&bars, 2), streamed);
        assert_eq!(state.samples(), 3);
    }
    #[test]
    fn state_round_trip_continues_exactly() {
        let mut state = AtrState::new(14).unwrap();
        state.next(bar(10.0, 8.0, 9.0));
        let mut restored: AtrState =
            serde_json::from_str(&serde_json::to_string(&state).unwrap()).unwrap();
        assert_eq!(
            state.next(bar(12.0, 10.0, 11.0)),
            restored.next(bar(12.0, 10.0, 11.0))
        );
    }
    #[test]
    fn yata_ema_and_rsi_update_per_value() {
        assert_eq!(ema(&[1.0, 2.0, 3.0, 4.0], 3), Some(3.125));
        assert_eq!(rsi(&[1.0, 2.0, 3.0, 4.0], 3), Some(100.0));
    }
}
