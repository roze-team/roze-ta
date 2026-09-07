//! R1 adapter for migrated Wickra kernels; formulas remain in crate::wickra.
use super::*;
use crate::wickra::{self, Indicator};
use std::collections::VecDeque;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) enum Reference {
    Alma(wickra::Alma),
    SuperTrend(wickra::SuperTrend),
    StochRsi {
        kernel: wickra::StochRsi,
        defined: VecDeque<bool>,
    },
    Vortex(wickra::Vortex),
    Ulcer(wickra::UlcerIndex),
}

fn price(x: f64) -> bool {
    x.is_finite() && x > 0. && x <= 1e100
}
fn nonnegative(x: f64) -> bool {
    x.is_finite() && x >= 0.
}
fn percentage(x: f64) -> bool {
    x.is_finite() && (0.0..=100.000000000001).contains(&x)
}
fn candle(c: wickra::Candle) -> bool {
    price(c.open)
        && price(c.high)
        && price(c.low)
        && price(c.close)
        && c.high >= c.low
        && c.close >= c.low
        && c.close <= c.high
        && c.open >= c.low
        && c.open <= c.high
        && nonnegative(c.volume)
        && c.volume <= 1e100
}
fn valid_atr(s: &wickra::Atr, samples: u64) -> bool {
    s.period == 10
        && s.n_minus_1 == 9.
        && s.inv_period == 0.1
        && s.prev_close.is_some() == (samples > 0)
        && s.prev_close.is_none_or(price)
        && s.seed_buf.len() == samples.min(10) as usize
        && s.seed_buf.iter().all(|v| nonnegative(*v))
        && s.seeded == (samples >= 10)
        && nonnegative(s.avg)
        && (s.seeded || s.avg == 0.)
}
fn valid_rsi(s: &wickra::Rsi, samples: u64) -> bool {
    let count = samples.saturating_sub(1).min(14) as usize;
    s.period == 14
        && s.n_minus_1 == 13.
        && s.inv_period == 1. / 14.
        && s.has_prev == (samples > 0)
        && (if s.has_prev {
            price(s.prev_close)
        } else {
            s.prev_close == 0.
        })
        && s.seed_buf_gains.len() == count
        && s.seed_buf_losses.len() == count
        && s.seed_buf_gains
            .iter()
            .chain(&s.seed_buf_losses)
            .all(|v| nonnegative(*v))
        && nonnegative(s.avg_gain)
        && nonnegative(s.avg_loss)
        && s.avgs_seeded == (samples >= 15)
        && s.last_value.is_some() == (samples >= 15)
        && s.last_value.is_none_or(percentage)
}
impl Reference {
    pub(super) fn new(id: &str) -> Result<Self, TaError> {
        let value = match id {
            "alma.9" => wickra::Alma::new(9, 0.85, 6.).map(Self::Alma),
            "supertrend.10_3" => wickra::SuperTrend::new(10, 3.).map(Self::SuperTrend),
            "stoch_rsi.14_14" => wickra::StochRsi::new(14, 14).map(|kernel| Self::StochRsi {
                kernel,
                defined: VecDeque::new(),
            }),
            "vortex.14" => wickra::Vortex::new(14).map(Self::Vortex),
            "ulcer.14" => wickra::UlcerIndex::new(14).map(Self::Ulcer),
            _ => {
                return Err(TaError::new(
                    ErrorCode::UnsupportedProfile,
                    "unregistered reference profile",
                ))
            }
        };
        value.map_err(|_| {
            TaError::new(
                ErrorCode::UpstreamFailure,
                "invalid fixed Wickra parameters",
            )
        })
    }
    pub(super) fn kind(&self) -> &'static str {
        match self {
            Self::Alma(_) => "alma.9",
            Self::SuperTrend(_) => "supertrend.10_3",
            Self::StochRsi { .. } => "stoch_rsi.14_14",
            Self::Vortex(_) => "vortex.14",
            Self::Ulcer(_) => "ulcer.14",
        }
    }
    pub(super) fn valid_for(&self, id: &str, samples: u64) -> bool {
        if self.kind() != id {
            return false;
        }
        match self {
            Self::Alma(s) => {
                s.period == 9
                    && s.offset == 0.85
                    && s.sigma == 6.
                    && wickra::Alma::new(9, 0.85, 6.)
                        .is_ok_and(|expected| s.weights == expected.weights)
                    && s.window.len() == samples.min(9) as usize
                    && s.window.iter().all(|v| price(*v))
                    && s.current.is_some() == (samples >= 9)
                    && s.current.is_none_or(price)
            }
            Self::SuperTrend(s) => {
                s.atr_period == 10
                    && s.multiplier == 3.
                    && valid_atr(&s.atr, samples)
                    && s.prev.is_some() == (samples >= 10)
                    && s.prev.is_none_or(|p| {
                        p.final_upper.is_finite()
                            && p.final_lower.is_finite()
                            && price(p.close)
                            && (p.direction == 1. || p.direction == -1.)
                            && Some(p.close) == s.atr.prev_close
                    })
            }
            Self::StochRsi { kernel: s, defined } => {
                s.rsi_period == 14
                    && s.stoch_period == 14
                    && valid_rsi(&s.rsi, samples)
                    && s.window.len() == samples.saturating_sub(14).min(14) as usize
                    && s.window.iter().all(|v| percentage(*v))
                    && s.last.is_some() == (samples >= 28)
                    && s.last.is_none_or(percentage)
                    && defined.len() == s.window.len()
            }
            Self::Vortex(s) => {
                s.period == 14
                    && s.prev.is_some() == (samples > 0)
                    && s.prev.is_none_or(candle)
                    && s.window.len() == samples.saturating_sub(1).min(14) as usize
                    && s.window
                        .iter()
                        .all(|(a, b, c)| nonnegative(*a) && nonnegative(*b) && nonnegative(*c))
                    && s.sum_vm_plus == s.window.iter().map(|v| v.0).sum::<f64>()
                    && s.sum_vm_minus == s.window.iter().map(|v| v.1).sum::<f64>()
                    && s.sum_tr == s.window.iter().map(|v| v.2).sum::<f64>()
                    && s.last.is_some() == (samples >= 15)
                    && s.last
                        .is_none_or(|v| nonnegative(v.plus) && nonnegative(v.minus))
            }
            Self::Ulcer(s) => {
                s.period == 14
                    && s.count == samples
                    && s.max_dq.is_empty() == (samples == 0)
                    && s.max_dq.len() <= samples.min(14) as usize
                    && s.max_dq.iter().all(|(i, p)| {
                        *i > 0 && *i <= samples && *i >= samples.saturating_sub(13) && price(*p)
                    })
                    && s.max_dq
                        .iter()
                        .zip(s.max_dq.iter().skip(1))
                        .all(|(a, b)| a.0 < b.0 && a.1 > b.1)
                    && s.drawdowns_sq.len() == samples.saturating_sub(13).min(14) as usize
                    && s.drawdowns_sq
                        .iter()
                        .all(|v| v.is_finite() && (0.0..=10000.0).contains(v))
                    && s.sum_sq.total == s.drawdowns_sq.iter().sum::<f64>()
                    && s.sum_sq.pushes_since_reseed == 0
                    && s.last.is_some() == (samples >= 27)
                    && s.last.is_none_or(percentage)
            }
        }
    }
    pub(super) fn step(&mut self, bar: &Candle) -> Vec<f64> {
        // The engine validates positive finite OHLCV and as-of ordering before this adapter.
        let c = wickra::Candle::new_unchecked(
            bar.open,
            bar.high,
            bar.low,
            bar.close,
            bar.volume,
            bar.closed_at_ms,
        );
        match self {
            Self::Alma(s) => s.update(bar.close).into_iter().collect(),
            Self::SuperTrend(s) => s
                .update(c)
                .map(|v| vec![v.value, v.direction])
                .unwrap_or_default(),
            Self::StochRsi { kernel: s, defined } => {
                let value = s.update(bar.close);
                if s.rsi.avgs_seeded {
                    if defined.len() == 14 {
                        defined.pop_front();
                    }
                    defined.push_back(s.rsi.avg_gain + s.rsi.avg_loss > 0.);
                }
                let low = s.window.iter().copied().fold(f64::INFINITY, f64::min);
                let high = s.window.iter().copied().fold(f64::NEG_INFINITY, f64::max);
                if defined.len() == 14 && defined.iter().all(|v| *v) && high > low {
                    value.into_iter().collect()
                } else {
                    vec![]
                }
            }
            Self::Vortex(s) => {
                let value = s.update(c);
                if s.sum_tr > 0. {
                    value.map(|v| vec![v.plus, v.minus]).unwrap_or_default()
                } else {
                    vec![]
                }
            }
            Self::Ulcer(s) => s.update(bar.close).into_iter().collect(),
        }
    }
}
