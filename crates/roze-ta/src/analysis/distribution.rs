use super::*;
use rand::{distr::Open01, Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use statrs::distribution::{
    Beta, Binomial, Continuous, ContinuousCDF, Discrete, DiscreteCDF, Exp, LogNormal, Normal,
    Poisson, StudentsT,
};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(tag = "family", rename_all = "snake_case", deny_unknown_fields)]
pub enum Distribution {
    Bernoulli {
        probability: f64,
    },
    Poisson {
        rate: f64,
    },
    Exponential {
        rate: f64,
    },
    LogNormal {
        log_mean: f64,
        log_standard_deviation: f64,
    },
    Normal {
        mean: f64,
        standard_deviation: f64,
    },
    StudentT {
        location: f64,
        scale: f64,
        degrees_of_freedom: f64,
    },
    Beta {
        alpha: f64,
        beta: f64,
    },
    Binomial {
        trials: u32,
        probability: f64,
    },
    Empirical,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct Sampling {
    pub seed: u64,
    pub samples: usize,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct DistributionTask {
    pub distribution: Distribution,
    pub evaluate_at: Vec<f64>,
    pub quantiles: Vec<f64>,
    pub sampling: Option<Sampling>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct Evaluation {
    pub x: f64,
    pub pdf_or_pmf: Scalar,
    pub cdf: f64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct DistributionResult {
    pub distribution: Distribution,
    pub density_kind: String,
    pub evaluations: Vec<Evaluation>,
    pub quantiles: Vec<Scalar>,
    pub empirical_sample_count: Option<usize>,
    pub sampling: Option<Sampling>,
    pub rng_algorithm: Option<String>,
    pub completed_samples: usize,
    pub samples: Vec<f64>,
}
impl DistributionTask {
    pub(super) fn validate(&self) -> Result<(), TaError> {
        if self.evaluate_at.len() > 256
            || self.quantiles.len() > 256
            || self
                .sampling
                .as_ref()
                .is_some_and(|s| s.samples == 0 || s.samples > MAX_SAMPLES)
        {
            return Err(err(
                ErrorCode::LimitExceeded,
                "distribution limits: 256 evaluations/quantiles; sampling 1..4096",
            ));
        }
        for &x in &self.evaluate_at {
            number(x)?;
        }
        if self
            .quantiles
            .iter()
            .any(|p| !p.is_finite() || !(0.0..=1.0).contains(p))
        {
            return Err(err(
                ErrorCode::InvalidParameter,
                "quantiles require finite p in [0,1]",
            ));
        }
        let positive = |x: f64| x.is_finite() && (1e-12..=1e12).contains(&x);
        let shape = |x: f64| x.is_finite() && (0.05..=10_000.0).contains(&x);
        let location = |x: f64| x.is_finite() && x.abs() <= 1e12;
        let valid = match self.distribution {
            Distribution::Bernoulli { probability } => {
                probability.is_finite() && (0.0..=1.0).contains(&probability)
            }
            Distribution::Poisson { rate } => {
                rate.is_finite() && (1e-12..=10_000.0).contains(&rate)
            }
            Distribution::Exponential { rate } => positive(rate),
            Distribution::LogNormal {
                log_mean,
                log_standard_deviation,
            } => {
                log_mean.is_finite()
                    && log_mean.abs() <= 100.0
                    && log_standard_deviation.is_finite()
                    && (0.05..=10.0).contains(&log_standard_deviation)
            }
            Distribution::Normal {
                mean,
                standard_deviation,
            } => location(mean) && positive(standard_deviation),
            Distribution::StudentT {
                location: loc,
                scale,
                degrees_of_freedom,
            } => location(loc) && positive(scale) && shape(degrees_of_freedom),
            Distribution::Beta { alpha, beta } => shape(alpha) && shape(beta),
            Distribution::Binomial {
                trials,
                probability,
            } => {
                trials <= 1_000_000 && probability.is_finite() && (0.0..=1.0).contains(&probability)
            }
            Distribution::Empirical => true,
        };
        if valid {
            Ok(())
        } else {
            Err(err(
                ErrorCode::InvalidParameter,
                "distribution parameters outside documented finite bounds",
            ))
        }
    }
    pub(super) fn output_count(&self) -> usize {
        self.evaluate_at.len() * 3
            + self.quantiles.len()
            + self.sampling.as_ref().map_or(0, |s| s.samples)
    }
    pub(super) fn cost(&self, n: usize) -> usize {
        self.output_count() * 384 + n * 16
    }
}
enum Model {
    Poisson(Poisson),
    Exponential(Exp),
    LogNormal(LogNormal),
    Normal(Normal),
    StudentT(StudentsT),
    Beta(Beta),
    Binomial(Binomial),
    Empirical(Vec<f64>),
}
impl Model {
    fn new(distribution: &Distribution, points: &[Point]) -> Result<Self, TaError> {
        let failure = || {
            err(
                ErrorCode::InvalidParameter,
                "invalid distribution parameters",
            )
        };
        Ok(match *distribution {
            Distribution::Bernoulli { probability } => {
                Self::Binomial(Binomial::new(probability, 1).map_err(|_| failure())?)
            }
            Distribution::Poisson { rate } => {
                Self::Poisson(Poisson::new(rate).map_err(|_| failure())?)
            }
            Distribution::Exponential { rate } => {
                Self::Exponential(Exp::new(rate).map_err(|_| failure())?)
            }
            Distribution::LogNormal {
                log_mean,
                log_standard_deviation,
            } => Self::LogNormal(
                LogNormal::new(log_mean, log_standard_deviation).map_err(|_| failure())?,
            ),
            Distribution::Normal {
                mean,
                standard_deviation,
            } => Self::Normal(Normal::new(mean, standard_deviation).map_err(|_| failure())?),
            Distribution::StudentT {
                location,
                scale,
                degrees_of_freedom,
            } => Self::StudentT(
                StudentsT::new(location, scale, degrees_of_freedom).map_err(|_| failure())?,
            ),
            Distribution::Beta { alpha, beta } => {
                Self::Beta(Beta::new(alpha, beta).map_err(|_| failure())?)
            }
            Distribution::Binomial {
                trials,
                probability,
            } => Self::Binomial(Binomial::new(probability, trials as u64).map_err(|_| failure())?),
            Distribution::Empirical => {
                if points.is_empty() {
                    return Err(err(
                        ErrorCode::InsufficientData,
                        "empirical distribution requires selected samples",
                    ));
                }
                Self::Empirical(statistics::sorted(points.iter().map(|p| p.x)))
            }
        })
    }
    fn evaluate(&self, x: f64) -> (f64, f64) {
        match self {
            Self::Exponential(d) => (d.pdf(x), d.cdf(x)),
            Self::LogNormal(d) => (d.pdf(x), d.cdf(x)),
            Self::Poisson(d) => {
                let pmf = if (0.0..=1_000_000.0).contains(&x) && x.fract() == 0.0 {
                    d.pmf(x as u64)
                } else {
                    0.0
                };
                let cdf = if x < 0.0 {
                    0.0
                } else if x > 1_000_000.0 {
                    1.0
                } else {
                    d.cdf(x.floor() as u64)
                };
                (pmf, cdf)
            }
            Self::Normal(d) => (d.pdf(x), d.cdf(x)),
            Self::StudentT(d) => (d.pdf(x), d.cdf(x)),
            Self::Beta(d) => (d.pdf(x), d.cdf(x)),
            Self::Binomial(d) => {
                let pmf = if x >= 0.0 && x.fract() == 0.0 && x <= 1_000_000.0 {
                    d.pmf(x as u64)
                } else {
                    0.0
                };
                let cdf = if x < 0.0 {
                    0.0
                } else if x > 1_000_000.0 {
                    1.0
                } else {
                    d.cdf(x.floor() as u64)
                };
                (pmf, cdf)
            }
            Self::Empirical(xs) => {
                let lower = xs.partition_point(|v| *v < x);
                let upper = xs.partition_point(|v| *v <= x);
                (
                    (upper - lower) as f64 / xs.len() as f64,
                    upper as f64 / xs.len() as f64,
                )
            }
        }
    }
    fn inverse(
        &self,
        p: f64,
        checkpoint: &mut impl FnMut() -> Result<(), TaError>,
    ) -> Result<f64, TaError> {
        Ok(match self {
            Self::Exponential(d) => d.inverse_cdf(p),
            Self::LogNormal(d) => d.inverse_cdf(p),
            Self::Poisson(d) => {
                if p == 1.0 {
                    f64::INFINITY
                } else {
                    let (mut lo, mut hi) = (0u64, 1_000_000u64);
                    while lo < hi {
                        checkpoint()?;
                        let mid = lo + (hi - lo) / 2;
                        if d.cdf(mid) >= p {
                            hi = mid;
                        } else {
                            lo = mid + 1;
                        }
                    }
                    lo as f64
                }
            }
            Self::Normal(d) => d.inverse_cdf(p),
            Self::StudentT(d) => student_quantile(d, p, checkpoint)?,
            Self::Beta(d) => bounded_quantile(|x| d.cdf(x), p, 0.0, 1.0, checkpoint)?,
            Self::Binomial(d) => d.inverse_cdf(p) as f64,
            Self::Empirical(xs) => {
                xs[((p * xs.len() as f64).ceil() as usize)
                    .saturating_sub(1)
                    .min(xs.len() - 1)]
            }
        })
    }
}
pub(super) fn calculate(
    task: &DistributionTask,
    points: &[Point],
    checkpoint: &mut impl FnMut() -> Result<(), TaError>,
) -> Result<DistributionResult, TaError> {
    let model = Model::new(&task.distribution, points)?;
    let mut evaluations = Vec::with_capacity(task.evaluate_at.len());
    for &x in &task.evaluate_at {
        checkpoint()?;
        let (pdf, cdf) = model.evaluate(x);
        if !cdf.is_finite() || !(0.0..=1.0).contains(&cdf) {
            return Err(err(ErrorCode::NumericalFailure, "CDF outside finite [0,1]"));
        }
        evaluations.push(Evaluation {
            x,
            pdf_or_pmf: Scalar::number(pdf),
            cdf,
        });
    }
    let mut quantiles = Vec::with_capacity(task.quantiles.len());
    for &p in &task.quantiles {
        checkpoint()?;
        quantiles.push(Scalar::number(model.inverse(p, checkpoint)?));
    }
    let mut samples = Vec::new();
    if let Some(s) = &task.sampling {
        let mut rng = ChaCha8Rng::seed_from_u64(s.seed);
        samples.reserve(s.samples);
        for _ in 0..s.samples {
            checkpoint()?;
            let p: f64 = rng.sample(Open01);
            let x = model.inverse(p, checkpoint)?;
            if !x.is_finite() {
                return Err(err(
                    ErrorCode::NumericalFailure,
                    "non-finite inverse-CDF sample",
                ));
            }
            samples.push(x);
        }
    }
    Ok(DistributionResult {
        distribution: task.distribution.clone(),
        density_kind: if matches!(
            task.distribution,
            Distribution::Bernoulli { .. }
                | Distribution::Poisson { .. }
                | Distribution::Binomial { .. }
                | Distribution::Empirical
        ) {
            "probability_mass"
        } else {
            "probability_density"
        }
        .into(),
        evaluations,
        quantiles,
        empirical_sample_count: if matches!(task.distribution, Distribution::Empirical) {
            Some(points.len())
        } else {
            None
        },
        sampling: task.sampling.clone(),
        rng_algorithm: task
            .sampling
            .as_ref()
            .map(|_| "ChaCha8/rand_chacha=0.9.0/rand=0.9.5/Open01/inverse_cdf".into()),
        completed_samples: samples.len(),
        samples,
    })
}

/// Bisection has a fixed iteration cap; failure is reported rather than silently
/// returning an unconverged tail value. The CDF evaluator is supplied by statrs.
pub(super) fn bounded_quantile(
    mut cdf: impl FnMut(f64) -> f64,
    p: f64,
    mut lo: f64,
    mut hi: f64,
    checkpoint: &mut impl FnMut() -> Result<(), TaError>,
) -> Result<f64, TaError> {
    if p == 0.0 {
        return Ok(lo);
    }
    if p == 1.0 {
        return Ok(hi);
    }
    for _ in 0..256 {
        checkpoint()?;
        let mid = lo + (hi - lo) * 0.5;
        let y = cdf(mid);
        if !y.is_finite() {
            return Err(err(
                ErrorCode::NumericalFailure,
                "non-finite CDF during bounded inversion",
            ));
        }
        if (y - p).abs() <= 1e-12 * p.min(1.0 - p) {
            return Ok(mid);
        }
        if mid == lo || mid == hi {
            break;
        }
        if y < p {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    Err(err(
        ErrorCode::NumericalFailure,
        "quantile precision unavailable within 256 bisections",
    ))
}
pub(super) fn student_quantile(
    d: &StudentsT,
    p: f64,
    checkpoint: &mut impl FnMut() -> Result<(), TaError>,
) -> Result<f64, TaError> {
    if p == 0.0 {
        return Ok(f64::NEG_INFINITY);
    }
    if p == 1.0 {
        return Ok(f64::INFINITY);
    }
    let mut radius = d.scale().max(1.0);
    for _ in 0..128 {
        checkpoint()?;
        let lo = d.location() - radius;
        let hi = d.location() + radius;
        if d.cdf(lo) <= p && d.cdf(hi) >= p {
            return bounded_quantile(|x| d.cdf(x), p, lo, hi, checkpoint);
        }
        radius *= 2.0;
    }
    Err(err(
        ErrorCode::NumericalFailure,
        "Student-t quantile outside bounded search range",
    ))
}
