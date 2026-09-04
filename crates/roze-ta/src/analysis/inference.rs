//! Explicit statistical tests, estimators and reduced-rank time-series statistics.
use super::*;
use linalg::{dot, Matrix};
use research::NamedValue;
use statrs::distribution::{ContinuousCDF, StudentsT};
mod calibration_tables;
mod continuation;
mod fitting;
pub mod standard_calibration;
pub use continuation::VecmResult;
pub use fitting::FitResult;
pub const VERSION: &str = "roze-ta-inference-v1.2";
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum Trend {
    None,
    Constant,
    Linear,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum MleFamily {
    Normal,
    LogNormal,
    Poisson,
    Exponential,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum InferenceTask {
    AdfMacKinnon {
        samples: Vec<f64>,
        lags: usize,
        trend: Trend,
    },
    EngleGrangerMacKinnon {
        dependent: Vec<f64>,
        independent: Vec<f64>,
        lags: usize,
    },
    JohansenRank {
        observations: Matrix,
        lagged_differences: usize,
        include_constant: bool,
        significance: standard_calibration::Significance,
        test: standard_calibration::RankTest,
        forecast_steps: Option<usize>,
    },
    DynamicsFit {
        model: Box<dynamics::DynamicsTask>,
        lower: Vec<f64>,
        upper: Vec<f64>,
        max_iterations: usize,
        tolerance: f64,
    },
    AdfGaussianCalibration {
        samples: Vec<f64>,
        lags: usize,
        trend: Trend,
        replicates: usize,
        seed: u64,
    },
    EngleGrangerGaussianCalibration {
        dependent: Vec<f64>,
        independent: Vec<f64>,
        lags: usize,
        replicates: usize,
        seed: u64,
    },
    Vecm {
        observations: Matrix,
        lagged_differences: usize,
        include_constant: bool,
        rank: usize,
        steps: usize,
    },
    EngleGranger {
        dependent: Vec<f64>,
        independent: Vec<f64>,
        lags: usize,
    },
    OneSampleT {
        samples: Vec<f64>,
        null_mean: f64,
    },
    Welch {
        first: Vec<f64>,
        second: Vec<f64>,
    },
    CorrelationTest {
        first: Vec<f64>,
        second: Vec<f64>,
    },
    MaximumLikelihood {
        samples: Vec<f64>,
        family: MleFamily,
    },
    NormalMeanPosterior {
        samples: Vec<f64>,
        known_variance: f64,
        prior_mean: f64,
        prior_variance: f64,
    },
    Adf {
        samples: Vec<f64>,
        lags: usize,
        trend: Trend,
    },
    Johansen {
        observations: Matrix,
        lagged_differences: usize,
        include_constant: bool,
        rank: usize,
    },
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct InferenceSpec {
    pub available_at_ms: i64,
    pub task: InferenceTask,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct InferenceResult {
    pub method_version: String,
    pub values: Vec<NamedValue>,
    pub coefficients: Vec<f64>,
    pub eigenvalues: Vec<f64>,
    pub cointegration_vectors: Matrix,
    pub adjustment: Matrix,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vecm: Option<VecmResult>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fit: Option<FitResult>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mackinnon: Option<standard_calibration::MacKinnonCalibration>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rank_selection: Option<standard_calibration::RankSelection>,
    pub assumptions: Vec<String>,
}
fn bad(s: &str) -> TaError {
    err(ErrorCode::InvalidParameter, s)
}
fn vector(v: &[f64]) -> Result<(), TaError> {
    if v.len() > MAX_SAMPLES || v.iter().any(|v| !v.is_finite() || v.abs() > 1e20) {
        Err(bad(
            "inference sample length <=4096 and finite magnitude <=1e20 required",
        ))
    } else {
        Ok(())
    }
}
fn enough(n: usize, minimum: usize) -> Result<(), TaError> {
    if n >= minimum {
        Ok(())
    } else {
        Err(err(
            ErrorCode::InsufficientData,
            "insufficient inference observations",
        ))
    }
}
impl InferenceSpec {
    pub(super) fn validate(&self, r: &Request) -> Result<(usize, usize), TaError> {
        if r.input_kind != "inference_samples" || !r.points.is_empty() || !r.events.is_empty() {
            return Err(bad(
                "inference requires inference_samples and empty points/events",
            ));
        }
        if self.available_at_ms <= 0 || self.available_at_ms > r.fit_cutoff_ms {
            return Err(err(
                ErrorCode::InvalidTime,
                "inference sample/parameters must be known by fit cutoff",
            ));
        }
        let work = match &self.task {
            InferenceTask::AdfMacKinnon { .. }
            | InferenceTask::EngleGrangerMacKinnon { .. }
            | InferenceTask::JohansenRank { .. } => return standard_calibration::validate(self, r),
            InferenceTask::DynamicsFit { .. } => return fitting::validate(self, r),
            InferenceTask::AdfGaussianCalibration { .. }
            | InferenceTask::EngleGrangerGaussianCalibration { .. }
            | InferenceTask::Vecm { .. } => return continuation::validate(self, r),
            InferenceTask::EngleGranger {
                dependent,
                independent,
                lags,
            } => {
                vector(dependent)?;
                vector(independent)?;
                if dependent.len() != independent.len() || *lags > 16 {
                    return Err(bad("Engle-Granger requires aligned series and lags <=16"));
                }
                enough(dependent.len(), 2 * lags + 5)?;
                dependent.len() * (lags + 2).pow(2) * 8
            }
            InferenceTask::OneSampleT { samples, null_mean } => {
                vector(samples)?;
                vector(&[*null_mean])?;
                enough(samples.len(), 2)?;
                samples.len() * 16
            }
            InferenceTask::Welch { first, second }
            | InferenceTask::CorrelationTest { first, second } => {
                vector(first)?;
                vector(second)?;
                enough(first.len(), 3)?;
                enough(second.len(), 3)?;
                if matches!(self.task, InferenceTask::CorrelationTest { .. })
                    && first.len() != second.len()
                {
                    return Err(bad("correlation samples must be aligned"));
                }
                (first.len() + second.len()) * 16
            }
            InferenceTask::MaximumLikelihood { samples, family } => {
                vector(samples)?;
                enough(samples.len(), 1)?;
                if matches!(family, MleFamily::LogNormal | MleFamily::Exponential)
                    && samples.iter().any(|v| *v <= 0.0)
                {
                    return Err(bad("positive lognormal/exponential samples required"));
                }
                if matches!(family, MleFamily::Poisson)
                    && samples.iter().any(|v| *v < 0.0 || v.fract() != 0.0)
                {
                    return Err(bad("Poisson samples require nonnegative integers"));
                }
                samples.len() * 16
            }
            InferenceTask::NormalMeanPosterior {
                samples,
                known_variance,
                prior_mean,
                prior_variance,
            } => {
                vector(samples)?;
                vector(&[*known_variance, *prior_mean, *prior_variance])?;
                if *known_variance <= 0.0 || *prior_variance <= 0.0 {
                    return Err(bad("positive known/prior variance required"));
                }
                samples.len() * 16 + 32
            }
            InferenceTask::Adf {
                samples,
                lags,
                trend,
            } => {
                vector(samples)?;
                if *lags > 16 {
                    return Err(bad("ADF lags <=16"));
                }
                let p = 1
                    + lags
                    + match trend {
                        Trend::None => 0,
                        Trend::Constant => 1,
                        Trend::Linear => 2,
                    };
                enough(samples.len(), lags + p + 2)?;
                samples.len() * p * p * 8
            }
            InferenceTask::Johansen {
                observations,
                lagged_differences,
                include_constant,
                rank,
            } => {
                let d = observations.first().map_or(0, Vec::len);
                if !(2..=4).contains(&d)
                    || observations.len() > MAX_SAMPLES
                    || *lagged_differences > 8
                    || *rank == 0
                    || *rank > d
                {
                    return Err(bad("Johansen dimensions 2..4, differences <=8, rank 1..dimension and rows <=4096"));
                }
                for row in observations {
                    vector(row)?;
                    if row.len() != d {
                        return Err(bad("Johansen matrix must be rectangular"));
                    }
                }
                let p = d * lagged_differences + usize::from(*include_constant);
                enough(observations.len(), p + lagged_differences + d + 2)?;
                observations.len() * (p + d).pow(2) * 8 + 100 * d.pow(4)
            }
        };
        Ok((work, 512))
    }
}
fn nv(n: &str, v: f64) -> NamedValue {
    NamedValue {
        name: n.into(),
        value: Scalar::number(v),
    }
}
fn mean(x: &[f64]) -> f64 {
    x.iter().sum::<f64>() / x.len() as f64
}
fn variance(x: &[f64]) -> f64 {
    let m = mean(x);
    x.iter().map(|v| (v - m).powi(2)).sum::<f64>() / (x.len() - 1) as f64
}
fn t_result(values: &mut Vec<NamedValue>, numerator: f64, se: f64, df: f64) -> Result<(), TaError> {
    values.push(nv("standard_error", se));
    values.push(nv("degrees_of_freedom", df));
    if se > 0.0 && df.is_finite() && df > 0.0 {
        let statistic = numerator / se;
        let t = StudentsT::new(0.0, 1.0, df).map_err(|_| bad("invalid t degrees of freedom"))?;
        values.extend([
            nv("t_statistic", statistic),
            nv("two_sided_p_value", 2.0 * t.sf(statistic.abs())),
        ]);
    } else {
        values.push(NamedValue {
            name: "t_statistic".into(),
            value: Scalar::undefined("zero_sampling_variance"),
        });
    }
    Ok(())
}
fn residualize(y: &Matrix, z: &Matrix) -> Result<Matrix, TaError> {
    if z[0].is_empty() {
        return Ok(y.clone());
    }
    let mut columns = vec![];
    for col in linalg::transpose(y) {
        let b = linalg::least_squares(z, &col)?.0;
        columns.push(col.iter().zip(z).map(|(v, r)| v - dot(r, &b)).collect());
    }
    Ok(linalg::transpose(&columns))
}
fn cross(a: &Matrix, b: &Matrix) -> Matrix {
    let n = a.len() as f64;
    linalg::multiply(&linalg::transpose(a), b)
        .iter()
        .map(|r| r.iter().map(|v| v / n).collect())
        .collect()
}
pub(super) fn calculate(
    spec: &InferenceSpec,
    checkpoint: &mut impl FnMut() -> Result<(), TaError>,
) -> Result<InferenceResult, TaError> {
    let mut out = InferenceResult {
        method_version: VERSION.into(),
        values: vec![],
        coefficients: vec![],
        eigenvalues: vec![],
        cointegration_vectors: vec![],
        adjustment: vec![],
        vecm: None,
        fit: None,
        mackinnon: None,
        rank_selection: None,
        assumptions: vec![
            "explicit frozen samples; sampling assumptions require external validation".into(),
        ],
    };
    match &spec.task {
        InferenceTask::AdfMacKinnon { .. }
        | InferenceTask::EngleGrangerMacKinnon { .. }
        | InferenceTask::JohansenRank { .. } => return standard_calibration::run(spec, checkpoint),
        InferenceTask::DynamicsFit { .. } => {
            out.fit = Some(fitting::fit(spec, checkpoint)?);
            return Ok(out);
        }
        InferenceTask::AdfGaussianCalibration { .. }
        | InferenceTask::EngleGrangerGaussianCalibration { .. }
        | InferenceTask::Vecm { .. } => return continuation::calculate_extended(spec, checkpoint),
        InferenceTask::EngleGranger {
            dependent,
            independent,
            lags,
        } => {
            let design: Matrix = independent.iter().map(|v| vec![1.0, *v]).collect();
            let coefficients = linalg::least_squares(&design, dependent)?.0;
            let residuals = dependent
                .iter()
                .zip(&design)
                .map(|(v, row)| v - dot(row, &coefficients))
                .collect();
            let residual_test = InferenceSpec {
                available_at_ms: spec.available_at_ms,
                task: InferenceTask::Adf {
                    samples: residuals,
                    lags: *lags,
                    trend: Trend::None,
                },
            };
            out = calculate(&residual_test, checkpoint)?;
            out.coefficients = coefficients;
            out.assumptions.push("Engle-Granger first-stage OLS has an intercept; dependent/independent direction explicit; residual ADF has no intercept; cointegration critical values differ from ordinary ADF and no p-value is inferred".into());
        }
        InferenceTask::OneSampleT { samples, null_mean } => {
            t_result(
                &mut out.values,
                mean(samples) - null_mean,
                (variance(samples) / samples.len() as f64).sqrt(),
                (samples.len() - 1) as f64,
            )?;
            out.assumptions
                .push("one-sample two-sided Student t test, IID normal observations".into());
        }
        InferenceTask::Welch { first, second } => {
            let a = variance(first) / first.len() as f64;
            let b = variance(second) / second.len() as f64;
            let df = (a + b).powi(2)
                / (a * a / (first.len() - 1) as f64 + b * b / (second.len() - 1) as f64);
            t_result(
                &mut out.values,
                mean(first) - mean(second),
                (a + b).sqrt(),
                df,
            )?;
            out.assumptions
                .push("Welch-Satterthwaite unequal-variance independent two-sample t test".into());
        }
        InferenceTask::CorrelationTest { first, second } => {
            let a = mean(first);
            let b = mean(second);
            let x: Vec<_> = first.iter().map(|v| v - a).collect();
            let y: Vec<_> = second.iter().map(|v| v - b).collect();
            let rho = dot(&x, &y) / (dot(&x, &x) * dot(&y, &y)).sqrt();
            out.values.push(nv("pearson_correlation", rho));
            if rho.is_finite() && rho.abs() < 1.0 {
                t_result(
                    &mut out.values,
                    rho,
                    ((1.0 - rho * rho) / (first.len() - 2) as f64).sqrt(),
                    (first.len() - 2) as f64,
                )?;
            } else {
                out.values.push(NamedValue {
                    name: "t_statistic".into(),
                    value: Scalar::undefined("constant_or_perfectly_collinear_samples"),
                });
            }
            out.assumptions
                .push("IID bivariate normal samples, zero-correlation null".into());
        }
        InferenceTask::MaximumLikelihood { samples, family } => {
            let n = samples.len() as f64;
            match family {
                MleFamily::Normal | MleFamily::LogNormal => {
                    let values = if matches!(family, MleFamily::LogNormal) {
                        samples.iter().map(|v| v.ln()).collect()
                    } else {
                        samples.clone()
                    };
                    let m = mean(&values);
                    let variance = values.iter().map(|v| (v - m).powi(2)).sum::<f64>() / n;
                    out.values
                        .extend([nv("location_mle", m), nv("variance_mle", variance)]);
                    if variance == 0.0 {
                        out.assumptions.push("degenerate zero-variance boundary; no positive-variance density MLE exists".into());
                    }
                }
                MleFamily::Poisson => out.values.push(nv("rate_mle", mean(samples))),
                MleFamily::Exponential => out.values.push(nv("rate_mle", 1.0 / mean(samples))),
            }
            out.assumptions.push("IID likelihood; lognormal location/variance are in log space; variance uses divisor n".into());
        }
        InferenceTask::NormalMeanPosterior {
            samples,
            known_variance,
            prior_mean,
            prior_variance,
        } => {
            let v = 1.0 / (1.0 / prior_variance + samples.len() as f64 / known_variance);
            let m =
                v * (prior_mean / prior_variance + samples.iter().sum::<f64>() / known_variance);
            out.values.extend([
                nv("posterior_mean", m),
                nv("map", m),
                nv("posterior_variance", v),
            ]);
            out.assumptions.push("normal prior for unknown mean with known observation variance; IID Gaussian likelihood".into());
        }
        InferenceTask::Adf {
            samples: x,
            lags,
            trend,
        } => {
            let mut design = vec![];
            let mut target = vec![];
            for t in lags + 1..x.len() {
                let mut row = vec![x[t - 1]];
                if !matches!(trend, Trend::None) {
                    row.push(1.0);
                }
                if matches!(trend, Trend::Linear) {
                    row.push(t as f64);
                }
                for k in 1..=*lags {
                    row.push(x[t - k] - x[t - k - 1]);
                }
                design.push(row);
                target.push(x[t] - x[t - 1]);
            }
            let (b, c) = linalg::least_squares(&design, &target)?;
            let sse = target
                .iter()
                .zip(&design)
                .map(|(y, r)| (y - dot(r, &b)).powi(2))
                .sum::<f64>();
            let se = (sse / (target.len() - b.len()) as f64 * c[0][0]).sqrt();
            out.values.push(nv("adf_statistic", b[0] / se));
            out.coefficients = b;
            out.assumptions.push("ADF lag count and deterministic terms explicit; coefficient order lagged level, optional constant/trend, lagged differences; statistic has Dickey-Fuller nonstandard distribution: no ordinary Student-t p-value or fabricated critical value is returned".into());
        }
        InferenceTask::Johansen {
            observations: x,
            lagged_differences: p,
            include_constant,
            rank,
        } => {
            let d = x[0].len();
            let mut delta = vec![];
            let mut levels = vec![];
            let mut z = vec![];
            for t in p + 1..x.len() {
                delta.push(x[t].iter().zip(&x[t - 1]).map(|(a, b)| a - b).collect());
                levels.push(x[t - 1].clone());
                let mut row = vec![];
                if *include_constant {
                    row.push(1.0);
                }
                for lag in 1..=*p {
                    row.extend(x[t - lag].iter().zip(&x[t - lag - 1]).map(|(a, b)| a - b));
                }
                z.push(row);
            }
            checkpoint()?;
            let r0 = residualize(&delta, &z)?;
            let r1 = residualize(&levels, &z)?;
            let s00 = cross(&r0, &r0);
            let s11 = cross(&r1, &r1);
            let s01 = cross(&r0, &r1);
            let (ev, vectors) = linalg::symmetric_eigen(&s11)?;
            if ev.iter().any(|v| *v <= 1e-12 * ev[0]) {
                return Err(linalg::failure(
                    "singular Johansen level residual covariance",
                ));
            }
            let whitening: Matrix = (0..d)
                .map(|i| {
                    (0..d)
                        .map(|j| {
                            (0..d)
                                .map(|k| vectors[i][k] * vectors[j][k] / ev[k].sqrt())
                                .sum()
                        })
                        .collect()
                })
                .collect();
            let s10 = linalg::transpose(&s01);
            let middle = linalg::multiply(&linalg::multiply(&s10, &linalg::inverse(&s00)?), &s01);
            let canonical = linalg::multiply(&linalg::multiply(&whitening, &middle), &whitening);
            let (mut eigen, eigenvectors) = linalg::symmetric_eigen(&canonical)?;
            if eigen.iter().any(|v| *v < -1e-10 || *v >= 1.0) {
                return Err(linalg::failure(
                    "Johansen canonical eigenvalues outside [0,1)",
                ));
            }
            for v in &mut eigen {
                *v = v.max(0.0);
            }
            let beta = linalg::multiply(&whitening, &eigenvectors);
            out.cointegration_vectors = beta.iter().map(|r| r[..*rank].to_vec()).collect();
            out.adjustment = linalg::multiply(&s01, &out.cointegration_vectors);
            for r in 0..d {
                out.values.push(nv(
                    &format!("trace_rank_{r}"),
                    -(r0.len() as f64) * eigen[r..].iter().map(|v| (-v).ln_1p()).sum::<f64>(),
                ));
                out.values.push(nv(
                    &format!("max_eigen_rank_{r}"),
                    -(r0.len() as f64) * (-eigen[r]).ln_1p(),
                ));
            }
            out.eigenvalues = eigen;
            out.assumptions.push("Johansen reduced-rank regression with lagged differences and optional unrestricted constant residualized from both equations; beta normalized in S11 metric; requested rank is supplied, not inferred; nonstandard rank critical values and rank selection are not fabricated".into());
        }
    }
    checkpoint()?;
    Ok(out)
}
