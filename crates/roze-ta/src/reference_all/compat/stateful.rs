use super::*;
pub(super) fn sar(source: &str, p: &Value, d: &Data) -> Value {
    let init = number(p, "af_start", 0.02);
    let step = number(p, "af_step", 0.02);
    let max = number(p, "af_max", 0.2);
    let mut up = true;
    let mut af = init;
    if source == "talipp" {
        if d.len() < 5 {
            return Value::Null;
        }
        let mut sar = d.l[..5].iter().copied().fold(f64::INFINITY, f64::min);
        let mut ep = d.h[..5].iter().copied().fold(f64::NEG_INFINITY, f64::max);
        for i in 5..d.len() {
            let prev_ep = ep;
            sar += af * (ep - sar);
            if up {
                sar = sar.min(d.l[i - 1].min(d.l[i - 2]));
                if d.h[i] > ep {
                    ep = d.h[i];
                    af = (af + step).min(max);
                }
                if sar > d.l[i] {
                    sar = prev_ep.max(d.h[i]);
                    ep = d.l[i];
                    up = false;
                    af = init;
                }
            } else {
                sar = sar.max(d.h[i - 1].max(d.h[i - 2]));
                if d.l[i] < ep {
                    ep = d.l[i];
                    af = (af + step).min(max);
                }
                if sar < d.h[i] {
                    sar = prev_ep.min(d.l[i]);
                    ep = d.h[i];
                    up = true;
                    af = init;
                }
            }
        }
        json!({"value":sar,"trend":if up{"UP"}else{"DOWN"},"ep":ep,"accel_factor":af})
    } else {
        let mut sar = d.c[0];
        let mut high = d.h[0];
        let mut low = d.l[0];
        if d.len() > 1 {
            sar = d.c[1];
        }
        for i in 2..d.len() {
            if up {
                sar += af * (high - sar);
                if d.l[i] < sar {
                    sar = high;
                    low = d.l[i];
                    af = step;
                    up = false;
                } else {
                    if d.h[i] > high {
                        high = d.h[i];
                        af = (af + step).min(max);
                    }
                    if d.l[i - 2] < sar {
                        sar = d.l[i - 2];
                    } else if d.l[i - 1] < sar {
                        sar = d.l[i - 1];
                    }
                }
            } else {
                sar += af * (low - sar);
                if d.h[i] > sar {
                    sar = low;
                    high = d.h[i];
                    af = step;
                    up = true;
                } else {
                    if d.l[i] < low {
                        low = d.l[i];
                        af = (af + step).min(max);
                    }
                    if d.h[i - 2] > sar {
                        sar = d.h[i - 2];
                    } else if d.h[i - 1] > sar {
                        sar = d.h[i - 1];
                    }
                }
            }
        }
        json!(sar)
    }
}
pub(super) fn supertrend(p: &Value, d: &Data) -> Value {
    let n = period(p, "atr_period", 10);
    if d.len() < n {
        return Value::Null;
    }
    let a = ema(&d.tr(true), n, 1. / n as f64, false);
    let mult = number(p, "multiplier", 3.);
    let mut upper = 0.;
    let mut lower = 0.;
    let mut st = 0.;
    for (i, atr) in a.iter().enumerate().take(d.len()).skip(n) {
        let pu = upper;
        let pl = lower;
        let center = (d.h[i] + d.l[i]) / 2.;
        let u = center + mult * atr;
        let l = center - mult * atr;
        if u < upper || d.c[i - 1] > upper {
            upper = u;
        }
        if l > lower || d.c[i - 1] < lower {
            lower = l;
        }
        st = if st == pu {
            if d.c[i] <= upper {
                upper
            } else {
                lower
            }
        } else if st == pl {
            if d.c[i] >= lower {
                lower
            } else {
                upper
            }
        } else {
            st
        };
    }
    obj(&[
        ("value", st),
        ("direction", if last(&d.c) > st { 1. } else { -1. }),
    ])
}
pub(super) fn pivot(d: &Data, index: usize, high: bool) -> Value {
    json!({"ohlcv":{"open":d.o[index],"high":d.h[index],"low":d.l[index],"close":d.c[index],"volume":d.v[index],"time":null},"type":if high{"HIGH"}else{"LOW"}})
}
pub(super) fn zigzag(p: &Value, d: &Data) -> Value {
    let threshold = number(p, "threshold", 0.1);
    let mut index = 0;
    let mut high = false;
    for i in 1..d.len() {
        if high {
            if d.l[i] <= d.h[index] * (1. - threshold) {
                index = i;
                high = false;
            } else if d.h[i] >= d.h[index] {
                index = i;
            }
        } else if d.h[i] >= d.l[index] * (1. + threshold) {
            index = i;
            high = true;
        } else if d.l[i] <= d.l[index] {
            index = i;
        }
    }
    pivot(d, index, high)
}
pub(super) fn dm(d: &Data) -> (Vec<f64>, Vec<f64>) {
    let h = diff(&d.h, 1);
    let l = diff(&d.l, 1);
    let up = (0..d.len())
        .map(|i| {
            if i == 0 {
                f64::NAN
            } else if h[i] > -l[i] && h[i] > 0. {
                h[i]
            } else {
                0.
            }
        })
        .collect();
    let down = (0..d.len())
        .map(|i| {
            if i == 0 {
                f64::NAN
            } else if -l[i] > h[i] && l[i] < 0. {
                -l[i]
            } else {
                0.
            }
        })
        .collect();
    (up, down)
}
