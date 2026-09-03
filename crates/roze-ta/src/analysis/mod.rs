//! Bounded, deterministic S1 analysis. No I/O, implicit fitting or hidden seed.
//! See docs/contracts/analysis-v1.md for formulas and sample selection semantics.
pub mod bootstrap;
pub mod calibration;
mod distribution;
pub mod evaluation;
mod probability;
mod statistics;
pub mod temporal;
use crate::{
    engine::SeriesIdentity,
    error::{ErrorCode, TaError},
    fingerprint,
};
pub use distribution::{Distribution, DistributionResult, DistributionTask, Evaluation, Sampling};
pub use probability::{
    infer_beta, BernoulliAssumption, BetaArtifact, BetaInference, BetaPrior, Event,
    ProbabilityResult, ProbabilityTask,
};
use serde::{Deserialize, Serialize};
pub use statistics::{Description, PairEstimate, SeriesValue, Transform};

pub const VERSION: &str = "roze-ta-analysis-v1.0/statrs-0.19.1/chacha8-rand_chacha-0.9.0";
pub const MAX_SAMPLES: usize = 4096;
pub const MAX_OPERATIONS: usize = 16;
pub const MAX_WORK: usize = 2_000_000;
pub const MAX_OUTPUT_VALUES: usize = 65_536;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct Point {
    pub at_ms: i64,
    pub available_at_ms: i64,
    pub x: f64,
    /// Aligned second series. Pair operations require y on every selected row.
    pub y: Option<f64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub schema_version: u32,
    pub identity: SeriesIdentity,
    /// Input interpretation, e.g. close_price or simple_return. No automatic conversion.
    pub input_kind: String,
    pub units: String,
    pub as_of_ms: i64,
    pub fit_cutoff_ms: i64,
    pub points: Vec<Point>,
    pub events: Vec<Event>,
    pub operations: Vec<Operation>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(tag = "method", rename_all = "snake_case", deny_unknown_fields)]
pub enum Operation {
    Describe {
        ddof: u8,
        quantiles: Vec<f64>,
        trim_fraction: f64,
        interval_level: f64,
    },
    Transform {
        kind: Transform,
        lag: usize,
    },
    RollingZscore {
        window: usize,
        ddof: u8,
        robust: bool,
    },
    Pair {
        ddof: u8,
        window: Option<usize>,
    },
    Distribution {
        task: DistributionTask,
    },
    Probability {
        task: ProbabilityTask,
    },
    InferBeta {
        artifact: Box<BetaArtifact>,
    },
    Performance {
        spec: evaluation::PerformanceSpec,
    },
    TradeSummary {
        spec: evaluation::TradeSpec,
    },
    FactorEvaluation {
        spec: evaluation::FactorSpec,
    },
    CalibrationEvaluation {
        spec: calibration::CalibrationSpec,
    },
    BootstrapMean {
        spec: bootstrap::BootstrapSpec,
    },
    TemporalSplit {
        spec: temporal::TemporalSpec,
    },
}

/// Unavailable and mathematically undefined values never masquerade as zero.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum Scalar {
    Ready { value: f64 },
    InsufficientData,
    Undefined { reason: String },
}
impl Scalar {
    fn number(value: f64) -> Self {
        if value.is_finite() {
            Self::Ready { value }
        } else {
            Self::Undefined {
                reason: "non_finite_numeric_result".into(),
            }
        }
    }
    fn undefined(reason: &str) -> Self {
        Self::Undefined {
            reason: reason.into(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct Interval {
    pub interval_type: String,
    pub method: String,
    pub level: f64,
    pub lower: f64,
    pub upper: f64,
    pub assumptions: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(tag = "analysis_kind", content = "result", rename_all = "snake_case")]
pub enum Output {
    Description(Description),
    Transform(Vec<SeriesValue>),
    Zscore(Vec<SeriesValue>),
    Pair(Vec<PairEstimate>),
    Distribution(DistributionResult),
    Probability(Box<ProbabilityResult>),
    BetaInference(BetaInference),
    Performance(Box<evaluation::PerformanceResult>),
    TradeSummary(evaluation::TradeResult),
    FactorEvaluation(evaluation::FactorResult),
    CalibrationEvaluation(Box<calibration::CalibrationResult>),
    BootstrapMean(Box<bootstrap::BootstrapResult>),
    TemporalSplit(Box<temporal::TemporalResult>),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct ResultSet {
    pub schema_version: u32,
    pub method_version: String,
    pub hash_protocol: String,
    pub identity: SeriesIdentity,
    pub input_kind: String,
    pub units: String,
    pub as_of_ms: i64,
    pub fit_cutoff_ms: i64,
    pub input_hash: String,
    pub output_hash: String,
    pub supplied_points: usize,
    pub selected_points: usize,
    pub selected_range_ms: Option<[i64; 2]>,
    pub selection_hash: String,
    pub specifications: Vec<Operation>,
    pub assumptions: Vec<String>,
    pub results: Vec<Output>,
}

pub fn calculate(request: &Request) -> Result<ResultSet, TaError> {
    calculate_controlled(request, || Ok(()))
}

pub fn calculate_controlled(
    request: &Request,
    mut checkpoint: impl FnMut() -> Result<(), TaError>,
) -> Result<ResultSet, TaError> {
    checkpoint()?;
    request.identity.validate()?;
    text_field(&request.input_kind)?;
    text_field(&request.units)?;
    if request.schema_version != 1 {
        return Err(err(
            ErrorCode::InvalidParameter,
            "analysis schema_version must be 1",
        ));
    }
    if request.fit_cutoff_ms <= 0 || request.fit_cutoff_ms > request.as_of_ms {
        return Err(err(
            ErrorCode::InvalidTime,
            "require 0 < fit_cutoff <= as_of",
        ));
    }
    if request.points.len() > MAX_SAMPLES
        || request.events.len() > MAX_SAMPLES
        || request.operations.is_empty()
        || request.operations.len() > MAX_OPERATIONS
    {
        return Err(err(
            ErrorCode::LimitExceeded,
            "analysis limits: 4096 points/events, 1..16 operations",
        ));
    }
    let mut previous = 0;
    for p in &request.points {
        checkpoint()?;
        if p.at_ms <= previous || p.available_at_ms < p.at_ms {
            return Err(err(
                ErrorCode::InvalidTime,
                "points require increasing positive times and available_at >= at",
            ));
        }
        number(p.x)?;
        if let Some(y) = p.y {
            number(y)?;
        }
        previous = p.at_ms;
    }
    probability::validate_events(&request.events)?;
    let points: Vec<_> = request
        .points
        .iter()
        .filter(|p| p.available_at_ms <= request.fit_cutoff_ms)
        .cloned()
        .collect();
    let mut work = 0usize;
    let mut output_values = 0usize;
    // Validate every operation before allocating expensive output or hashing caller numbers.
    for op in &request.operations {
        let (w, out) = match op {
            Operation::Describe {
                ddof,
                quantiles,
                trim_fraction,
                interval_level,
            } => {
                validate_ddof(*ddof)?;
                level(*interval_level)?;
                if !trim_fraction.is_finite()
                    || !(0.0..0.5).contains(trim_fraction)
                    || quantiles.len() > 64
                    || quantiles
                        .iter()
                        .any(|p| !p.is_finite() || !(0.0..=1.0).contains(p))
                {
                    return Err(err(
                        ErrorCode::InvalidParameter,
                        "invalid trim fraction or quantiles",
                    ));
                }
                (points.len() * 16, points.len() * 3 + 64)
            }
            Operation::Transform { lag, .. } => {
                if *lag == 0 || *lag > MAX_SAMPLES {
                    return Err(err(ErrorCode::InvalidParameter, "lag must be 1..4096"));
                }
                (points.len(), points.len())
            }
            Operation::RollingZscore {
                window,
                ddof,
                robust,
            } => {
                validate_ddof(*ddof)?;
                validate_window(*window)?;
                if *robust && *ddof != 0 {
                    return Err(err(
                        ErrorCode::InvalidParameter,
                        "robust zscore requires ddof=0; MAD has no degrees-of-freedom correction",
                    ));
                }
                (points.len() * window * 16, points.len())
            }
            Operation::Pair { ddof, window } => {
                validate_ddof(*ddof)?;
                if let Some(w) = window {
                    validate_window(*w)?;
                }
                if points.iter().any(|p| p.y.is_none()) {
                    return Err(err(
                        ErrorCode::InvalidSample,
                        "pair operations require explicitly aligned x/y on every selected row",
                    ));
                }
                (
                    points.len() * window.unwrap_or(1) * 16,
                    if window.is_some() {
                        points.len() * 12
                    } else {
                        points.len() + 12
                    },
                )
            }
            Operation::Distribution { task } => {
                task.validate()?;
                (task.cost(points.len()), task.output_count())
            }
            Operation::Probability { task } => {
                task.validate()?;
                (request.events.len() * 16, 64)
            }
            Operation::InferBeta { artifact } => {
                infer_beta(artifact, request.as_of_ms)?;
                (128, 16)
            }
            Operation::Performance { spec } => {
                spec.validate(request, &points)?;
                (points.len() * (32 + spec.hac_lags), points.len() * 6 + 256)
            }
            Operation::TradeSummary { spec } => {
                spec.validate()?;
                (spec.trades.len() * 16, spec.trades.len() + 32)
            }
            Operation::FactorEvaluation { spec } => {
                spec.validate()?;
                (spec.observations.len() * 64, spec.observations.len() * 12)
            }
            Operation::CalibrationEvaluation { spec } => {
                spec.validate()?;
                (
                    spec.forecasts.len() * 64,
                    spec.forecasts.len() + spec.bin_edges.len() * 12 + 128,
                )
            }
            Operation::BootstrapMean { spec } => {
                spec.validate(request, &points)?;
                (points.len() * spec.replicates, spec.replicates + 64)
            }
            Operation::TemporalSplit { spec } => {
                spec.validate()?;
                (
                    spec.observations.len() * spec.folds.len() * 8,
                    spec.folds.len() * (spec.observations.len() + 64),
                )
            }
        };
        work = work.saturating_add(w);
        output_values = output_values.saturating_add(out);
    }
    if work > MAX_WORK || output_values > MAX_OUTPUT_VALUES {
        return Err(err(
            ErrorCode::LimitExceeded,
            "analysis work/output budget exceeded; reduce windows, samples or operations",
        ));
    }
    let input_hash = fingerprint::digest(request)?;
    let mut results = Vec::with_capacity(request.operations.len());
    for op in &request.operations {
        checkpoint()?;
        results.push(match op {
            Operation::Describe {
                ddof,
                quantiles,
                trim_fraction,
                interval_level,
            } => Output::Description(statistics::describe(
                &points,
                *ddof,
                quantiles,
                *trim_fraction,
                *interval_level,
            )?),
            Operation::Transform { kind, lag } => Output::Transform(statistics::transform(
                &points,
                *kind,
                *lag,
                &mut checkpoint,
            )?),
            Operation::RollingZscore {
                window,
                ddof,
                robust,
            } => Output::Zscore(statistics::zscore(
                &points,
                *window,
                *ddof,
                *robust,
                &mut checkpoint,
            )?),
            Operation::Pair { ddof, window } => {
                Output::Pair(statistics::pairs(&points, *ddof, *window, &mut checkpoint)?)
            }
            Operation::Distribution { task } => {
                Output::Distribution(distribution::calculate(task, &points, &mut checkpoint)?)
            }
            Operation::Probability { task } => Output::Probability(Box::new(
                probability::calculate(task, request, &input_hash, &mut checkpoint)?,
            )),
            Operation::InferBeta { artifact } => {
                Output::BetaInference(infer_beta(artifact, request.as_of_ms)?)
            }
            Operation::Performance { spec } => Output::Performance(Box::new(
                evaluation::performance(spec, &points, &mut checkpoint)?,
            )),
            Operation::TradeSummary { spec } => Output::TradeSummary(evaluation::trades(
                spec,
                request.fit_cutoff_ms,
                &mut checkpoint,
            )?),
            Operation::FactorEvaluation { spec } => Output::FactorEvaluation(evaluation::factors(
                spec,
                request.fit_cutoff_ms,
                &mut checkpoint,
            )?),
            Operation::CalibrationEvaluation { spec } => Output::CalibrationEvaluation(Box::new(
                calibration::calculate(spec, request.fit_cutoff_ms, &mut checkpoint)?,
            )),
            Operation::BootstrapMean { spec } => Output::BootstrapMean(Box::new(
                bootstrap::calculate(spec, &points, &mut checkpoint)?,
            )),
            Operation::TemporalSplit { spec } => Output::TemporalSplit(Box::new(
                temporal::calculate(spec, request.fit_cutoff_ms, &mut checkpoint)?,
            )),
        });
    }
    let mut result = ResultSet {
        schema_version: 1,
        method_version: VERSION.into(),
        hash_protocol: fingerprint::HASH_PROTOCOL.into(),
        identity: request.identity.clone(),
        input_kind: request.input_kind.clone(),
        units: request.units.clone(),
        as_of_ms: request.as_of_ms,
        fit_cutoff_ms: request.fit_cutoff_ms,
        input_hash,
        output_hash: String::new(),
        supplied_points: request.points.len(),
        selected_points: points.len(),
        selected_range_ms: points
            .first()
            .zip(points.last())
            .map(|(a, b)| [a.at_ms, b.at_ms]),
        selection_hash: fingerprint::digest(&points)?,
        specifications: request.operations.clone(),
        assumptions: vec![
            "caller supplies units, data identity and truthful availability; no imputation".into(),
            "statistics summarize selected history; correlation and OLS do not establish causation"
                .into(),
        ],
        results,
    };
    checkpoint()?;
    result.output_hash = fingerprint::digest(&result)?;
    Ok(result)
}

fn err(code: ErrorCode, message: &str) -> TaError {
    TaError::new(code, message)
}
fn text_field(value: &str) -> Result<(), TaError> {
    if value.trim().is_empty() || value.len() > 1024 {
        Err(err(
            ErrorCode::InvalidParameter,
            "text fields require 1..1024 bytes",
        ))
    } else {
        Ok(())
    }
}
fn number(x: f64) -> Result<(), TaError> {
    if !x.is_finite() || x.abs() > 1e50 {
        Err(err(
            ErrorCode::InvalidSample,
            "values must be finite with magnitude <= 1e50",
        ))
    } else {
        Ok(())
    }
}
fn level(x: f64) -> Result<(), TaError> {
    if x.is_finite() && (0.5..=0.999).contains(&x) {
        Ok(())
    } else {
        Err(err(
            ErrorCode::InvalidParameter,
            "interval level must be 0.5..0.999",
        ))
    }
}
fn validate_ddof(x: u8) -> Result<(), TaError> {
    if x <= 1 {
        Ok(())
    } else {
        Err(err(ErrorCode::InvalidParameter, "ddof must be 0 or 1"))
    }
}
fn validate_window(x: usize) -> Result<(), TaError> {
    if (2..=MAX_SAMPLES).contains(&x) {
        Ok(())
    } else {
        Err(err(ErrorCode::InvalidParameter, "window must be 2..4096"))
    }
}

/// Capabilities are families; window variants do not increase indicator counts.
pub fn catalog() -> serde_json::Value {
    serde_json::json!({"schema_version":1,"method_version":VERSION,"tool":"analysis_batch_calculate",
        "capabilities":[
            {"id":"stat_describe","capability_kind":"statistics","reuses":["stat_stddev","stat_percentile","stat_skew","stat_kurt"],"data":"timed_scalar","fits":false,"random":false,"online_update":false},
            {"id":"stat_transform","capability_kind":"statistics","data":"timed_scalar","fits":false,"random":false,"online_update":false},
            {"id":"stat_zscore","capability_kind":"statistics","data":"timed_scalar","fits":false,"random":false,"online_update":false},
            {"id":"stat_pair","capability_kind":"statistics","reuses":["stat_corr","stat_beta","stat_linreg"],"data":"aligned_timed_pair","fits":true,"random":false,"online_update":false},
            {"id":"prob_distribution","capability_kind":"probability","data":"explicit_parameters_or_timed_scalar","fits":false,"random":"only_when_sampling_requested","online_update":false},
            {"id":"prob_beta_binomial","capability_kind":"probability","data":"mature_event_intervals","fits":true,"random":false,"online_update":false},
            {"id":"eval_performance","capability_kind":"evaluation","data":"consecutive_net_simple_returns_and_optional_benchmark","method_version":evaluation::VERSION,"fits":false,"random":false,"online_update":false},
            {"id":"eval_trades","capability_kind":"evaluation","data":"closed_gross_pnl_and_explicit_costs","method_version":evaluation::VERSION,"fits":false,"random":false,"online_update":false},
            {"id":"eval_factor","capability_kind":"evaluation","data":"cross_sectional_scores_and_mature_forward_labels","method_version":evaluation::VERSION,"fits":false,"random":false,"online_update":false},
            {"id":"eval_calibration","capability_kind":"evaluation","data":"frozen_binary_forecasts_and_mature_labels","method_version":calibration::VERSION,"fits":false,"random":false,"online_update":false},
            {"id":"prob_bootstrap_mean","capability_kind":"simulation","data":"timed_scalar_with_explicit_sampling_assumption","method_version":bootstrap::VERSION,"fits":false,"random":true,"online_update":false},
            {"id":"stat_temporal_split","capability_kind":"statistics","data":"label_intervals_and_explicit_chronological_windows","method_version":temporal::VERSION,"fits":false,"random":false,"online_update":false}
        ],"limits":{"samples":MAX_SAMPLES,"operations":MAX_OPERATIONS,"work_units":MAX_WORK,"output_values":MAX_OUTPUT_VALUES,"distribution_samples":4096}})
}
