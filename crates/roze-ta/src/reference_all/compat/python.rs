use super::*;
pub(super) fn evaluate(
    source: &str,
    name: &str,
    p: &Value,
    d: &Data,
    base: &[Value],
) -> Result<Value, TaError> {
    let n = period(p, "period", 14);
    let i = d.len() - 1;
    let x = &d.x;
    let is_ta = source == "ta";
    let value = match name {
        "ADXIndicator" => {
            let (up, dn) = stateful::dm(d);
            let tr = ema(&d.tr(false), n, 1. / n as f64, false);
            let plus = zip(&ema(&up, n, 1. / n as f64, false), &tr, |a, b| {
                if b == 0. {
                    0.
                } else {
                    100. * a / b
                }
            });
            let minus = zip(&ema(&dn, n, 1. / n as f64, false), &tr, |a, b| {
                if b == 0. {
                    0.
                } else {
                    100. * a / b
                }
            });
            let dx = zip(&plus, &minus, |a, b| {
                if a + b == 0. {
                    0.
                } else {
                    100. * (a - b).abs() / (a + b)
                }
            });
            obj(&[
                (
                    "adx",
                    if i < 2 * n - 1 {
                        0.
                    } else {
                        last(&ema(&dx, n, 1. / n as f64, false))
                    },
                ),
                ("plus_di", if i <= n { 0. } else { plus[i] }),
                ("minus_di", if i <= n { 0. } else { minus[i] }),
            ])
        }
        "Williams" | "WilliamsRIndicator" => {
            json!(-100. * (last(&hi(&d.h, n)) - d.c[i]) / (last(&hi(&d.h, n)) - last(&lo(&d.l, n))))
        }
        "ParabolicSAR" | "PSARIndicator" => stateful::sar(source, p, d),
        "SuperTrend" => stateful::supertrend(p, d),
        "ZigZag" => stateful::zigzag(p, d),
        "PivotsHL" => {
            if let Some(b) = base.iter().rev().find(|b| !b.is_null()) {
                let index = b["pivot_at_ms"]
                    .as_i64()
                    .and_then(|v| d.times.iter().position(|t| *t == v))
                    .ok_or_else(|| invalid("pivot timestamp missing from observed history"))?;
                stateful::pivot(d, index, b["kind"] == "high")
            } else {
                Value::Null
            }
        }
        "KVO" => {
            let mut vf = vec![f64::NAN; d.len()];
            let mut cm = 0.;
            let mut trend = 0.;
            for (j, value) in vf.iter_mut().enumerate().skip(2) {
                let t = if d.h[j] + d.l[j] + d.c[j] > d.h[j - 1] + d.l[j - 1] + d.c[j - 1] {
                    1.
                } else {
                    -1.
                };
                let range = d.h[j] - d.l[j];
                cm = if t == trend {
                    cm + range
                } else {
                    d.h[j - 1] - d.l[j - 1] + range
                };
                trend = t;
                *value = if cm == 0. {
                    0.
                } else {
                    d.v[j] * (2. * (range / cm - 1.)).abs() * t * 100.
                };
            }
            let f = period(p, "fast", 34);
            let s = period(p, "slow", 55);
            json!(
                last(&ema(&vf, f, 2. / (f + 1) as f64, false))
                    - last(&ema(&vf, s, 2. / (s + 1) as f64, false))
            )
        }
        "ADX" => {
            let (up, dn) = stateful::dm(d);
            let a = ema(&d.tr(true), n, 1. / n as f64, false);
            let u = ema(&up, n, 1. / n as f64, false);
            let l = ema(&dn, n, 1. / n as f64, false);
            let plus = zip(&u, &a, |u, a| {
                if u.is_nan() {
                    f64::NAN
                } else if a == 0. {
                    0.
                } else {
                    100. * u / a
                }
            });
            let minus = zip(&l, &a, |u, a| {
                if u.is_nan() {
                    f64::NAN
                } else if a == 0. {
                    0.
                } else {
                    100. * u / a
                }
            });
            let dx = zip(&plus, &minus, |u, l| {
                if u + l == 0. {
                    0.
                } else {
                    100. * (u - l).abs() / (u + l)
                }
            });
            if i < n {
                Value::Null
            } else {
                obj(&[
                    ("adx", last(&ema(&dx, n, 1. / n as f64, false))),
                    ("plus_di", plus[i]),
                    ("minus_di", minus[i]),
                ])
            }
        }
        "VTX" | "VortexIndicator" => {
            let tr = sum(&d.tr(true), n);
            let plus = sum(&zip(&d.h, &shift(&d.l, 1), |h, l| (h - l).abs()), n);
            let minus = sum(&zip(&d.l, &shift(&d.h, 1), |l, h| (l - h).abs()), n);
            if !is_ta && i >= n && tr[i] == 0. {
                return Err(TaError::new(
                    ErrorCode::UndefinedResult,
                    "source VTX zero true range",
                ));
            }
            if !is_ta && i < n {
                Value::Null
            } else {
                obj(&[("plus", plus[i] / tr[i]), ("minus", minus[i] / tr[i])])
            }
        }
        "TTM" => {
            if i + 1 < n {
                Value::Null
            } else {
                let m = sma(x, n);
                let dc = zip(&hi(&d.h, n), &lo(&d.l, n), |a, b| (a + b) / 2.);
                let delta = (0..=i)
                    .map(|j| x[j] - (m[j] + dc[j]) / 2.)
                    .collect::<Vec<_>>();
                let hist = last(&rolling(&delta, n, |a| {
                    let mx = (n - 1) as f64 / 2.;
                    let my = mean(a);
                    let num = a
                        .iter()
                        .enumerate()
                        .map(|(j, y)| (j as f64 - mx) * (y - my))
                        .sum::<f64>();
                    let den = (0..n).map(|j| (j as f64 - mx).powi(2)).sum::<f64>();
                    my + num / den * mx
                }));
                let s = last(&std(x, n, false)) * number(p, "bb_mult", 2.);
                let kc = last(&ema(x, n, 2. / (n + 1) as f64, false));
                let a =
                    last(&ema(&d.tr(true), n, 1. / n as f64, false)) * number(p, "kc_mult", 1.5);
                obj(&[
                    (
                        "squeeze",
                        if m[i] + s < kc + a && m[i] - s > kc - a {
                            1.
                        } else {
                            0.
                        },
                    ),
                    ("momentum", hist),
                ])
            }
        }
        "MACD" | "PercentagePriceOscillator" | "PercentageVolumeOscillator" => {
            let fast = period(p, "fast", 12);
            let slow = period(p, "slow", 26);
            let sig = period(p, "signal", 9);
            let a = ema(x, fast, 2. / (fast + 1) as f64, is_ta);
            let b = ema(x, slow, 2. / (slow + 1) as f64, is_ta);
            let macd = zip(&a, &b, |a, b| {
                if name == "MACD" {
                    a - b
                } else {
                    100. * (a - b) / b
                }
            });
            let signal = ema(&macd, sig, 2. / (sig + 1) as f64, is_ta);
            if name != "MACD" {
                json!(macd[i])
            } else if !is_ta && macd[i].is_nan() {
                Value::Null
            } else {
                obj(&[
                    ("macd", macd[i]),
                    ("signal", signal[i]),
                    ("histogram", macd[i] - signal[i]),
                ])
            }
        }
        "TRIX" | "TRIXIndicator" => {
            let a = ema(x, n, 2. / (n + 1) as f64, is_ta);
            let b = ema(&a, n, 2. / (n + 1) as f64, is_ta);
            let c = ema(&b, n, 2. / (n + 1) as f64, is_ta);
            json!(last(&roc(&c, 1, if is_ta { 100. } else { 10000. })))
        }
        "TSI" | "TSIIndicator" => {
            let slow = period(p, "long", 25);
            let fast = period(p, "short", 13);
            let change = diff(x, 1);
            let smooth = |a: &[f64]| {
                ema(
                    &ema(a, slow, 2. / (slow + 1) as f64, is_ta),
                    fast,
                    2. / (fast + 1) as f64,
                    is_ta,
                )
            };
            let a = smooth(&change);
            let b = smooth(&change.iter().map(|v| v.abs()).collect::<Vec<_>>());
            json!(100. * a[i] / b[i])
        }
        "Stoch" | "StochasticOscillator" => {
            let k = period(p, "k_period", 5);
            let dp = period(p, "d_period", 3);
            let raw = stoch(x, &d.h, &d.l, k, if is_ta { f64::NAN } else { 100. });
            if !is_ta && i + 1 < k {
                Value::Null
            } else {
                obj(&[("k", raw[i]), ("d", last(&sma(&raw, dp)))])
            }
        }
        "StochRSI" | "StochRSIIndicator" => {
            let rn = period(p, "rsi_period", 14);
            let sn = period(p, "stoch_period", 14);
            let r = rsi(x, rn, is_ta, 100.);
            let raw = stoch(&r, &r, &r, sn, if is_ta { f64::NAN } else { 100. });
            if is_ta {
                json!(raw[i] / 100.)
            } else if i < rn + sn - 1 {
                Value::Null
            } else {
                let k = sma(&raw, 3);
                obj(&[("k", k[i]), ("d", last(&sma(&k, 3)))])
            }
        }
        "STC" | "STCIndicator" => {
            let fast = period(p, "fast", 23);
            let slow = period(p, "slow", 50);
            let sn = period(p, "schaff_period", 10);
            let ma = |n| {
                if is_ta {
                    ema(x, n, 2. / (n + 1) as f64, true)
                } else {
                    weighted_ema(x, n)
                }
            };
            let macd = zip(&ma(fast), &ma(slow), |a, b| a - b);
            let smooth = |a: &[f64]| {
                if is_ta {
                    ema(a, 3, 0.5, true)
                } else {
                    recursive_sma(a, 3)
                }
            };
            let raw = stoch(&macd, &macd, &macd, sn, if is_ta { f64::NAN } else { 100. });
            let a = smooth(&raw);
            let mut b = stoch(&a, &a, &a, sn, if is_ta { f64::NAN } else { 100. });
            if !is_ta {
                for j in slow - 1 + 2 * (sn - 1)..d.len() {
                    let valid = a[j + 1 - sn..=j]
                        .iter()
                        .copied()
                        .filter(|v| v.is_finite())
                        .collect::<Vec<_>>();
                    let lo = valid.iter().copied().fold(f64::INFINITY, f64::min);
                    let hi = valid.iter().copied().fold(f64::NEG_INFINITY, f64::max);
                    b[j] = if hi == lo {
                        100.
                    } else {
                        100. * (a[j] - lo) / (hi - lo)
                    };
                }
            }
            json!(last(&smooth(&b)))
        }
        "SOBV" => json!(last(&sma(&obv(x, &d.v), n))),
        "SFX" => {
            let a = ema(&d.tr(true), n, 1. / n as f64, false);
            let s = std(x, n, false);
            obj(&[
                ("atr", a[i]),
                ("stddev", s[i]),
                ("smoothed_stddev", last(&sma(&s, n))),
            ])
        }
        "KeltnerChannels" | "KeltnerChannel" => {
            let ma_n = period(p, "ema_period", 5);
            let an = period(p, "atr_period", 5);
            let mult = number(p, "multiplier", 2.);
            if is_ta {
                let lower = (0..=i)
                    .map(|j| (-2. * d.h[j] + 4. * d.l[j] + d.c[j]) / 3.)
                    .collect::<Vec<_>>();
                let upper = (0..=i)
                    .map(|j| (4. * d.h[j] - 2. * d.l[j] + d.c[j]) / 3.)
                    .collect::<Vec<_>>();
                obj(&[
                    ("lower", last(&partial(&lower, ma_n, mean))),
                    ("middle", last(&sma(&d.tp(), ma_n))),
                    ("upper", last(&partial(&upper, ma_n, mean))),
                ])
            } else {
                let m = last(&ema(x, ma_n, 2. / (ma_n + 1) as f64, false));
                let a = last(&ema(&d.tr(true), an, 1. / an as f64, false));
                if m.is_nan() || a.is_nan() {
                    Value::Null
                } else {
                    obj(&[
                        ("lower", m - mult * a),
                        ("middle", m),
                        ("upper", m + mult * a),
                    ])
                }
            }
        }
        "MassIndex" => {
            let en = period(p, "ema_period", 9);
            let sn = period(p, "sum_period", 25);
            let range = zip(&d.h, &d.l, |a, b| a - b);
            let a = ema(&range, en, 2. / (en + 1) as f64, is_ta);
            let b = ema(&a, en, 2. / (en + 1) as f64, is_ta);
            if !is_ta && b[i] == 0. {
                return Err(TaError::new(
                    ErrorCode::UndefinedResult,
                    "source MassIndex zero range",
                ));
            }
            json!(last(&sum(&zip(&a, &b, |a, b| a / b), sn)))
        }
        "KAMA" | "KAMAIndicator" => {
            let n = period(p, "er_period", 10);
            let fast = 2. / (number(p, "fast", 2.) + 1.);
            let slow = 2. / (number(p, "slow", 30.) + 1.);
            let mut state = f64::NAN;
            let start = if is_ta { n - 1 } else { n };
            for j in start..=i {
                if is_ta && j == n - 1 {
                    state = x[j];
                    continue;
                }
                let vol = (j + 1 - n..=j)
                    .map(|k| (x[k] - x[k - 1]).abs())
                    .sum::<f64>();
                let er = if vol == 0. {
                    0.
                } else {
                    (x[j] - x[j - n]).abs() / vol
                };
                let a = (er * (fast - slow) + slow).powi(2);
                if state.is_nan() {
                    state = x[j - 1];
                }
                state += a * (x[j] - state);
            }
            json!(state)
        }
        "McGinleyDynamic" => {
            let mut state = f64::NAN;
            if i + 1 >= n {
                state = mean(&x[..n]);
                for v in &x[n..] {
                    state += (v - state) / (n as f64 * (v / state).powi(4));
                }
            }
            json!(state)
        }
        "UlcerIndex" => {
            let max = partial(x, n, |a| {
                a.iter().copied().fold(f64::NEG_INFINITY, f64::max)
            });
            let r = zip(x, &max, |a, b| (100. * (a - b) / b).powi(2));
            json!(last(&sma(&r, n)).sqrt())
        }
        "VolumePriceTrendIndicator" => {
            let delta = zip(&roc(x, 1, 1.), &d.v, |a, b| a * b);
            json!(if i == 0 {
                f64::NAN
            } else {
                delta[1..].iter().sum::<f64>()
            })
        }
        "MFIIndicator" => {
            let tp = d.tp();
            let mf = zip(&tp, &d.v, |a, b| a * b);
            let up = (0..=i)
                .map(|j| {
                    if j > 0 && tp[j] > tp[j - 1] {
                        mf[j]
                    } else {
                        0.
                    }
                })
                .collect::<Vec<_>>();
            let dn = (0..=i)
                .map(|j| {
                    if j > 0 && tp[j] < tp[j - 1] {
                        mf[j]
                    } else {
                        0.
                    }
                })
                .collect::<Vec<_>>();
            json!(100. - 100. / (1. + last(&sum(&up, n)) / last(&sum(&dn, n))))
        }
        "KST" | "KSTIndicator" => {
            let mut kst = vec![0.; d.len()];
            for j in 1..=4 {
                let rn = period(p, &format!("roc{j}"), 10);
                let mn = period(p, &format!("sma{j}"), 10);
                let mut r = roc(x, rn, 100.);
                if is_ta {
                    let m = mean(x);
                    for (t, v) in r.iter_mut().enumerate().take(rn) {
                        *v = (x[t] / m - 1.) * 100.;
                    }
                }
                let a = sma(&r, mn);
                for t in 0..d.len() {
                    kst[t] += j as f64 * a[t];
                }
            }
            let sn = period(p, "signal", 9);
            let signal = if is_ta {
                partial(&kst, sn, |a| {
                    let a = a
                        .iter()
                        .copied()
                        .filter(|v| v.is_finite())
                        .collect::<Vec<_>>();
                    mean(&a)
                })
            } else {
                sma(&kst, sn)
            };
            if !is_ta && kst[i].is_nan() {
                Value::Null
            } else {
                obj(&[("kst", kst[i]), ("signal", signal[i])])
            }
        }
        "Ichimoku" | "IchimokuIndicator" => {
            let kn = period(p, "kijun_period", 26);
            let tn = period(p, "tenkan_period", 9);
            let bn = period(p, "senkou_b_period", 52);
            let disp = period(p, "displacement", 26);
            let mid = |n| zip(&hi(&d.h, n), &lo(&d.l, n), |a, b| (a + b) / 2.);
            let k = mid(kn);
            let t = mid(tn);
            let a = zip(&k, &t, |k, t| (k + t) / 2.);
            if is_ta {
                let b = zip(
                    &partial(&d.h, bn, |a| {
                        a.iter().copied().fold(f64::NEG_INFINITY, f64::max)
                    }),
                    &partial(&d.l, bn, |a| {
                        a.iter().copied().fold(f64::INFINITY, f64::min)
                    }),
                    |a, b| (a + b) / 2.,
                );
                obj(&[
                    ("tenkan", t[i]),
                    ("kijun", k[i]),
                    ("senkou_a", a[i]),
                    ("senkou_b", b[i]),
                ])
            } else {
                obj(&[
                    ("tenkan", t[i]),
                    ("kijun", k[i]),
                    ("chikou", if i + 1 >= disp { x[i] } else { f64::NAN }),
                    ("senkou_a", last(&shift(&a, disp))),
                    ("senkou_b", last(&shift(&mid(bn), disp + 1))),
                ])
            }
        }
        "FibonacciRetracement" => json!(d.x[i] * 1.618),
        "DailyLogReturnIndicator" => json!(if i == 0 {
            f64::NAN
        } else {
            (x[i].ln() - x[i - 1].ln()) * 100.
        }),
        "EMAIndicator" => json!(last(&ema(x, n, 2. / (n + 1) as f64, true))),
        "AverageTrueRange" => {
            let a = ema(&d.tr(true), n, 1. / n as f64, false);
            json!(if i + 1 < n { 0. } else { a[i] })
        }
        "BB" | "BollingerBands" => {
            let m = last(&sma(x, n));
            let s = last(&std(x, n, false)) * number(p, "multiplier", 2.);
            let v = obj(&[("lower", m - s), ("middle", m), ("upper", m + s)]);
            if !is_ta && i + 1 < n {
                Value::Null
            } else {
                v
            }
        }
        "AroonIndicator" => {
            if i < n {
                obj(&[("up", f64::NAN), ("down", f64::NAN)])
            } else {
                let index = |a: &[f64], high: bool| {
                    let w = &a[i - n..=i];
                    let best = if high {
                        w.iter().copied().fold(f64::NEG_INFINITY, f64::max)
                    } else {
                        w.iter().copied().fold(f64::INFINITY, f64::min)
                    };
                    w.iter().position(|x| *x == best).unwrap_or(0) as f64 / n as f64 * 100.
                };
                obj(&[("up", index(&d.h, true)), ("down", index(&d.l, false))])
            }
        }
        "DonchianChannel" => obj(&[
            ("lower", last(&lo(&d.l, n))),
            ("middle", (last(&lo(&d.l, n)) + last(&hi(&d.h, n))) / 2.),
            ("upper", last(&hi(&d.h, n))),
        ]),
        "ChandeKrollStop" => {
            let b = fallback(base);
            if b.is_null() {
                b
            } else {
                json!({"long_stop":b["stop_long"],"short_stop":b["stop_short"]})
            }
        }
        "RSIIndicator" => json!(last(&rsi(x, n, true, 100.))),
        "RSI" => json!(last(&rsi(x, n, false, 100.))),
        "CCI" | "CCIIndicator" => {
            let t = d.tp();
            let md = rolling(&t, n, |a| {
                let m = mean(a);
                a.iter().map(|x| (x - m).abs()).sum::<f64>() / n as f64
            });
            if !is_ta && i + 1 >= n && md[i] == 0. {
                return Err(TaError::new(
                    ErrorCode::UndefinedResult,
                    "source CCI zero deviation",
                ));
            }
            json!((t[i] - last(&sma(&t, n))) / (0.015 * md[i]))
        }
        "BOP" => json!(if d.h[i] == d.l[i] {
            f64::NAN
        } else {
            (d.c[i] - d.o[i]) / (d.h[i] - d.l[i])
        }),
        "AccuDist" | "ChaikinOsc" => {
            let mut state = 0.;
            let mut ad = vec![f64::NAN; d.len()];
            for (j, value) in ad.iter_mut().enumerate() {
                if d.h[j] != d.l[j] {
                    state += (2. * d.c[j] - d.h[j] - d.l[j]) / (d.h[j] - d.l[j]) * d.v[j];
                    *value = state;
                }
            }
            if name == "AccuDist" {
                json!(ad[i])
            } else {
                let f = period(p, "fast", 3);
                let sn = period(p, "slow", 10);
                json!(if ad[i].is_nan() {
                    f64::NAN
                } else {
                    last(&ema(&ad, f, 2. / (f + 1) as f64, false))
                        - last(&ema(&ad, sn, 2. / (sn + 1) as f64, false))
                })
            }
        }
        "ChaikinMoneyFlowIndicator" => {
            let flow = (0..=i)
                .map(|j| {
                    if d.h[j] == d.l[j] {
                        0.
                    } else {
                        (2. * d.c[j] - d.h[j] - d.l[j]) / (d.h[j] - d.l[j]) * d.v[j]
                    }
                })
                .collect::<Vec<_>>();
            json!(last(&sum(&flow, n)) / last(&sum(&d.v, n)))
        }
        "ForceIndexIndicator" => json!(last(&ema(
            &zip(&diff(x, 1), &d.v, |a, b| a * b),
            n,
            2. / (n + 1) as f64,
            true
        ))),
        "HMA" => {
            let r = (n as f64).sqrt() as usize;
            let mut a = zip(&wma(x, n / 2), &wma(x, n), |a, b| 2. * a - b);
            for v in a.iter_mut().take(n + r - 2) {
                *v = f64::NAN;
            }
            json!(last(&wma(&a, r)))
        }
        "OnBalanceVolumeIndicator" => json!((0..=i)
            .map(|j| if j == 0 || d.c[j] >= d.c[j - 1] {
                d.v[j]
            } else {
                -d.v[j]
            })
            .sum::<f64>()),
        "RogersSatchell" => {
            let v = last(&sma(
                &(0..=i)
                    .map(|j| {
                        (d.h[j] / d.c[j]).ln() * (d.h[j] / d.o[j]).ln()
                            + (d.l[j] / d.c[j]).ln() * (d.l[j] / d.o[j]).ln()
                    })
                    .collect::<Vec<_>>(),
                n,
            ));
            json!(if v > 0. { v.sqrt() } else { f64::NAN })
        }
        "VWMA" => {
            let den = last(&sum(&d.v, n));
            if den == 0. {
                return Err(TaError::new(
                    ErrorCode::UndefinedResult,
                    "source VWMA zero volume",
                ));
            }
            json!(last(&sum(&zip(x, &d.v, |x, v| x * v), n)) / den)
        }
        "CHOP" => json!(
            100. * (last(&sum(&d.tr(true), n)) / (last(&hi(&d.h, n)) - last(&lo(&d.l, n)))).log10()
                / (n as f64).log10()
        ),
        "EaseOfMovementIndicator" => {
            let mid = zip(&d.h, &d.l, |a, b| (a + b) / 2.);
            let e = zip(
                &zip(&diff(&mid, 1), &zip(&d.h, &d.l, |a, b| a - b), |a, b| a * b),
                &d.v,
                |a, b| a / b * 100000000.,
            );
            json!(last(&sma(&e, n)))
        }
        "UO" | "UltimateOscillator" => ultimate(p, d),
        _ => fallback(base),
    };
    Ok(value)
}

pub(super) fn ultimate(p: &Value, d: &Data) -> Value {
    let bp = (0..d.len())
        .map(|j| {
            if j == 0 {
                f64::NAN
            } else {
                d.c[j] - d.l[j].min(d.c[j - 1])
            }
        })
        .collect::<Vec<_>>();
    let tr = d.tr(false);
    let mut v = 0.;
    for (key, default, w) in [("short", 7, 4.), ("mid", 14, 2.), ("long", 28, 1.)] {
        let n = period(p, key, default);
        v += w * last(&sum(&bp, n)) / last(&sum(&tr, n));
    }
    json!(100. * v / 7.)
}
