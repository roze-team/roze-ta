//! Paper-based research diagnostics. All observations and research metadata are explicit.
use super::*;
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use statrs::distribution::{ContinuousCDF, Normal};

pub const VERSION: &str = "roze-ta-research-v1/chacha8-rand_chacha-0.9.0";
pub const MAX_CANDIDATES: usize = 32;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct TrialSummary {
    pub experiment_id: String,
    pub available_at_ms: i64,
    pub total_trials: usize,
    pub effective_independent_trials: f64,
    /// Mean and sample standard deviation of unannualized trial Sharpe ratios.
    pub sharpe_mean: f64,
    pub sharpe_stddev: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ResearchRow {
    pub at_ms: i64,
    pub available_at_ms: i64,
    pub values: Vec<f64>,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum PboMetric {
    Mean,
    Sharpe,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ResearchTask {
    /// Points contain period excess returns, without annualization.
    SharpeInference {
        aggregation_periods: usize,
        benchmark_sharpe: f64,
        confidence: f64,
        assume_stationary: bool,
        assume_iid_for_psr: bool,
        trials: Option<TrialSummary>,
    },
    MultipleTesting {
        alpha: f64,
    },
    /// Points contain loss_1 - loss_2; positive means model 2 is better.
    DieboldMariano {
        hac_lags: usize,
        assume_stationary: bool,
    },
    /// Aligned period scores/returns; columns are candidates, higher is better.
    Pbo {
        candidates: Vec<String>,
        rows: Vec<ResearchRow>,
        blocks: usize,
        metric: PboMetric,
    },
    /// Aligned benchmark loss minus candidate loss; positive is better.
    RealityCheckSpa {
        candidates: Vec<String>,
        rows: Vec<ResearchRow>,
        block_length: usize,
        replicates: usize,
        seed: u64,
        hac_lags: usize,
        assume_stationary: bool,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct NamedValue {
    pub name: String,
    pub value: Scalar,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct ResearchResult {
    pub method_version: String,
    pub selected_samples: usize,
    pub selection_hash: String,
    pub values: Vec<NamedValue>,
    /// BH and Bonferroni adjusted p-values in original input order, or CSCV logits.
    pub series: Vec<NamedValue>,
    pub assumptions: Vec<String>,
}

fn invalid(s: &str) -> TaError {
    err(ErrorCode::InvalidParameter, s)
}
fn bounded(v: f64) -> Result<(), TaError> {
    if v.is_finite() && v.abs() <= 1e50 {
        Ok(())
    } else {
        Err(invalid(
            "research values must be finite with magnitude <=1e50",
        ))
    }
}
fn prefix(request: &Request, points: &[Point]) -> Result<(), TaError> {
    if request
        .points
        .iter()
        .take_while(|p| p.available_at_ms <= request.fit_cutoff_ms)
        .count()
        == points.len()
    {
        Ok(())
    } else {
        Err(err(
            ErrorCode::InvalidTime,
            "research series must select a contiguous available prefix",
        ))
    }
}

impl ResearchTask {
    pub(super) fn validate(
        &self,
        request: &Request,
        points: &[Point],
    ) -> Result<(usize, usize), TaError> {
        if !request.events.is_empty() {
            return Err(invalid("research does not accept events"));
        }
        for p in &request.points {
            bounded(p.x)?;
            if p.y.is_some() {
                return Err(invalid("research scalar tasks do not accept y"));
            }
        }
        let n = points.len();
        match self {
            Self::SharpeInference {
                aggregation_periods: q,
                benchmark_sharpe,
                confidence,
                assume_stationary,
                assume_iid_for_psr,
                trials,
            } => {
                if request.input_kind != "excess_simple_return" || request.units != "ratio" {
                    return Err(invalid(
                        "Sharpe inference requires excess_simple_return in ratio units",
                    ));
                }
                prefix(request, points)?;
                bounded(*benchmark_sharpe)?;
                if *q == 0
                    || *q > MAX_SAMPLES
                    || !confidence.is_finite()
                    || !(0.5..1.0).contains(confidence)
                    || !assume_stationary
                    || !assume_iid_for_psr
                {
                    return Err(invalid("q=1..4096, confidence in (0.5,1), stationarity and IID PSR assumptions required"));
                }
                if let Some(t) = trials {
                    text_field(&t.experiment_id)?;
                    bounded(t.sharpe_mean)?;
                    bounded(t.sharpe_stddev)?;
                    if t.available_at_ms <= 0 || t.available_at_ms > request.fit_cutoff_ms {
                        return Err(err(
                            ErrorCode::InvalidTime,
                            "trial metadata must be available by fit cutoff",
                        ));
                    }
                    if t.total_trials == 0
                        || t.total_trials > 1_000_000_000
                        || !t.effective_independent_trials.is_finite()
                        || t.effective_independent_trials < 1.0
                        || t.effective_independent_trials > t.total_trials as f64
                        || (t.effective_independent_trials > 1.0
                            && t.effective_independent_trials < 2.0)
                        || t.sharpe_stddev < 0.0
                    {
                        return Err(invalid("effective trials must equal 1 or be in [2,total_trials]; nonnegative trial standard deviation required"));
                    }
                }
                Ok((n * (q + 16), 32))
            }
            Self::MultipleTesting { alpha } => {
                if !alpha.is_finite() || *alpha <= 0.0 || *alpha >= 1.0 {
                    return Err(invalid("alpha must be in (0,1)"));
                }
                if request.input_kind != "p_value"
                    || request.units != "probability"
                    || request.points.iter().any(|p| !(0.0..=1.0).contains(&p.x))
                {
                    return Err(invalid(
                        "multiple testing requires p_value in probability units within [0,1]",
                    ));
                }
                Ok((n * 16, n * 4 + 16))
            }
            Self::DieboldMariano {
                hac_lags,
                assume_stationary,
            } => {
                prefix(request, points)?;
                if request.input_kind != "loss_difference"
                    || !assume_stationary
                    || *hac_lags > MAX_SAMPLES
                {
                    return Err(invalid(
                        "DM requires loss_difference, stationarity, and HAC lags <=4096",
                    ));
                }
                Ok((n * (hac_lags + 8), 16))
            }
            Self::Pbo {
                candidates,
                rows,
                blocks,
                ..
            }
            | Self::RealityCheckSpa {
                candidates,
                rows,
                block_length: blocks,
                ..
            } => {
                if !request.points.is_empty() {
                    return Err(invalid(
                        "matrix research data belongs in rows; points must be empty",
                    ));
                }
                if candidates.len() < 2
                    || candidates.len() > MAX_CANDIDATES
                    || rows.len() > MAX_SAMPLES
                {
                    return Err(err(
                        ErrorCode::LimitExceeded,
                        "research matrix limits: 2..32 candidates, at most 4096 rows",
                    ));
                }
                let mut ids = std::collections::BTreeSet::new();
                for id in candidates {
                    text_field(id)?;
                    if !ids.insert(id) {
                        return Err(invalid("candidate IDs must be unique"));
                    }
                }
                let mut previous = 0;
                let mut unavailable = false;
                let mut selected = 0;
                for row in rows {
                    if row.at_ms <= previous || row.available_at_ms < row.at_ms {
                        return Err(err(
                            ErrorCode::InvalidTime,
                            "matrix rows require increasing times and availability >= time",
                        ));
                    }
                    previous = row.at_ms;
                    if row.values.len() != candidates.len() {
                        return Err(invalid("matrix row width must equal candidate count"));
                    }
                    for v in &row.values {
                        bounded(*v)?;
                    }
                    if row.available_at_ms <= request.fit_cutoff_ms {
                        if unavailable {
                            return Err(err(
                                ErrorCode::InvalidTime,
                                "matrix selection must be a contiguous available prefix",
                            ));
                        }
                        selected += 1;
                    } else {
                        unavailable = true;
                    }
                }
                match self {
                    Self::Pbo { .. } => {
                        if request.input_kind != "candidate_performance"
                            || !(2..=10).contains(blocks)
                            || blocks % 2 != 0
                        {
                            return Err(invalid(
                                "PBO requires candidate_performance and even blocks 2..10",
                            ));
                        }
                        if selected < *blocks || selected % blocks != 0 {
                            return Err(err(
                                ErrorCode::InsufficientData,
                                "PBO selected rows must divide into equal nonempty blocks",
                            ));
                        }
                        let splits = (0..(1u32 << blocks))
                            .filter(|v| v.count_ones() as usize == blocks / 2)
                            .count();
                        Ok((selected * candidates.len() * splits * 4, splits + 32))
                    }
                    Self::RealityCheckSpa {
                        replicates,
                        hac_lags,
                        assume_stationary,
                        ..
                    } => {
                        if request.input_kind != "benchmark_loss_advantage"
                            || !assume_stationary
                            || *blocks == 0
                            || *blocks > selected
                            || *hac_lags >= selected
                        {
                            return Err(invalid("RC/SPA requires benchmark_loss_advantage, stationarity, block=1..n and HAC lags <n"));
                        }
                        if selected < 3 {
                            return Err(err(
                                ErrorCode::InsufficientData,
                                "RC/SPA needs at least three selected rows",
                            ));
                        }
                        if !(99..=4096).contains(replicates) {
                            return Err(err(
                                ErrorCode::LimitExceeded,
                                "99..4096 replicates required",
                            ));
                        }
                        Ok((
                            selected * candidates.len() * (replicates + hac_lags + 16),
                            64,
                        ))
                    }
                    _ => Err(invalid("invalid matrix task")),
                }
            }
        }
    }
}

fn value(name: impl Into<String>, x: f64) -> NamedValue {
    NamedValue {
        name: name.into(),
        value: Scalar::number(x),
    }
}
fn undefined(name: &str, reason: &str) -> NamedValue {
    NamedValue {
        name: name.into(),
        value: Scalar::undefined(reason),
    }
}
fn mean(x: &[f64]) -> f64 {
    x.iter().sum::<f64>() / x.len() as f64
}
fn moments(x: &[f64]) -> (f64, f64, f64, f64) {
    let m = mean(x);
    let (mut a, mut b, mut c) = (0.0, 0.0, 0.0);
    for v in x {
        let d = v - m;
        a += d * d;
        b += d.powi(3);
        c += d.powi(4);
    }
    let n = x.len() as f64;
    let v = a / n;
    (
        m,
        (a / (n - 1.0)).sqrt(),
        b / n / v.powf(1.5),
        c / n / v.powi(2),
    )
}
fn hac(x: &[f64], lags: usize) -> f64 {
    let n = x.len();
    let m = mean(x);
    let covariance = |lag: usize| {
        x[lag..]
            .iter()
            .zip(&x[..n - lag])
            .map(|(a, b)| (a - m) * (b - m))
            .sum::<f64>()
            / n as f64
    };
    let mut v = covariance(0);
    for k in 1..=lags {
        v += 2.0 * (1.0 - k as f64 / (lags + 1) as f64) * covariance(k);
    }
    v
}

pub(super) fn calculate(
    task: &ResearchTask,
    request: &Request,
    points: &[Point],
    checkpoint: &mut impl FnMut() -> Result<(), TaError>,
) -> Result<ResearchResult, TaError> {
    let normal = Normal::new(0.0, 1.0).map_err(|_| invalid("normal construction failed"))?;
    let xs: Vec<_> = points.iter().map(|p| p.x).collect();
    let mut result = ResearchResult {
        method_version: VERSION.into(),
        selected_samples: xs.len(),
        selection_hash: fingerprint::digest(&points)?,
        values: vec![],
        series: vec![],
        assumptions: vec![],
    };
    match task {
        ResearchTask::SharpeInference {
            aggregation_periods: q,
            benchmark_sharpe,
            confidence,
            trials,
            ..
        } => {
            result.assumptions.push("period excess returns; sample SD (n-1); biased central skewness and non-excess kurtosis; Lo uses centered autocovariance with divisor n; PSR is an IID asymptotic statistic, not a Bayesian posterior".into());
            if xs.len() < 3 {
                result.values.push(NamedValue {
                    name: "sharpe_inference".into(),
                    value: Scalar::InsufficientData,
                });
                return Ok(result);
            }
            let n = xs.len() as f64;
            let (m, sd, skew, kurt) = moments(&xs);
            if sd <= 0.0 {
                result
                    .values
                    .push(undefined("sharpe_inference", "zero_variance"));
                return Ok(result);
            }
            let sr = m / sd;
            result.values.extend([
                value("period_sharpe", sr),
                value("skewness", skew),
                value("non_excess_kurtosis", kurt),
            ]);
            if *q > xs.len() {
                result.values.push(NamedValue {
                    name: "lo_aggregated_sharpe".into(),
                    value: Scalar::InsufficientData,
                });
            } else {
                let variance = xs.iter().map(|v| (v - m).powi(2)).sum::<f64>();
                let mut factor = 1.0;
                for k in 1..*q {
                    checkpoint()?;
                    let rho = xs[k..]
                        .iter()
                        .zip(&xs[..xs.len() - k])
                        .map(|(a, b)| (a - m) * (b - m))
                        .sum::<f64>()
                        / variance;
                    factor += 2.0 * (1.0 - k as f64 / *q as f64) * rho;
                }
                result.values.push(if factor > 0.0 {
                    value("lo_aggregated_sharpe", sr * (*q as f64 / factor).sqrt())
                } else {
                    undefined("lo_aggregated_sharpe", "nonpositive_aggregation_variance")
                });
            }
            let variance = 1.0 - skew * sr + (kurt - 1.0) * sr * sr / 4.0;
            if variance <= 0.0 {
                result
                    .values
                    .push(undefined("psr", "nonpositive_asymptotic_variance"));
                return Ok(result);
            }
            let psr = |benchmark| normal.cdf((sr - benchmark) * ((n - 1.0) / variance).sqrt());
            result.values.push(value("psr", psr(*benchmark_sharpe)));
            result.values.push(if sr > *benchmark_sharpe {
                value(
                    "minimum_track_record_length",
                    1.0 + (normal.inverse_cdf(*confidence) / (sr - benchmark_sharpe)).powi(2)
                        * variance,
                )
            } else {
                undefined(
                    "minimum_track_record_length",
                    "estimated_sharpe_not_above_benchmark",
                )
            });
            if let Some(t) = trials {
                let effective = t.effective_independent_trials;
                let gamma = 0.577_215_664_901_532_9;
                let threshold = if effective == 1.0 {
                    t.sharpe_mean
                } else {
                    t.sharpe_mean
                        + t.sharpe_stddev
                            * ((1.0 - gamma) * normal.inverse_cdf(1.0 - 1.0 / effective)
                                + gamma
                                    * normal
                                        .inverse_cdf(1.0 - 1.0 / (effective * std::f64::consts::E)))
                };
                result.values.extend([
                    value("selection_threshold_sharpe", threshold),
                    value("dsr", psr(threshold)),
                ]);
                result.assumptions.push("DSR relies on caller-supplied complete trial summary and effective independent trial count; no independence count is inferred".into());
            }
        }
        ResearchTask::MultipleTesting { alpha } => {
            result.assumptions.push("BH step-up controls FDR under independent or suitable positive dependence; Bonferroni controls FWER without that dependence assumption; selected p-values define the entire supplied family".into());
            if xs.is_empty() {
                result.values.push(NamedValue {
                    name: "multiple_testing".into(),
                    value: Scalar::InsufficientData,
                });
                return Ok(result);
            }
            let mut order: Vec<_> = (0..xs.len()).collect();
            order.sort_by(|&a, &b| xs[a].total_cmp(&xs[b]));
            let mut adjusted = vec![1.0; xs.len()];
            let mut running: f64 = 1.0;
            for (rank, &i) in order.iter().enumerate().rev() {
                running = running.min(xs[i] * xs.len() as f64 / (rank + 1) as f64);
                adjusted[i] = running;
            }
            for (i, &p) in xs.iter().enumerate() {
                result.series.push(value(format!("bh_{i}"), adjusted[i]));
                result.series.push(value(
                    format!("bonferroni_{i}"),
                    (p * xs.len() as f64).min(1.0),
                ));
            }
            result.values.push(value(
                "bh_rejections",
                adjusted.iter().filter(|&&p| p <= *alpha).count() as f64,
            ));
        }
        ResearchTask::DieboldMariano { hac_lags, .. } => {
            result.assumptions.push("two-sided asymptotic normal DM; Bartlett Newey-West HAC with divisor n; supplied loss_1-loss_2, common units; no automatic horizon or small-sample correction".into());
            if xs.len() <= *hac_lags || xs.len() < 3 {
                result.values.push(NamedValue {
                    name: "dm_statistic".into(),
                    value: Scalar::InsufficientData,
                });
                return Ok(result);
            }
            let v = hac(&xs, *hac_lags);
            let m = mean(&xs);
            result.values.push(value("mean_loss_difference", m));
            if v <= 0.0 {
                result
                    .values
                    .push(undefined("dm_statistic", "nonpositive_long_run_variance"));
            } else {
                let stat = m / (v / xs.len() as f64).sqrt();
                result.values.extend([
                    value("long_run_variance", v),
                    value("dm_statistic", stat),
                    value("two_sided_p_value", 2.0 * normal.sf(stat.abs())),
                ]);
            }
        }
        ResearchTask::Pbo {
            candidates,
            rows,
            blocks,
            metric,
        } => {
            let selected: Vec<_> = rows
                .iter()
                .filter(|r| r.available_at_ms <= request.fit_cutoff_ms)
                .collect();
            result.selected_samples = selected.len();
            result.selection_hash = fingerprint::digest(&selected)?;
            result.assumptions.push("CSCV over equal contiguous blocks; all balanced combinations including complements; lowest candidate index wins IS ties; average ascending OOS rank; logit <=0 counts as overfit; frozen candidates are not refitted".into());
            let block_size = selected.len() / blocks;
            let mut count = 0;
            let mut bad = 0;
            for mask in 0..(1u32 << blocks) {
                if mask.count_ones() as usize != blocks / 2 {
                    continue;
                }
                checkpoint()?;
                let mut ins = vec![];
                let mut outs = vec![];
                for j in 0..candidates.len() {
                    let mut a = vec![];
                    let mut b = vec![];
                    for (i, row) in selected.iter().enumerate() {
                        if mask & (1 << (i / block_size)) != 0 {
                            a.push(row.values[j]);
                        } else {
                            b.push(row.values[j]);
                        }
                    }
                    let score = |v: &[f64]| match metric {
                        PboMetric::Mean => mean(v),
                        PboMetric::Sharpe => {
                            let (m, s, _, _) = moments(v);
                            m / s
                        }
                    };
                    ins.push(score(&a));
                    outs.push(score(&b));
                }
                if ins.iter().chain(&outs).any(|v| !v.is_finite()) {
                    result.series.clear();
                    result
                        .values
                        .push(undefined("pbo", "undefined_candidate_score_in_split"));
                    return Ok(result);
                }
                let mut winner = 0;
                for i in 1..ins.len() {
                    if ins[i] > ins[winner] {
                        winner = i;
                    }
                }
                let less = outs.iter().filter(|&&v| v < outs[winner]).count() as f64;
                let equal = outs.iter().filter(|&&v| v == outs[winner]).count() as f64;
                let omega = (less + (equal + 1.0) / 2.0) / (candidates.len() + 1) as f64;
                let logit = (omega / (1.0 - omega)).ln();
                if logit <= 0.0 {
                    bad += 1;
                }
                result
                    .series
                    .push(value(format!("split_{mask}_winner_{winner}"), logit));
                count += 1;
            }
            result.values.extend([
                value("pbo", bad as f64 / count as f64),
                value("splits", count as f64),
            ]);
        }
        ResearchTask::RealityCheckSpa {
            candidates,
            rows,
            block_length,
            replicates,
            seed,
            hac_lags,
            ..
        } => {
            let selected: Vec<_> = rows
                .iter()
                .filter(|r| r.available_at_ms <= request.fit_cutoff_ms)
                .collect();
            result.selected_samples = selected.len();
            result.selection_hash = fingerprint::digest(&selected)?;
            let n = selected.len();
            let m = candidates.len();
            let root = (n as f64).sqrt();
            let columns: Vec<Vec<_>> = (0..m)
                .map(|j| selected.iter().map(|r| r.values[j]).collect())
                .collect();
            let means: Vec<_> = columns.iter().map(|v| mean(v)).collect();
            let omega: Vec<_> = columns.iter().map(|v| hac(v, *hac_lags).sqrt()).collect();
            let rc = means
                .iter()
                .map(|v| root * v)
                .fold(f64::NEG_INFINITY, f64::max);
            let spa_valid = omega.iter().all(|v| v.is_finite() && *v > 0.0);
            let spa = means
                .iter()
                .zip(&omega)
                .map(|(v, s)| root * v / s)
                .fold(0.0, f64::max);
            let threshold = -(2.0 * (n as f64).ln().ln()).sqrt();
            let recenter: Vec<_> = means
                .iter()
                .zip(&omega)
                .map(|(&v, &s)| if root * v / s <= threshold { v } else { 0.0 })
                .collect();
            let mut rng = ChaCha8Rng::seed_from_u64(*seed);
            let mut rc_count = 0;
            let mut spa_count = 0;
            for _ in 0..*replicates {
                checkpoint()?;
                let mut sums = vec![0.0; m];
                let mut take = 0;
                while take < n {
                    let start = rng.random_range(0..n as u64) as usize;
                    for k in 0..(*block_length).min(n - take) {
                        for (j, s) in sums.iter_mut().enumerate() {
                            *s += selected[(start + k) % n].values[j];
                        }
                    }
                    take += (*block_length).min(n - take);
                }
                let boot_rc = sums
                    .iter()
                    .zip(&means)
                    .map(|(s, mu)| root * (s / n as f64 - mu))
                    .fold(f64::NEG_INFINITY, f64::max);
                if boot_rc >= rc {
                    rc_count += 1;
                }
                if spa_valid {
                    let boot_spa = (0..m)
                        .map(|j| root * (sums[j] / n as f64 - means[j] + recenter[j]) / omega[j])
                        .fold(0.0, f64::max);
                    if boot_spa >= spa {
                        spa_count += 1;
                    }
                }
            }
            result.values.extend([
                value("reality_check_statistic", rc),
                value(
                    "reality_check_p_value",
                    (rc_count + 1) as f64 / (replicates + 1) as f64,
                ),
            ]);
            if spa_valid {
                result.values.extend([
                    value("spa_statistic", spa),
                    value(
                        "spa_consistent_p_value",
                        (spa_count + 1) as f64 / (replicates + 1) as f64,
                    ),
                ]);
            } else {
                result.values.push(undefined(
                    "spa_consistent_p_value",
                    "candidate_has_nonpositive_long_run_variance",
                ));
            }
            result.assumptions.push("joint circular-block bootstrap preserves cross-candidate alignment; fixed Bartlett HAC studentization; Hansen consistent negative-mean recentering; p=(1+exceedances)/(B+1); complete frozen candidate family required".into());
        }
    }
    checkpoint()?;
    Ok(result)
}
