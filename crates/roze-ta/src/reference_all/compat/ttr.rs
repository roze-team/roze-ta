use super::*;
// Preserve the reference's running-sum rounding, which affects percentile ties.
fn sum(a: &[f64], n: usize) -> Vec<f64> {
    let mut out = vec![f64::NAN; a.len()];
    let Some(start) = a.iter().position(|v| v.is_finite()) else {
        return out;
    };
    if start + n > a.len() {
        return out;
    }
    let mut state = a[start..start + n].iter().sum::<f64>();
    out[start + n - 1] = state;
    for j in start + n..a.len() {
        state = state + a[j] - a[j - n];
        out[j] = state;
    }
    out
}
fn sma(a: &[f64], n: usize) -> Vec<f64> {
    sum(a, n).iter().map(|v| v / n as f64).collect()
}
pub(super) fn evaluate(name: &str, p: &Value, d: &Data, base: &[Value]) -> Result<Value, TaError> {
    let n = period(p, "period", 14);
    let i = d.len() - 1;
    let x = &d.x;
    Ok(match name {
        "DPO" => {
            let delay = n / 2 + 1;
            json!(if i < delay {
                f64::NAN
            } else {
                x[i - delay] - last(&sma(x, n))
            })
        }
        "ZigZag" => {
            if i == 0 {
                Value::Null
            } else {
                let change = number(p, "threshold", 0.05);
                let mut point = (d.h[1] + d.l[1]) / 2.;
                let mut at = 1;
                let mut direction = 0_i8;
                for j in 1..d.len() {
                    let lower = point * (1. - change);
                    let upper = point * (1. + change);
                    let high = point.max(d.h[j]);
                    let low = point.min(d.l[j]);
                    if direction == 0 {
                        if low <= lower {
                            direction = -1;
                        }
                        if high >= upper {
                            direction = 1;
                        }
                    }
                    match direction {
                        -1 => {
                            if d.l[j] == low {
                                point = d.l[j];
                                at = j;
                            }
                            if d.h[j] >= upper {
                                point = d.h[j];
                                at = j;
                                direction = 1;
                            }
                        }
                        1 => {
                            if d.h[j] == high {
                                point = d.h[j];
                                at = j;
                            }
                            if d.l[j] <= lower {
                                point = d.l[j];
                                at = j;
                                direction = -1;
                            }
                        }
                        _ => {}
                    }
                }
                json!(if at == i { point } else { f64::NAN })
            }
        }
        "SAR" => {
            let step = number(p, "af_step", 0.02);
            let max = number(p, "af_max", 0.2);
            let mut sar = d.l[0] - (d.h[0] - d.l[0]) / 2_f64.sqrt();
            let mut ep = d.h[0];
            let mut af = step;
            let mut up = true;
            for j in 1..d.len() {
                let low = d.l[j - 1].min(d.l[j]);
                let high = d.h[j - 1].max(d.h[j]);
                let next = if up { d.l[j] > sar } else { d.h[j] >= sar };
                let next_ep = if up { ep.max(high) } else { ep.min(low) };
                if next == up {
                    sar += (ep - sar) * af;
                    let candidate = if af == max { max } else { af + step };
                    if (up && next_ep > ep) || (!up && next_ep < ep) {
                        af = candidate;
                    }
                    sar = if up { sar.min(low) } else { sar.max(high) };
                } else {
                    af = step;
                    sar = next_ep;
                }
                up = next;
                ep = next_ep;
            }
            json!(sar)
        }
        "growth" => {
            let r = zip(x, &shift(x, 1), |a, b| (a / b).ln());
            json!(r.iter().zip(&d.y).map(|(r, s)| 1. + r * s).product::<f64>())
        }
        "ZLEMA" => {
            let alpha = 2. / (n + 1) as f64;
            let lag = 1. / alpha;
            let weight = lag.fract();
            let mut z = f64::NAN;
            if i + 1 >= n {
                z = mean(&x[..n]);
                for j in n..=i {
                    let loc = (j as f64 - lag) as usize;
                    z = alpha * (2. * x[j] - ((1. - weight) * x[loc] + weight * x[loc + 1]))
                        + (1. - alpha) * z;
                }
            }
            json!(z)
        }
        "ADX" => {
            let (up, dn) = stateful::dm(d);
            let tr = wilder_sum(&d.tr(false), n);
            let plus = zip(&wilder_sum(&up, n), &tr, |a, b| 100. * a / b);
            let minus = zip(&wilder_sum(&dn, n), &tr, |a, b| 100. * a / b);
            let dx = zip(&plus, &minus, |a, b| 100. * (a - b).abs() / (a + b));
            if i >= 2 * n && dx[n..].iter().all(|v| !v.is_finite()) {
                return Err(TaError::new(
                    ErrorCode::UndefinedResult,
                    "source ADX undefined direction",
                ));
            }
            obj(&[
                ("plus_di", plus[i]),
                ("minus_di", minus[i]),
                ("DX", dx[i]),
                ("adx", last(&ema(&dx, n, 1. / n as f64, false))),
            ])
        }
        "stoch" => {
            let k = period(p, "k_period", 5);
            let dn = period(p, "d_period", 3);
            let raw = zip(
                &zip(x, &lo(&d.l, k), |a, b| a - b),
                &zip(&hi(&d.h, k), &lo(&d.l, k), |a, b| a - b),
                |a, b| a / b,
            );
            if i >= k + dn && raw[k - 1..].iter().all(|v| !v.is_finite()) {
                return Err(TaError::new(
                    ErrorCode::UndefinedResult,
                    "source stoch undefined range",
                ));
            }
            let fast = sma(&raw, dn);
            obj(&[
                ("k", raw[i]),
                ("d", fast[i]),
                ("slowD", last(&sma(&fast, 3))),
            ])
        }
        "SMI" => {
            let slow = period(p, "d_period", 3);
            let fast = period(p, "d2_period", 3);
            let mut h = hi(&d.h, n);
            let mut l = lo(&d.l, n);
            for j in 0..d.len() {
                if h[j].is_nan() {
                    h[j] = d.h[j];
                    l[j] = d.l[j];
                }
            }
            let smooth = |a: &[f64]| {
                ema(
                    &ema(a, slow, 2. / (slow + 1) as f64, false),
                    fast,
                    2. / (fast + 1) as f64,
                    false,
                )
            };
            let num = smooth(&zip(x, &zip(&h, &l, |a, b| (a + b) / 2.), |a, b| a - b));
            let den = smooth(&zip(&h, &l, |a, b| a - b));
            let smi = zip(&num, &den, |a, b| 200. * a / b);
            if i >= slow + fast + 9 && smi.iter().all(|v| !v.is_finite()) {
                return Err(TaError::new(
                    ErrorCode::UndefinedResult,
                    "source SMI undefined range",
                ));
            }
            obj(&[("SMI", smi[i]), ("signal", last(&ema(&smi, 9, 0.2, false)))])
        }
        "wilderSum" => json!(last(&wilder_sum(x, n))),
        "volatility" => {
            let r = zip(x, &shift(x, 1), |a, b| (a / b).ln());
            json!(last(&std(&r, n - 1, true)) * number(p, "trading_periods", 252.).sqrt())
        }
        "CLV" => json!(if d.h[i] == d.l[i] {
            0.
        } else {
            (2. * d.c[i] - d.h[i] - d.l[i]) / (d.h[i] - d.l[i])
        }),
        "CMF" => {
            let f = (0..=i)
                .map(|j| (2. * d.c[j] - d.h[j] - d.l[j]) / (d.h[j] - d.l[j]) * d.v[j])
                .collect::<Vec<_>>();
            json!(last(&sum(&f, n)) / last(&sum(&d.v, n)))
        }
        "CCI" => {
            let tp = d.tp();
            let md = rolling(&tp, n, |a| {
                let m = mean(a);
                a.iter().map(|x| (x - m).abs()).sum::<f64>() / n as f64
            });
            json!((tp[i] - last(&sma(&tp, n))) / (0.015 * md[i]))
        }
        "TR" | "ATR" => {
            let tr = d.tr(false);
            let h = if i == 0 {
                f64::NAN
            } else {
                d.h[i].max(d.c[i - 1])
            };
            let l = if i == 0 {
                f64::NAN
            } else {
                d.l[i].min(d.c[i - 1])
            };
            if name == "TR" {
                obj(&[("tr", tr[i]), ("trueHigh", h), ("trueLow", l)])
            } else {
                obj(&[
                    ("tr", tr[i]),
                    ("atr", last(&ema(&tr, n, 1. / n as f64, false))),
                    ("trueHigh", h),
                    ("trueLow", l),
                ])
            }
        }
        "BBands" => {
            let m = last(&sma(x, n));
            let s = last(&std(x, n, false)) * number(p, "multiplier", 2.);
            obj(&[
                ("lower", m - s),
                ("middle", m),
                ("upper", m + s),
                ("pctB", (x[i] - (m - s)) / (2. * s)),
            ])
        }
        "DonchianChannel" => {
            let l = last(&lo(&d.l, n));
            let h = last(&hi(&d.h, n));
            obj(&[("high", h), ("mid", (h + l) / 2.), ("low", l)])
        }
        "runCor" => json!(if i + 1 < n {
            f64::NAN
        } else {
            correlation(&x[i + 1 - n..], &d.y[i + 1 - n..])
        }),
        "rollSFM" => {
            if i + 1 < n {
                obj(&[
                    ("alpha", f64::NAN),
                    ("beta", f64::NAN),
                    ("r.squared", f64::NAN),
                ])
            } else {
                let a = &x[i + 1 - n..];
                let b = &d.y[i + 1 - n..];
                let beta = covariance(a, b) / covariance(b, b);
                obj(&[
                    ("alpha", mean(a) - beta * mean(b)),
                    ("beta", beta),
                    ("r.squared", {
                        let nn = n as f64;
                        let sa = last(&sum(x, n));
                        let sb = last(&sum(&d.y, n));
                        let aa = last(&sum(&x.iter().map(|v| v * v).collect::<Vec<_>>(), n));
                        let bb = last(&sum(&d.y.iter().map(|v| v * v).collect::<Vec<_>>(), n));
                        let residual = 1. / (nn * (nn - 2.))
                            * (nn * aa - sa * sa - beta * beta * (nn * bb - sb * sb));
                        1. - residual / (covariance(a, a) * (nn - 1.) / (nn - 2.))
                    }),
                ])
            }
        }
        "CTI" => json!(last(&rolling(x, n, |a| {
            let ranks = a
                .iter()
                .map(|v| {
                    let less = a.iter().filter(|x| *x < v).count();
                    let same = a.iter().filter(|x| *x == v).count();
                    less as f64 + (same + 1) as f64 / 2.
                })
                .collect::<Vec<_>>();
            correlation(&ranks, &(0..n).map(|j| j as f64).collect::<Vec<_>>())
        }))),
        "lags" => {
            let mut fields = serde_json::Map::new();
            for lag in 0..=n {
                fields.insert(
                    if lag == 0 {
                        "V1".into()
                    } else {
                        format!("V1.{lag}")
                    },
                    json!(if i < n { f64::NAN } else { x[i - lag] }),
                );
            }
            Value::Object(fields)
        }
        "GMMA" => {
            let mut fields = serde_json::Map::new();
            for (group, periods) in [
                ("short", vec![3, 5, 8, 10, 12, 15]),
                ("long", vec![30, 35, 40, 45, 50, 60]),
            ] {
                for n in periods {
                    fields.insert(
                        format!("{group} lag {n}"),
                        json!(last(&ema(x, n, 2. / (n + 1) as f64, false))),
                    );
                }
            }
            Value::Object(fields)
        }
        "MACD" => {
            let f = period(p, "fast", 12);
            let s = period(p, "slow", 26);
            let sn = period(p, "signal", 9);
            let a = ema(x, f, 2. / (f + 1) as f64, false);
            let b = ema(x, s, 2. / (s + 1) as f64, false);
            let macd = zip(&a, &b, |a, b| 100. * (a - b) / b);
            obj(&[
                ("macd", macd[i]),
                ("signal", last(&ema(&macd, sn, 2. / (sn + 1) as f64, false))),
            ])
        }
        "TRIX" => {
            let mut a = x.clone();
            for _ in 0..3 {
                a = ema(&a, n, 2. / (n + 1) as f64, false);
            }
            let trix = roc(&a, 1, 100.);
            obj(&[
                ("TRIX", trix[i]),
                ("signal", last(&ema(&trix, 9, 0.2, false))),
            ])
        }
        "KST" => {
            let mut kst = vec![0.; d.len()];
            for j in 1..=4 {
                let rn = period(p, &format!("roc{j}"), 10);
                let mn = period(p, &format!("sma{j}"), 10);
                let r = zip(x, &shift(x, rn), |a, b| (a / b).ln());
                let a = sma(&r, mn);
                for t in 0..d.len() {
                    kst[t] += 100. * j as f64 * a[t];
                }
            }
            obj(&[
                ("kst", kst[i]),
                ("signal", last(&sma(&kst, period(p, "signal", 9)))),
            ])
        }
        "TDI" => {
            let m = diff(x, n)
                .iter()
                .map(|v| if v.is_nan() { 0. } else { *v })
                .collect::<Vec<_>>();
            let di = last(&sum(&m, n));
            let abs = m.iter().map(|v| v.abs()).collect::<Vec<_>>();
            obj(&[
                (
                    "tdi",
                    di.abs() - last(&sum(&abs, 2 * n)) + last(&sum(&abs, n)),
                ),
                ("di", di),
            ])
        }
        "PBands" => {
            let a = sma(x, n);
            let diff = zip(&a, &sma(x, 2), |a, b| a - b);
            let s = last(&std(&diff, n, false));
            obj(&[
                ("lower", a[i] - 2. * s),
                ("center", a[i]),
                ("upper", a[i] + 2. * s),
            ])
        }
        "SNR" => json!(
            (x[i] - if i < n { f64::NAN } else { x[i - n] }).abs()
                / last(&ema(&d.tr(false), n, 1. / n as f64, false))
        ),
        "EMV" => {
            let m = zip(&d.h, &d.l, |a, b| (a + b) / 2.);
            let raw = zip(&diff(&m, 1), &zip(&d.h, &d.l, |a, b| a - b), |a, b| a * b);
            let emv = zip(&raw, &d.v, |a, b| a / b * 10000.);
            if i >= n && emv[1..].iter().any(|v| !v.is_finite()) {
                return Err(TaError::new(
                    ErrorCode::UndefinedResult,
                    "source EMV undefined volume",
                ));
            }
            obj(&[("emv", emv[i]), ("maEMV", last(&sma(&emv, n)))])
        }
        "keltnerChannels" => {
            let nn = period(p, "ema_period", 5);
            let a = last(&ema(&d.tp(), nn, 2. / (nn + 1) as f64, false));
            let atr =
                last(&ema(&d.tr(false), nn, 1. / nn as f64, false)) * number(p, "multiplier", 2.);
            obj(&[("lower", a - atr), ("middle", a), ("upper", a + atr)])
        }
        "DVI" => {
            let r = zip(x, &sma(x, 3), |a, b| a / b - 1.);
            let mag = sma(
                &zip(&sma(&r, 5), &sma(&r, 100), |a, b| (a + b / 10.) / 2.),
                5,
            );
            let b = (0..=i)
                .map(|j| {
                    if j == 0 {
                        f64::NAN
                    } else if x[j] > x[j - 1] {
                        1.
                    } else {
                        -1.
                    }
                })
                .collect::<Vec<_>>();
            let stretch = sma(
                &zip(&sum(&b, 10), &sum(&b, 100), |a, b| (a + b / 10.) / 2.),
                2,
            );
            let rank = |a: &[f64]| {
                last(&rolling(a, 252, |a| {
                    a.iter()
                        .filter(|x| **x < a[a.len() - 1] || (**x - a[a.len() - 1]).abs() < 1e-8)
                        .count() as f64
                        / 252.
                }))
            };
            let m = rank(&mag);
            let s = rank(&stretch);
            obj(&[("dvi.mag", m), ("dvi.str", s), ("dvi", 0.8 * m + 0.2 * s)])
        }
        "aroon" => {
            if i + 1 < n {
                obj(&[
                    ("up", f64::NAN),
                    ("down", f64::NAN),
                    ("oscillator", f64::NAN),
                ])
            } else {
                let index = |a: &[f64], high: bool| {
                    let start = (i + 1).saturating_sub(n + 1);
                    let w = &a[start..=i];
                    let best = if high {
                        w.iter().copied().fold(f64::NEG_INFINITY, f64::max)
                    } else {
                        w.iter().copied().fold(f64::INFINITY, f64::min)
                    };
                    (n - (i - start - w.iter().rposition(|x| *x == best).unwrap_or(0))) as f64
                        / n as f64
                        * 100.
                };
                let u = index(&d.h, true);
                let l = index(&d.l, false);
                obj(&[("up", u), ("down", l), ("oscillator", u - l)])
            }
        }
        "ROC" => json!(if i < n {
            f64::NAN
        } else {
            (x[i] / x[i - n]).ln()
        }),
        "WPR" => {
            let h = last(&hi(&d.h, n));
            let l = last(&lo(&d.l, n));
            json!(if h == l { 0.5 } else { (h - d.c[i]) / (h - l) })
        }
        "runPercentRank" => {
            let b = fallback(base);
            json!(b.as_f64().unwrap_or(f64::NAN) / 100.)
        }
        "runRange" => obj(&[("min", last(&lo(x, n))), ("max", last(&hi(x, n)))]),
        "RSI" => json!(last(&rsi(x, n, false, f64::NAN))),
        "CMO" => {
            let z = diff(x, 1);
            let u = sum(
                &z.iter()
                    .map(|z| if z.is_nan() { f64::NAN } else { z.max(0.) })
                    .collect::<Vec<_>>(),
                n,
            );
            let a = sum(&z.iter().map(|z| z.abs()).collect::<Vec<_>>(), n);
            json!(100. * (2. * u[i] - a[i]) / a[i])
        }
        "VWMA" | "VWAP" => {
            let n = if name == "VWAP" { 10 } else { n };
            json!(last(&sum(&zip(x, &d.v, |a, b| a * b), n)) / last(&sum(&d.v, n)))
        }
        "ALMA" => {
            let offset = (number(p, "offset", 0.85) * (n - 1) as f64).floor();
            let s = n as f64 / number(p, "sigma", 6.);
            json!(last(&rolling(x, n, |a| {
                let weights = (0..n)
                    .map(|j| (-((j as f64 - offset).powi(2)) / (2. * s * s)).exp())
                    .collect::<Vec<_>>();
                a.iter().zip(&weights).map(|(x, w)| x * w).sum::<f64>()
                    / weights.iter().sum::<f64>()
            })))
        }
        "ultimateOscillator" => python::ultimate(p, d),
        "chaikinVolatility" => {
            let n = period(p, "ema_period", 10);
            let a = ema(
                &zip(&d.h, &d.l, |a, b| a - b),
                n,
                2. / (n + 1) as f64,
                false,
            );
            json!(last(&roc(&a, n, 1.)))
        }
        "VHF" => json!(
            (last(&hi(x, n)) - last(&lo(x, n)))
                / last(&sum(
                    &diff(x, 1).iter().map(|v| v.abs()).collect::<Vec<_>>(),
                    n
                ))
        ),
        "EVWMA" => {
            let vol = sum(&d.v, n);
            let mut state = f64::NAN;
            for j in n - 1..d.len() {
                state = if j == n - 1 {
                    x[j]
                } else {
                    ((vol[j] - d.v[j]) * state + d.v[j] * x[j]) / vol[j]
                };
            }
            json!(state)
        }
        _ => fallback(base),
    })
}
