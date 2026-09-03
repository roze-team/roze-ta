//! Versioned retrospective evaluation; caller-supplied data, no account actions.
use super::*;
use statrs::distribution::{Continuous, ContinuousCDF, Normal};
use std::collections::{BTreeMap, BTreeSet};

pub const VERSION: &str = "roze-ta-evaluation-v1";

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum ReturnBasis {
    NetOfCosts,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum ReturnSpacing {
    ConsecutiveTradingPeriods,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct PerformanceSpec {
    pub periods_per_year: f64,
    pub risk_free_per_period: f64,
    pub mar_per_period: f64,
    pub ddof: u8,
    pub tail_level: f64,
    pub include_gaussian_tail: bool,
    pub hac_lags: usize,
    pub benchmark_id: Option<String>,
    pub return_basis: ReturnBasis,
    pub spacing: ReturnSpacing,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct EquityPoint {
    pub at_ms: i64,
    pub available_at_ms: i64,
    pub cumulative_return: Scalar,
    pub drawdown: f64,
    pub underwater_periods: usize,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct TailRisk {
    pub method: String,
    pub level: f64,
    pub loss_definition: String,
    pub value_at_risk: f64,
    pub expected_shortfall: f64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct HacEstimate {
    pub method: String,
    pub lags: usize,
    pub autocorrelations: Vec<Scalar>,
    pub mean_excess_standard_error: Scalar,
    pub mean_excess_t_statistic: Scalar,
    pub effective_sample_count: Scalar,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct PerformanceResult {
    pub method_version: String,
    pub sample_count: usize,
    pub spec: PerformanceSpec,
    pub arithmetic_mean_return: f64,
    pub cumulative_return: Scalar,
    pub annualized_return: Scalar,
    pub annualized_volatility: f64,
    pub annualized_sharpe: Scalar,
    pub annualized_sortino: Scalar,
    pub annualized_information_ratio: Option<Scalar>,
    pub annualized_tracking_error: Option<f64>,
    pub maximum_drawdown: f64,
    pub maximum_underwater_periods: usize,
    pub calmar: Scalar,
    pub historical_tail: TailRisk,
    pub gaussian_tail: Option<TailRisk>,
    pub hac: HacEstimate,
    pub equity_curve: Vec<EquityPoint>,
    pub assumptions: Vec<String>,
}
impl PerformanceSpec {
    pub(super) fn validate(&self, request: &Request, points: &[Point]) -> Result<(), TaError> {
        validate_ddof(self.ddof)?;
        level(self.tail_level)?;
        if !self.periods_per_year.is_finite()
            || !(1.0..=1_000_000.0).contains(&self.periods_per_year)
            || self.hac_lags > 128
        {
            return Err(err(
                ErrorCode::InvalidParameter,
                "annualization must be 1..1000000; HAC lags <=128",
            ));
        }
        for r in [self.risk_free_per_period, self.mar_per_period] {
            validate_return(r)?;
        }
        if request.input_kind != "simple_return" || request.units != "ratio" {
            return Err(err(
                ErrorCode::InvalidParameter,
                "performance requires input_kind=simple_return and units=ratio",
            ));
        }
        if let Some(id) = &self.benchmark_id {
            text_field(id)?;
        }
        if points.len() < 2 {
            return Err(err(
                ErrorCode::InsufficientData,
                "performance requires two selected returns",
            ));
        }
        if self.hac_lags >= points.len() {
            return Err(err(
                ErrorCode::InvalidParameter,
                "HAC lags must be less than selected sample count",
            ));
        }
        if request
            .points
            .iter()
            .take_while(|p| p.available_at_ms <= request.fit_cutoff_ms)
            .count()
            != points.len()
        {
            return Err(err(ErrorCode::InvalidTime,"performance requires a contiguous available prefix; delayed interior returns cannot be skipped"));
        }
        for p in points {
            validate_return(p.x)?;
            match (self.benchmark_id.is_some(), p.y) {
                (true, Some(y)) => validate_return(y)?,
                (false, None) => {}
                _ => {
                    return Err(err(
                        ErrorCode::InvalidSample,
                        "benchmark_id and aligned y returns must both be supplied or both absent",
                    ))
                }
            }
        }
        Ok(())
    }
}
fn validate_return(r: f64) -> Result<(), TaError> {
    number(r)?;
    if r <= -1.0 {
        Err(err(ErrorCode::InvalidSample,"simple returns and per-period targets must exceed -1; ruined/negative equity requires a separate model"))
    } else {
        Ok(())
    }
}
fn mean(xs: &[f64]) -> f64 {
    xs.iter().sum::<f64>() / xs.len() as f64
}
fn variance(xs: &[f64], ddof: u8) -> f64 {
    let m = mean(xs);
    xs.iter().map(|x| (x - m).powi(2)).sum::<f64>() / (xs.len() - ddof as usize) as f64
}
fn ratio(a: f64, b: f64, reason: &str) -> Scalar {
    if b == 0.0 {
        Scalar::undefined(reason)
    } else {
        Scalar::number(a / b)
    }
}

pub(super) fn performance(
    spec: &PerformanceSpec,
    points: &[Point],
    checkpoint: &mut impl FnMut() -> Result<(), TaError>,
) -> Result<PerformanceResult, TaError> {
    let xs: Vec<_> = points.iter().map(|p| p.x).collect();
    let n = xs.len();
    let m = mean(&xs);
    let var = variance(&xs, spec.ddof);
    let scale = spec.periods_per_year.sqrt();
    let excess = m - spec.risk_free_per_period;
    let downside = (xs
        .iter()
        .map(|r| (r - spec.mar_per_period).min(0.0).powi(2))
        .sum::<f64>()
        / n as f64)
        .sqrt();
    let (mut log_equity, mut log_peak, mut peak_index, mut max_duration, mut maximum_drawdown) =
        (0.0_f64, 0.0_f64, 0, 0, 0.0_f64);
    let mut latest_available = 0;
    let mut curve = Vec::with_capacity(n);
    for (i, p) in points.iter().enumerate() {
        checkpoint()?;
        log_equity += p.x.ln_1p();
        latest_available = latest_available.max(p.available_at_ms);
        // A mathematical recovery can be a few ULPs below the previous log peak.
        let recovery_tolerance = 8.0 * f64::EPSILON * (1.0 + log_equity.abs() + log_peak.abs());
        let recovered = log_equity >= log_peak - recovery_tolerance;
        if recovered {
            log_peak = log_equity.max(log_peak);
            peak_index = i + 1;
        }
        let drawdown = if recovered {
            0.0
        } else {
            -(log_equity - log_peak).exp_m1()
        };
        let duration = if drawdown > 0.0 {
            i + 1 - peak_index
        } else {
            0
        };
        maximum_drawdown = maximum_drawdown.max(drawdown);
        max_duration = max_duration.max(duration);
        curve.push(EquityPoint {
            at_ms: p.at_ms,
            available_at_ms: latest_available,
            cumulative_return: Scalar::number(log_equity.exp_m1()),
            drawdown,
            underwater_periods: duration,
        });
    }
    let annual = (log_equity / n as f64 * spec.periods_per_year).exp_m1();
    let (information, tracking) = if spec.benchmark_id.is_some() {
        let active: Vec<_> = points
            .iter()
            .map(|p| {
                p.y.map(|y| p.x - y)
                    .ok_or_else(|| err(ErrorCode::InvalidSample, "missing benchmark return"))
            })
            .collect::<Result<_, _>>()?;
        let sd = variance(&active, spec.ddof).sqrt();
        (
            Some(ratio(mean(&active) * scale, sd, "zero_tracking_error")),
            Some(sd * scale),
        )
    } else {
        (None, None)
    };
    let losses = statistics::sorted(xs.iter().map(|x| -x));
    let quantile = losses[((spec.tail_level * n as f64).ceil() as usize)
        .saturating_sub(1)
        .min(n - 1)];
    let tail_mass = (1.0 - spec.tail_level) * n as f64;
    let mut remaining = tail_mass;
    let mut tail_sum = 0.0;
    for loss in losses.iter().rev() {
        checkpoint()?;
        let weight = remaining.min(1.0);
        tail_sum += weight * loss;
        remaining -= weight;
        if remaining <= 0.0 {
            break;
        }
    }
    let tail = TailRisk {
        method: "empirical_inverse_cdf_fractional_upper_tail".into(),
        level: spec.tail_level,
        loss_definition: "negative_one_period_net_simple_return; no_zero_floor".into(),
        value_at_risk: quantile,
        expected_shortfall: tail_sum / tail_mass,
    };
    let gaussian = if spec.include_gaussian_tail {
        let normal = Normal::new(0.0, 1.0)
            .map_err(|_| err(ErrorCode::NumericalFailure, "normal initialization failed"))?;
        let z = normal.inverse_cdf(spec.tail_level);
        let sd = var.sqrt();
        Some(TailRisk {
            method: "gaussian_explicit_assumption".into(),
            level: spec.tail_level,
            loss_definition: tail.loss_definition.clone(),
            value_at_risk: -m + sd * z,
            expected_shortfall: -m + sd * normal.pdf(z) / (1.0 - spec.tail_level),
        })
    } else {
        None
    };
    let centered: Vec<_> = xs.iter().map(|x| x - m).collect();
    let gamma0 = centered.iter().map(|x| x * x).sum::<f64>() / n as f64;
    let mut long_run = gamma0;
    let mut autocorrelations = Vec::with_capacity(spec.hac_lags);
    for lag in 1..=spec.hac_lags {
        checkpoint()?;
        let gamma = centered[lag..]
            .iter()
            .zip(&centered[..n - lag])
            .map(|(a, b)| a * b)
            .sum::<f64>()
            / n as f64;
        autocorrelations.push(ratio(gamma, gamma0, "zero_variance"));
        long_run += 2.0 * (1.0 - lag as f64 / (spec.hac_lags + 1) as f64) * gamma;
    }
    let hac = if long_run > 0.0 {
        let se = (long_run / n as f64).sqrt();
        HacEstimate {
            method: "newey_west_mean_bartlett_divisor_n_no_small_sample_correction".into(),
            lags: spec.hac_lags,
            autocorrelations,
            mean_excess_standard_error: Scalar::number(se),
            mean_excess_t_statistic: ratio(excess, se, "zero_standard_error"),
            effective_sample_count: Scalar::number(n as f64 * gamma0 / long_run),
        }
    } else {
        HacEstimate {
            method: "newey_west_mean_bartlett_divisor_n_no_small_sample_correction".into(),
            lags: spec.hac_lags,
            autocorrelations,
            mean_excess_standard_error: if long_run == 0.0 {
                Scalar::number(0.0)
            } else {
                Scalar::undefined("nonpositive_long_run_variance")
            },
            mean_excess_t_statistic: Scalar::undefined("nonpositive_long_run_variance"),
            effective_sample_count: Scalar::undefined("nonpositive_long_run_variance"),
        }
    };
    Ok(PerformanceResult{method_version:VERSION.into(),sample_count:n,spec:spec.clone(),arithmetic_mean_return:m,cumulative_return:Scalar::number(log_equity.exp_m1()),annualized_return:Scalar::number(annual),annualized_volatility:var.sqrt()*scale,
        annualized_sharpe:ratio(excess*scale,var.sqrt(),"zero_volatility"),annualized_sortino:ratio((m-spec.mar_per_period)*scale,downside,"zero_downside_deviation"),annualized_information_ratio:information,annualized_tracking_error:tracking,
        maximum_drawdown,maximum_underwater_periods:max_duration,calmar:ratio(annual,maximum_drawdown,"zero_maximum_drawdown"),historical_tail:tail,gaussian_tail:gaussian,hac,equity_curve:curve,
        assumptions:vec!["caller supplies consecutive trading-period net simple returns; no cash-flow adjustment or calendar inference".into(),"sqrt(A) ratio/volatility annualization assumes suitable stationary dependence structure; HAC is reported separately".into(),"tail estimates describe one-period loss; Gaussian model only when explicitly requested".into(),"HAC effective n is an approximation for the mean and may exceed n with negative dependence; no p-value or significance decision".into()]})
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct Trade {
    pub id: String,
    pub closed_at_ms: i64,
    pub available_at_ms: i64,
    pub gross_pnl: f64,
    pub costs: f64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct TradeSpec {
    pub currency: String,
    pub cost_definition: String,
    pub trades: Vec<Trade>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct TradeResult {
    pub method_version: String,
    pub currency: String,
    pub cost_definition: String,
    pub selection_hash: String,
    pub selected_ids: Vec<String>,
    pub selected_range_ms: [i64; 2],
    pub excluded_unavailable: usize,
    pub trade_count: usize,
    pub wins: usize,
    pub losses: usize,
    pub breakeven: usize,
    pub gross_pnl: f64,
    pub costs: f64,
    pub net_pnl: f64,
    pub win_rate: f64,
    pub mean_net_pnl: f64,
    pub average_win: Scalar,
    pub average_loss_magnitude: Scalar,
    pub payoff_ratio: Scalar,
    pub profit_factor: Scalar,
    pub maximum_consecutive_wins: usize,
    pub maximum_consecutive_losses: usize,
    pub assumptions: Vec<String>,
}
impl TradeSpec {
    pub(super) fn validate(&self) -> Result<(), TaError> {
        text_field(&self.currency)?;
        text_field(&self.cost_definition)?;
        if self.trades.is_empty() || self.trades.len() > MAX_SAMPLES {
            return Err(err(ErrorCode::LimitExceeded, "trade count must be 1..4096"));
        }
        let mut ids = BTreeSet::new();
        let mut previous = 0;
        for t in &self.trades {
            text_field(&t.id)?;
            number(t.gross_pnl)?;
            number(t.costs)?;
            if !ids.insert(&t.id) {
                return Err(err(ErrorCode::DuplicateSample, "duplicate trade ID"));
            }
            if t.closed_at_ms <= previous || t.available_at_ms < t.closed_at_ms {
                return Err(err(
                    ErrorCode::InvalidTime,
                    "trades require strictly increasing close times and available >= close",
                ));
            }
            if t.costs < 0.0 {
                return Err(err(
                    ErrorCode::InvalidSample,
                    "trade costs must be nonnegative",
                ));
            }
            previous = t.closed_at_ms;
        }
        Ok(())
    }
}
pub(super) fn trades(
    spec: &TradeSpec,
    cutoff: i64,
    checkpoint: &mut impl FnMut() -> Result<(), TaError>,
) -> Result<TradeResult, TaError> {
    let selected: Vec<_> = spec
        .trades
        .iter()
        .filter(|t| t.available_at_ms <= cutoff)
        .collect();
    if spec
        .trades
        .iter()
        .take_while(|t| t.available_at_ms <= cutoff)
        .count()
        != selected.len()
    {
        return Err(err(
            ErrorCode::InvalidTime,
            "trade streaks require an available prefix; interior unknown trades cannot be skipped",
        ));
    }
    let Some((first, last)) = selected.first().zip(selected.last()) else {
        return Err(err(
            ErrorCode::InsufficientData,
            "no available closed trades",
        ));
    };
    let (
        mut wins,
        mut losses,
        mut gross,
        mut costs,
        mut positive,
        mut negative,
        mut win_run,
        mut loss_run,
        mut max_wins,
        mut max_losses,
    ) = (0, 0, 0., 0., 0., 0., 0, 0, 0, 0);
    for t in &selected {
        checkpoint()?;
        gross += t.gross_pnl;
        costs += t.costs;
        let net = t.gross_pnl - t.costs;
        if net > 0.0 {
            wins += 1;
            positive += net;
            win_run += 1;
            loss_run = 0;
        } else if net < 0.0 {
            losses += 1;
            negative -= net;
            loss_run += 1;
            win_run = 0;
        } else {
            win_run = 0;
            loss_run = 0;
        }
        max_wins = max_wins.max(win_run);
        max_losses = max_losses.max(loss_run);
    }
    let n = selected.len();
    let avg_win = if wins > 0 {
        Scalar::number(positive / wins as f64)
    } else {
        Scalar::InsufficientData
    };
    let avg_loss = if losses > 0 {
        Scalar::number(negative / losses as f64)
    } else {
        Scalar::InsufficientData
    };
    Ok(TradeResult{method_version:VERSION.into(),currency:spec.currency.clone(),cost_definition:spec.cost_definition.clone(),selection_hash:fingerprint::digest(&selected)?,selected_ids:selected.iter().map(|t|t.id.clone()).collect(),selected_range_ms:[first.closed_at_ms,last.closed_at_ms],excluded_unavailable:spec.trades.len()-n,
        trade_count:n,wins,losses,breakeven:n-wins-losses,gross_pnl:gross,costs,net_pnl:gross-costs,win_rate:wins as f64/n as f64,mean_net_pnl:(gross-costs)/n as f64,average_win:avg_win,average_loss_magnitude:avg_loss,
        payoff_ratio:if wins>0&&losses>0{ratio(positive/wins as f64,negative/losses as f64,"zero_average_loss")}else{Scalar::InsufficientData},profit_factor:ratio(positive,negative,"no_losing_trades"),maximum_consecutive_wins:max_wins,maximum_consecutive_losses:max_losses,
        assumptions:vec!["gross PnL excludes the explicitly supplied total costs; costs deducted exactly once".into(),"wins/losses classified by net PnL; breakeven trades remain in win-rate denominator and interrupt streaks".into(),"currency conversion and overlapping position equity are not inferred from trade records".into()]})
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct FactorObservation {
    pub asset_id: String,
    pub at_ms: i64,
    pub factor_available_at_ms: i64,
    pub label_end_ms: i64,
    pub label_available_at_ms: i64,
    pub score: f64,
    pub future_return: Option<f64>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct FactorSpec {
    pub factor_id: String,
    pub universe_id: String,
    pub universe_size: usize,
    pub label_definition: String,
    pub horizon_ms: i64,
    pub minimum_cross_section: usize,
    pub ddof: u8,
    pub observations: Vec<FactorObservation>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct FactorRow {
    pub at_ms: i64,
    pub available_at_ms: i64,
    pub sample_count: usize,
    pub coverage: f64,
    pub information_coefficient: Scalar,
    pub rank_information_coefficient: Scalar,
    pub direction_hit_rate: Scalar,
    pub direction_count: usize,
    pub zero_direction_count: usize,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct FactorSummary {
    pub valid_groups: usize,
    pub mean: Scalar,
    pub standard_deviation: Scalar,
    pub information_ratio: Scalar,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct FactorResult {
    pub method_version: String,
    pub factor_id: String,
    pub universe_id: String,
    pub label_definition: String,
    pub horizon_ms: i64,
    pub selection_hash: String,
    pub excluded_immature_groups: usize,
    pub excluded_small_groups: usize,
    pub excluded_overlapping_groups: usize,
    pub ic: FactorSummary,
    pub rank_ic: FactorSummary,
    pub rows: Vec<FactorRow>,
    pub assumptions: Vec<String>,
}
impl FactorSpec {
    pub(super) fn validate(&self) -> Result<(), TaError> {
        text_field(&self.factor_id)?;
        text_field(&self.universe_id)?;
        text_field(&self.label_definition)?;
        validate_ddof(self.ddof)?;
        if self.horizon_ms <= 0
            || self.minimum_cross_section < 2
            || self.minimum_cross_section > self.universe_size
            || self.universe_size > MAX_SAMPLES
        {
            return Err(err(
                ErrorCode::InvalidParameter,
                "positive horizon; 2 <= minimum cross section <= universe size <=4096 required",
            ));
        }
        if self.observations.is_empty() || self.observations.len() > MAX_SAMPLES {
            return Err(err(
                ErrorCode::LimitExceeded,
                "factor observations must be 1..4096",
            ));
        }
        let mut keys = BTreeSet::new();
        for o in &self.observations {
            text_field(&o.asset_id)?;
            number(o.score)?;
            if let Some(r) = o.future_return {
                number(r)?;
            }
            if o.at_ms <= 0
                || o.factor_available_at_ms <= 0
                || o.factor_available_at_ms > o.at_ms
                || o.at_ms.checked_add(self.horizon_ms) != Some(o.label_end_ms)
                || o.label_available_at_ms < o.label_end_ms
            {
                return Err(err(ErrorCode::InvalidTime,"factor must be known at observation time; label interval/availability must match horizon"));
            }
            if !keys.insert((o.at_ms, &o.asset_id)) {
                return Err(err(
                    ErrorCode::DuplicateSample,
                    "duplicate factor asset/time pair",
                ));
            }
        }
        Ok(())
    }
}
fn factor_summary(values: impl Iterator<Item = f64>, ddof: u8) -> FactorSummary {
    let xs: Vec<_> = values.collect();
    let n = xs.len();
    let mean_value = if n > 0 {
        Scalar::number(mean(&xs))
    } else {
        Scalar::InsufficientData
    };
    if n <= ddof as usize {
        return FactorSummary {
            valid_groups: n,
            mean: mean_value,
            standard_deviation: Scalar::InsufficientData,
            information_ratio: Scalar::InsufficientData,
        };
    }
    let sd = variance(&xs, ddof).sqrt();
    FactorSummary {
        valid_groups: n,
        mean: mean_value,
        standard_deviation: Scalar::number(sd),
        information_ratio: ratio(mean(&xs), sd, "zero_ic_standard_deviation"),
    }
}
pub(super) fn factors(
    spec: &FactorSpec,
    cutoff: i64,
    checkpoint: &mut impl FnMut() -> Result<(), TaError>,
) -> Result<FactorResult, TaError> {
    let mut groups: BTreeMap<i64, Vec<&FactorObservation>> = BTreeMap::new();
    for o in &spec.observations {
        groups.entry(o.at_ms).or_default().push(o);
    }
    let (mut immature, mut small, mut overlap, mut last_end) = (0, 0, 0, 0);
    let mut rows = Vec::new();
    let mut selected = Vec::new();
    for (at, mut group) in groups {
        checkpoint()?;
        group.sort_by(|a, b| a.asset_id.cmp(&b.asset_id));
        if group.len() > spec.universe_size {
            return Err(err(
                ErrorCode::InvalidSample,
                "cross section exceeds declared universe size",
            ));
        }
        if group.iter().any(|o| o.label_available_at_ms > cutoff) {
            immature += 1;
            continue;
        }
        if group.iter().any(|o| o.future_return.is_none()) {
            return Err(err(ErrorCode::InvalidSample, "mature factor label missing"));
        }
        if group.len() < spec.minimum_cross_section {
            small += 1;
            continue;
        }
        if at < last_end {
            overlap += 1;
            continue;
        }
        let ys: Vec<_> = group
            .iter()
            .map(|o| {
                o.future_return
                    .ok_or_else(|| err(ErrorCode::InvalidSample, "missing label"))
            })
            .collect::<Result<_, _>>()?;
        let xs: Vec<_> = group.iter().map(|o| o.score).collect();
        let (_, _, xx, yy, xy) = statistics::sums(&xs, &ys);
        let (_, _, rxx, ryy, rxy) =
            statistics::sums(&statistics::ranks(&xs), &statistics::ranks(&ys));
        let (mut direction_count, mut hits) = (0, 0);
        for (x, y) in xs.iter().zip(&ys) {
            if *x != 0.0 && *y != 0.0 {
                direction_count += 1;
                if x.is_sign_positive() == y.is_sign_positive() {
                    hits += 1;
                }
            }
        }
        let available = group
            .iter()
            .map(|o| o.label_available_at_ms)
            .max()
            .ok_or_else(|| err(ErrorCode::InsufficientData, "empty cross section"))?;
        last_end = group[0].label_end_ms;
        rows.push(FactorRow {
            at_ms: at,
            available_at_ms: available,
            sample_count: group.len(),
            coverage: group.len() as f64 / spec.universe_size as f64,
            information_coefficient: statistics::correlation(xx, yy, xy),
            rank_information_coefficient: statistics::correlation(rxx, ryy, rxy),
            direction_hit_rate: if direction_count == 0 {
                Scalar::InsufficientData
            } else {
                Scalar::number(hits as f64 / direction_count as f64)
            },
            direction_count,
            zero_direction_count: group.len() - direction_count,
        });
        selected.extend(group);
    }
    if rows.is_empty() {
        return Err(err(
            ErrorCode::InsufficientData,
            "no mature nonoverlapping cross sections",
        ));
    }
    let value = |s: &Scalar| match s {
        Scalar::Ready { value } => Some(*value),
        _ => None,
    };
    let ic = factor_summary(
        rows.iter()
            .filter_map(|r| value(&r.information_coefficient)),
        spec.ddof,
    );
    let rank_ic = factor_summary(
        rows.iter()
            .filter_map(|r| value(&r.rank_information_coefficient)),
        spec.ddof,
    );
    Ok(FactorResult{method_version:VERSION.into(),factor_id:spec.factor_id.clone(),universe_id:spec.universe_id.clone(),label_definition:spec.label_definition.clone(),horizon_ms:spec.horizon_ms,selection_hash:fingerprint::digest(&selected)?,excluded_immature_groups:immature,excluded_small_groups:small,excluded_overlapping_groups:overlap,ic,rank_ic,rows,
        assumptions:vec!["cross-sectional IC per observation time; average ranks for ties; no pooled time-series correlation".into(),"entire cross section waits for all supplied labels; chronological nonoverlapping label intervals selected".into(),"coverage uses caller-declared universe size; universe membership and survivorship bias require caller provenance".into(),"ICIR is unannualized mean/std across valid groups; constant groups remain undefined and valid group counts are reported".into(),"direction hit rate excludes zero scores/returns and is descriptive, not a calibrated probability".into()]})
}
