pub(super) fn rolling(a: &[f64], n: usize, f: impl Fn(&[f64]) -> f64) -> Vec<f64> {
    (0..a.len())
        .map(|i| {
            if n == 0 || i + 1 < n {
                f64::NAN
            } else {
                let w = &a[i + 1 - n..=i];
                if w.iter().all(|x| x.is_finite()) {
                    f(w)
                } else {
                    f64::NAN
                }
            }
        })
        .collect()
}
pub(super) fn mean(a: &[f64]) -> f64 {
    a.iter().sum::<f64>() / a.len() as f64
}
pub(super) fn recursive_sma(a: &[f64], n: usize) -> Vec<f64> {
    let mut v = sma(a, n);
    for j in n..a.len() {
        if v[j - 1].is_finite() && a[j - n].is_finite() && a[j].is_finite() {
            v[j] = v[j - 1] - (a[j - n] - a[j]) / n as f64;
        }
    }
    v
}
pub(super) fn weighted_ema(a: &[f64], n: usize) -> Vec<f64> {
    let mut v = ema(a, n, 2. / (n + 1) as f64, false);
    let alpha = 2. / (n + 1) as f64;
    for j in 1..a.len() {
        if v[j - 1].is_finite() && a[j].is_finite() {
            v[j] = alpha * a[j] + (1. - alpha) * v[j - 1];
        }
    }
    v
}
pub(super) fn partial(a: &[f64], n: usize, f: impl Fn(&[f64]) -> f64) -> Vec<f64> {
    (0..a.len())
        .map(|i| f(&a[(i + 1).saturating_sub(n)..=i]))
        .collect()
}
pub(super) fn stoch(a: &[f64], h: &[f64], l: &[f64], n: usize, flat: f64) -> Vec<f64> {
    let hh = hi(h, n);
    let ll = lo(l, n);
    (0..a.len())
        .map(|i| {
            if hh[i] == ll[i] {
                flat
            } else {
                100. * (a[i] - ll[i]) / (hh[i] - ll[i])
            }
        })
        .collect()
}
pub(super) fn obv(c: &[f64], v: &[f64]) -> Vec<f64> {
    let mut s = 0.;
    (0..c.len())
        .map(|i| {
            s += if i == 0 || c[i] > c[i - 1] {
                v[i]
            } else if c[i] < c[i - 1] {
                -v[i]
            } else {
                0.
            };
            s
        })
        .collect()
}
pub(super) fn wilder_sum(a: &[f64], n: usize) -> Vec<f64> {
    let mut out = vec![f64::NAN; a.len()];
    let Some(start) = a.iter().position(|x| x.is_finite()) else {
        return out;
    };
    if start + n > a.len() {
        return out;
    }
    let mut s = a[start..start + n - 1].iter().sum::<f64>();
    for i in start + n - 1..a.len() {
        s = s * (n - 1) as f64 / n as f64 + a[i];
        out[i] = s;
    }
    out
}
pub(super) fn covariance(a: &[f64], b: &[f64]) -> f64 {
    let ma = mean(a);
    let mb = mean(b);
    a.iter()
        .zip(b)
        .map(|(a, b)| (a - ma) * (b - mb))
        .sum::<f64>()
        / (a.len() - 1) as f64
}
pub(super) fn correlation(a: &[f64], b: &[f64]) -> f64 {
    covariance(a, b) / (covariance(a, a) * covariance(b, b)).sqrt()
}
pub(super) fn sma(a: &[f64], n: usize) -> Vec<f64> {
    rolling(a, n, mean)
}
pub(super) fn sum(a: &[f64], n: usize) -> Vec<f64> {
    rolling(a, n, |a| a.iter().sum())
}
pub(super) fn std(a: &[f64], n: usize, sample: bool) -> Vec<f64> {
    rolling(a, n, |a| {
        let m = mean(a);
        (a.iter().map(|v| (v - m).powi(2)).sum::<f64>() / (n - usize::from(sample)) as f64).sqrt()
    })
}
pub(super) fn hi(a: &[f64], n: usize) -> Vec<f64> {
    rolling(a, n, |a| {
        a.iter().copied().fold(f64::NEG_INFINITY, f64::max)
    })
}
pub(super) fn lo(a: &[f64], n: usize) -> Vec<f64> {
    rolling(a, n, |a| a.iter().copied().fold(f64::INFINITY, f64::min))
}
pub(super) fn zip(a: &[f64], b: &[f64], f: impl Fn(f64, f64) -> f64) -> Vec<f64> {
    a.iter().zip(b).map(|(a, b)| f(*a, *b)).collect()
}
pub(super) fn shift(a: &[f64], n: usize) -> Vec<f64> {
    (0..a.len())
        .map(|i| if i < n { f64::NAN } else { a[i - n] })
        .collect()
}
pub(super) fn diff(a: &[f64], n: usize) -> Vec<f64> {
    zip(a, &shift(a, n), |a, b| a - b)
}
pub(super) fn roc(a: &[f64], n: usize, scale: f64) -> Vec<f64> {
    zip(a, &shift(a, n), |a, b| (a / b - 1.) * scale)
}
pub(super) fn ema(a: &[f64], n: usize, alpha: f64, first_seed: bool) -> Vec<f64> {
    let mut out = vec![f64::NAN; a.len()];
    let mut state = f64::NAN;
    let mut seed = Vec::new();
    for (i, x) in a.iter().copied().enumerate() {
        if !x.is_finite() {
            continue;
        }
        seed.push(x);
        if first_seed {
            state = if state.is_nan() {
                x
            } else {
                state + (x - state) * alpha
            };
            if seed.len() >= n {
                out[i] = state;
            }
        } else if seed.len() == n {
            state = mean(&seed);
            out[i] = state;
        } else if seed.len() > n {
            state += (x - state) * alpha;
            out[i] = state;
        }
    }
    out
}
pub(super) fn wma(a: &[f64], n: usize) -> Vec<f64> {
    rolling(a, n, |a| {
        a.iter()
            .enumerate()
            .map(|(i, v)| (i + 1) as f64 * v)
            .sum::<f64>()
            / (n * (n + 1) / 2) as f64
    })
}
pub(super) fn rsi(a: &[f64], n: usize, first_seed: bool, flat: f64) -> Vec<f64> {
    let d = diff(a, 1);
    let up = d
        .iter()
        .map(|x| {
            if x.is_nan() {
                if first_seed {
                    0.
                } else {
                    f64::NAN
                }
            } else {
                x.max(0.)
            }
        })
        .collect::<Vec<_>>();
    let dn = d
        .iter()
        .map(|x| {
            if x.is_nan() {
                if first_seed {
                    0.
                } else {
                    f64::NAN
                }
            } else {
                (-x).max(0.)
            }
        })
        .collect::<Vec<_>>();
    zip(
        &ema(&up, n, 1. / n as f64, first_seed),
        &ema(&dn, n, 1. / n as f64, first_seed),
        |u, d| {
            if u == 0. && d == 0. {
                flat
            } else if d == 0. {
                100.
            } else {
                100. - 100. / (1. + u / d)
            }
        },
    )
}
