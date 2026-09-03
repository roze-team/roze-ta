use super::*;
use statrs::distribution::StudentsT;

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum Transform {
    SimpleReturn,
    LogReturn,
    CumulativeReturn,
    Lag,
    Difference,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct SeriesValue {
    pub at_ms: i64,
    pub available_at_ms: i64,
    pub sample_count: usize,
    pub value: Scalar,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct Description {
    pub sample_count: usize,
    pub ddof: u8,
    pub mean: f64,
    pub median: f64,
    pub minimum: f64,
    pub maximum: f64,
    pub variance: Scalar,
    pub standard_deviation: Scalar,
    pub moment_skewness: Scalar,
    pub excess_moment_kurtosis: Scalar,
    pub quantile_method: String,
    pub quantiles: Vec<[f64; 2]>,
    /// [observed value, fraction <= value], with ties collapsed.
    pub ecdf: Vec<[f64; 2]>,
    pub mad_unscaled: f64,
    pub iqr: f64,
    pub trimmed_mean: f64,
    pub trimmed_sample_count: usize,
    pub winsorized_values: Vec<f64>,
    pub winsorized_sample_count: usize,
    pub mean_confidence_interval: Option<Interval>,
    pub mean_interval_unavailable_reason: Option<String>,
}

pub(super) fn sorted(values: impl Iterator<Item = f64>) -> Vec<f64> {
    let mut values: Vec<_> = values.collect();
    values.sort_by(f64::total_cmp);
    values
}
/// R type 7: h=(n-1)p, interpolate adjacent order statistics.
pub(super) fn quantile(values: &[f64], p: f64) -> f64 {
    let h = (values.len() - 1) as f64 * p;
    let i = h.floor() as usize;
    let j = (i + 1).min(values.len() - 1);
    values[i] + (values[j] - values[i]) * (h - i as f64)
}
fn moments(values: &[f64]) -> (f64, f64) {
    let mut mean = 0.0;
    let mut m2 = 0.0;
    for (i, x) in values.iter().enumerate() {
        let delta = x - mean;
        mean += delta / (i + 1) as f64;
        m2 += delta * (x - mean);
    }
    (mean, m2.max(0.0))
}
pub(super) fn describe(
    points: &[Point],
    ddof: u8,
    qs: &[f64],
    trim: f64,
    level: f64,
) -> Result<Description, TaError> {
    if points.is_empty() {
        return Err(err(
            ErrorCode::InsufficientData,
            "description requires selected samples",
        ));
    }
    let values: Vec<_> = points.iter().map(|p| p.x).collect();
    let order = sorted(values.iter().copied());
    let n = values.len();
    let (mean, m2) = moments(&values);
    let variance = if n > ddof as usize {
        Scalar::number(m2 / (n - ddof as usize) as f64)
    } else {
        Scalar::InsufficientData
    };
    let stddev = if n > ddof as usize {
        Scalar::number((m2 / (n - ddof as usize) as f64).sqrt())
    } else {
        Scalar::InsufficientData
    };
    let median = quantile(&order, 0.5);
    let mad = quantile(&sorted(values.iter().map(|x| (x - median).abs())), 0.5);
    let scale = (m2 / n as f64).sqrt();
    let (skew, kurt) = if scale == 0.0 {
        (
            Scalar::undefined("constant_sample"),
            Scalar::undefined("constant_sample"),
        )
    } else {
        (
            Scalar::number(
                values
                    .iter()
                    .map(|x| ((x - mean) / scale).powi(3))
                    .sum::<f64>()
                    / n as f64,
            ),
            Scalar::number(
                values
                    .iter()
                    .map(|x| ((x - mean) / scale).powi(4))
                    .sum::<f64>()
                    / n as f64
                    - 3.0,
            ),
        )
    };
    let k = (n as f64 * trim).floor() as usize;
    let kept = &order[k..n - k];
    let low = quantile(&order, trim);
    let high = quantile(&order, 1.0 - trim);
    let mut ecdf = Vec::new();
    for (i, &x) in order.iter().enumerate() {
        if i + 1 == n || order[i + 1] != x {
            ecdf.push([x, (i + 1) as f64 / n as f64]);
        }
    }
    let mean_ci = if n >= 2 {
        let t = StudentsT::new(0.0, 1.0, (n - 1) as f64)
            .map_err(|_| err(ErrorCode::NumericalFailure, "Student-t construction failed"))?;
        let margin = distribution::student_quantile(&t, 0.5 + level / 2.0, &mut || Ok(()))?
            * (m2 / (n - 1) as f64 / n as f64).sqrt();
        Some(Interval {
            interval_type: "frequentist_confidence".into(),
            method: "student_t_mean".into(),
            level,
            lower: mean - margin,
            upper: mean + margin,
            assumptions: vec![
                "IID normal observations; no autocorrelation or heteroskedasticity correction"
                    .into(),
                "variance for mean interval always uses ddof=1".into(),
            ],
        })
    } else {
        None
    };
    Ok(Description {
        sample_count: n,
        ddof,
        mean,
        median,
        minimum: order[0],
        maximum: order[n - 1],
        variance,
        standard_deviation: stddev,
        moment_skewness: skew,
        excess_moment_kurtosis: kurt,
        quantile_method: "R_type_7".into(),
        quantiles: qs.iter().map(|&q| [q, quantile(&order, q)]).collect(),
        ecdf,
        mad_unscaled: mad,
        iqr: quantile(&order, 0.75) - quantile(&order, 0.25),
        trimmed_mean: moments(kept).0,
        trimmed_sample_count: kept.len(),
        winsorized_values: values.iter().map(|x| x.clamp(low, high)).collect(),
        winsorized_sample_count: n,
        mean_confidence_interval: mean_ci,
        mean_interval_unavailable_reason: if n < 2 {
            Some("requires_two_samples".into())
        } else {
            None
        },
    })
}

pub(super) fn transform(
    points: &[Point],
    kind: Transform,
    lag: usize,
    checkpoint: &mut impl FnMut() -> Result<(), TaError>,
) -> Result<Vec<SeriesValue>, TaError> {
    let returns = matches!(
        kind,
        Transform::SimpleReturn | Transform::LogReturn | Transform::CumulativeReturn
    );
    if returns && points.iter().any(|p| p.x <= 0.0) {
        return Err(err(
            ErrorCode::InvalidSample,
            "return transforms require strictly positive prices",
        ));
    }
    if matches!(kind, Transform::CumulativeReturn) && lag != 1 {
        return Err(err(
            ErrorCode::InvalidParameter,
            "cumulative_return requires lag=1 and is relative to first selected price",
        ));
    }
    let mut result = Vec::with_capacity(points.len());
    for (i, p) in points.iter().enumerate() {
        checkpoint()?;
        let start = if matches!(kind, Transform::CumulativeReturn) {
            0
        } else {
            i.saturating_sub(lag)
        };
        let (value, count) = if i < lag && !matches!(kind, Transform::CumulativeReturn) {
            (Scalar::InsufficientData, i + 1)
        } else {
            let base = points[start].x;
            let value = match kind {
                Transform::Lag => base,
                Transform::Difference => p.x - base,
                Transform::SimpleReturn | Transform::CumulativeReturn => p.x / base - 1.0,
                Transform::LogReturn => p.x.ln() - base.ln(),
            };
            (Scalar::number(value), i - start + 1)
        };
        // Historical inputs may have delayed availability; output waits for all dependencies.
        let available = points[start].available_at_ms.max(p.available_at_ms);
        result.push(SeriesValue {
            at_ms: p.at_ms,
            available_at_ms: available,
            sample_count: count,
            value,
        });
    }
    Ok(result)
}
pub(super) fn zscore(
    points: &[Point],
    window: usize,
    ddof: u8,
    robust: bool,
    checkpoint: &mut impl FnMut() -> Result<(), TaError>,
) -> Result<Vec<SeriesValue>, TaError> {
    let mut result = Vec::with_capacity(points.len());
    for (i, p) in points.iter().enumerate() {
        checkpoint()?;
        let start = (i + 1).saturating_sub(window);
        let segment = &points[start..=i];
        let value = if segment.len() < window {
            Scalar::InsufficientData
        } else {
            let xs: Vec<_> = segment.iter().map(|p| p.x).collect();
            let (center, scale) = if robust {
                let center = quantile(&sorted(xs.iter().copied()), 0.5);
                (
                    center,
                    1.482602218505602
                        * quantile(&sorted(xs.iter().map(|x| (x - center).abs())), 0.5),
                )
            } else {
                let (mean, m2) = moments(&xs);
                (mean, (m2 / (window - ddof as usize) as f64).sqrt())
            };
            if scale == 0.0 {
                Scalar::undefined("zero_scale_window")
            } else {
                Scalar::number((p.x - center) / scale)
            }
        };
        result.push(SeriesValue {
            at_ms: p.at_ms,
            available_at_ms: segment
                .iter()
                .map(|p| p.available_at_ms)
                .max()
                .unwrap_or(p.available_at_ms),
            sample_count: segment.len(),
            value,
        });
    }
    Ok(result)
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct PairEstimate {
    pub at_ms: i64,
    pub available_at_ms: i64,
    pub sample_count: usize,
    pub ddof: u8,
    pub pearson: Scalar,
    pub spearman: Scalar,
    /// Row/column order x,y. OLS is y = intercept + slope*x.
    pub covariance_matrix: [[Scalar; 2]; 2],
    pub slope_beta_y_on_x: Scalar,
    pub intercept: Scalar,
    pub r_squared: Scalar,
    /// Full fit: one residual per input; rolling fit: only last residual.
    pub residuals: Vec<Scalar>,
    pub degrees_of_freedom: Option<usize>,
}
pub(super) fn ranks(xs: &[f64]) -> Vec<f64> {
    let mut order: Vec<_> = xs.iter().copied().enumerate().collect();
    order.sort_by(|a, b| a.1.total_cmp(&b.1));
    let mut ranks = vec![0.0; xs.len()];
    let mut i = 0;
    while i < order.len() {
        let mut j = i + 1;
        while j < order.len() && order[j].1 == order[i].1 {
            j += 1;
        }
        let rank = (i + j - 1) as f64 / 2.0 + 1.0;
        for row in &order[i..j] {
            ranks[row.0] = rank;
        }
        i = j;
    }
    ranks
}
pub(super) fn sums(xs: &[f64], ys: &[f64]) -> (f64, f64, f64, f64, f64) {
    let (mx, sxx) = moments(xs);
    let (my, syy) = moments(ys);
    let sxy = xs.iter().zip(ys).map(|(x, y)| (x - mx) * (y - my)).sum();
    (mx, my, sxx, syy, sxy)
}
pub(super) fn correlation(xx: f64, yy: f64, xy: f64) -> Scalar {
    if xx == 0.0 || yy == 0.0 {
        Scalar::undefined("constant_series")
    } else {
        Scalar::number((xy / xx.sqrt() / yy.sqrt()).clamp(-1.0, 1.0))
    }
}
pub(super) fn pairs(
    points: &[Point],
    ddof: u8,
    window: Option<usize>,
    checkpoint: &mut impl FnMut() -> Result<(), TaError>,
) -> Result<Vec<PairEstimate>, TaError> {
    if points.len() < 2 {
        return Err(err(
            ErrorCode::InsufficientData,
            "pair analysis requires at least two aligned samples",
        ));
    }
    let mut results = Vec::new();
    let ends: Box<dyn Iterator<Item = usize>> = if window.is_some() {
        Box::new(0..points.len())
    } else {
        Box::new(std::iter::once(points.len() - 1))
    };
    for end in ends {
        checkpoint()?;
        let start = window.map_or(0, |w| (end + 1).saturating_sub(w));
        let ps = &points[start..=end];
        let xs: Vec<_> = ps.iter().map(|p| p.x).collect();
        let ys: Vec<_> = ps
            .iter()
            .map(|p| {
                p.y.ok_or_else(|| err(ErrorCode::InvalidSample, "missing paired value"))
            })
            .collect::<Result<_, _>>()?;
        let (mx, my, xx, yy, xy) = sums(&xs, &ys);
        let enough = ps.len() >= window.unwrap_or(2);
        let scalar = |v| {
            if enough {
                Scalar::number(v)
            } else {
                Scalar::InsufficientData
            }
        };
        let corr = |xx, yy, xy| {
            if enough {
                correlation(xx, yy, xy)
            } else {
                Scalar::InsufficientData
            }
        };
        let (_, _, rxx, ryy, rxy) = sums(&ranks(&xs), &ranks(&ys));
        let singular = xx == 0.0;
        let beta = xy / xx;
        let intercept = my - beta * mx;
        let coefficient = |v| {
            if !enough {
                Scalar::InsufficientData
            } else if singular {
                Scalar::undefined("singular_predictor")
            } else {
                Scalar::number(v)
            }
        };
        let denom = (ps.len() as f64 - ddof as f64).max(1.0);
        let cov = scalar(xy / denom);
        let residuals = xs
            .iter()
            .zip(&ys)
            .enumerate()
            .filter(|(i, _)| window.is_none() || *i == xs.len() - 1)
            .map(|(_, (x, y))| coefficient((y - my) - beta * (x - mx)))
            .collect();
        results.push(PairEstimate {
            at_ms: points[end].at_ms,
            available_at_ms: ps
                .iter()
                .map(|p| p.available_at_ms)
                .max()
                .unwrap_or(points[end].available_at_ms),
            sample_count: ps.len(),
            ddof,
            pearson: corr(xx, yy, xy),
            spearman: corr(rxx, ryy, rxy),
            covariance_matrix: [[scalar(xx / denom), cov.clone()], [cov, scalar(yy / denom)]],
            slope_beta_y_on_x: coefficient(beta),
            intercept: coefficient(intercept),
            r_squared: if !enough {
                Scalar::InsufficientData
            } else if singular || yy == 0.0 {
                Scalar::undefined("constant_series")
            } else {
                scalar((xy / xx.sqrt() / yy.sqrt()).powi(2).clamp(0.0, 1.0))
            },
            residuals,
            degrees_of_freedom: if enough && !singular {
                ps.len().checked_sub(2)
            } else {
                None
            },
        });
    }
    Ok(results)
}
