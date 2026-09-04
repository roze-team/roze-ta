//! Seeded path simulation and characteristic-function option pricing.
use super::*;
use rand::{distr::Open01, Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use research::NamedValue;
use statrs::distribution::{ContinuousCDF, Normal};
pub mod calibration;
pub const VERSION: &str = "roze-ta-stochastic-v1.1/chacha8-rand_chacha-0.9.0";
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Process {
    Brownian {
        initial: f64,
        drift: f64,
        diffusion: f64,
    },
    GeometricBrownian {
        initial: f64,
        drift: f64,
        volatility: f64,
    },
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct HestonParameters {
    pub spot: f64,
    pub strike: f64,
    pub years: f64,
    pub rate: f64,
    pub dividend_yield: f64,
    pub initial_variance: f64,
    pub reversion: f64,
    pub long_run_variance: f64,
    pub vol_of_variance: f64,
    pub correlation: f64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum StochasticTask {
    HestonCalibrate {
        quotes: Vec<calibration::HestonQuote>,
        initial: calibration::HestonModel,
        parameters: Vec<calibration::HestonParameter>,
        lower: Vec<f64>,
        upper: Vec<f64>,
        max_iterations: usize,
        fit_tolerance: f64,
        integration_limit: f64,
        intervals: usize,
        pricing_tolerance: f64,
    },
    Paths {
        process: Process,
        horizon: f64,
        steps: usize,
        paths: usize,
        seed: u64,
    },
    Ito {
        time_derivative: f64,
        space_derivative: f64,
        second_space_derivative: f64,
        drift: f64,
        diffusion: f64,
    },
    Heston {
        parameters: HestonParameters,
        integration_limit: f64,
        intervals: usize,
        tolerance: f64,
    },
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct StochasticSpec {
    pub available_at_ms: i64,
    pub task: StochasticTask,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct StochasticResult {
    pub method_version: String,
    pub values: Vec<NamedValue>,
    pub paths: Vec<Vec<f64>>,
    pub converged: bool,
    pub assumptions: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub calibration: Option<calibration::HestonCalibration>,
}
fn bad(s: &str) -> TaError {
    err(ErrorCode::InvalidParameter, s)
}
fn finite(v: f64) -> bool {
    v.is_finite() && v.abs() <= 1e6
}
fn nv(n: &str, v: f64) -> NamedValue {
    NamedValue {
        name: n.into(),
        value: Scalar::number(v),
    }
}
impl StochasticSpec {
    pub(super) fn validate(&self, r: &Request) -> Result<(usize, usize), TaError> {
        if r.input_kind != "stochastic_parameters" || !r.points.is_empty() || !r.events.is_empty() {
            return Err(bad(
                "stochastic tasks require stochastic_parameters and empty points/events",
            ));
        }
        if self.available_at_ms <= 0 || self.available_at_ms > r.fit_cutoff_ms {
            return Err(err(
                ErrorCode::InvalidTime,
                "stochastic parameters must be available by fit cutoff",
            ));
        }
        match &self.task {
            StochasticTask::HestonCalibrate { .. } => calibration::validate(self, r),
            StochasticTask::Ito {
                time_derivative,
                space_derivative,
                second_space_derivative,
                drift,
                diffusion,
            } => {
                if [
                    *time_derivative,
                    *space_derivative,
                    *second_space_derivative,
                    *drift,
                    *diffusion,
                ]
                .iter()
                .any(|&v| !finite(v))
                {
                    return Err(bad("finite Ito inputs required"));
                }
                Ok((32, 8))
            }
            StochasticTask::Paths {
                process,
                horizon,
                steps,
                paths,
                ..
            } => {
                if !finite(*horizon)
                    || *horizon <= 0.0
                    || !(1..=4096).contains(steps)
                    || !(1..=4096).contains(paths)
                {
                    return Err(bad("positive horizon and 1..4096 paths/steps required"));
                }
                let (initial, drift, vol) = match *process {
                    Process::Brownian {
                        initial,
                        drift,
                        diffusion,
                    } => (initial, drift, diffusion),
                    Process::GeometricBrownian {
                        initial,
                        drift,
                        volatility,
                    } => {
                        if initial <= 0.0 {
                            return Err(bad("GBM initial value must be positive"));
                        }
                        (initial, drift, volatility)
                    }
                };
                if !finite(initial) || !finite(drift) || !finite(vol) || vol < 0.0 {
                    return Err(bad(
                        "finite process parameters and nonnegative diffusion required",
                    ));
                }
                Ok((paths * steps * 16, paths * (steps + 1) + 16))
            }
            StochasticTask::Heston {
                parameters: p,
                integration_limit,
                intervals,
                tolerance,
            } => {
                if [
                    p.spot,
                    p.strike,
                    p.years,
                    p.rate,
                    p.dividend_yield,
                    p.initial_variance,
                    p.reversion,
                    p.long_run_variance,
                    p.vol_of_variance,
                    p.correlation,
                ]
                .iter()
                .any(|&v| !finite(v))
                    || p.spot <= 0.0
                    || p.strike <= 0.0
                    || p.years <= 0.0
                    || p.initial_variance < 0.0
                    || p.reversion <= 0.0
                    || p.long_run_variance < 0.0
                    || p.vol_of_variance < 0.0
                    || p.correlation.abs() > 1.0
                {
                    return Err(bad("invalid Heston parameters"));
                }
                if !integration_limit.is_finite()
                    || !(10.0..=1000.0).contains(integration_limit)
                    || !(64..=4096).contains(intervals)
                    || intervals % 2 != 0
                    || !tolerance.is_finite()
                    || !(1e-10..=0.1).contains(tolerance)
                {
                    return Err(bad("Fourier limit 10..1000, even intervals 64..4096 and tolerance 1e-10..0.1 required"));
                }
                Ok((intervals * 256, 32))
            }
        }
    }
}
// Minimal private complex arithmetic for the affine characteristic function.
#[derive(Clone, Copy)]
struct C {
    re: f64,
    im: f64,
}
impl C {
    fn new(re: f64, im: f64) -> Self {
        Self { re, im }
    }
    fn exp(self) -> Self {
        let r = self.re.exp();
        Self::new(r * self.im.cos(), r * self.im.sin())
    }
    fn ln(self) -> Self {
        Self::new(self.re.hypot(self.im).ln(), self.im.atan2(self.re))
    }
    fn sqrt(self) -> Self {
        let radius = self.re.hypot(self.im);
        if radius == 0.0 {
            return real(0.0);
        }
        if self.re >= 0.0 {
            let re = ((radius + self.re) / 2.0).sqrt();
            Self::new(re, self.im / (2.0 * re))
        } else {
            let im = ((radius - self.re) / 2.0).sqrt().copysign(self.im);
            Self::new(self.im / (2.0 * im), im)
        }
    }
}
impl std::ops::Add for C {
    type Output = Self;
    fn add(self, b: Self) -> Self {
        Self::new(self.re + b.re, self.im + b.im)
    }
}
impl std::ops::Sub for C {
    type Output = Self;
    fn sub(self, b: Self) -> Self {
        Self::new(self.re - b.re, self.im - b.im)
    }
}
impl std::ops::Mul for C {
    type Output = Self;
    fn mul(self, b: Self) -> Self {
        Self::new(
            self.re * b.re - self.im * b.im,
            self.re * b.im + self.im * b.re,
        )
    }
}
impl std::ops::Div for C {
    type Output = Self;
    fn div(self, b: Self) -> Self {
        let denominator = b.re * b.re + b.im * b.im;
        Self::new(
            (self.re * b.re + self.im * b.im) / denominator,
            (self.im * b.re - self.re * b.im) / denominator,
        )
    }
}
fn real(v: f64) -> C {
    C::new(v, 0.0)
}
fn characteristic(p: &HestonParameters, u: C) -> C {
    let iu = C::new(0.0, 1.0) * u;
    let xi2 = p.vol_of_variance * p.vol_of_variance;
    let b = real(p.reversion) - iu * real(p.correlation * p.vol_of_variance);
    let d = (b * b + (u * u + iu) * real(xi2)).sqrt();
    let g = (b - d) / (b + d);
    let e = (real(-p.years) * d).exp();
    let one = real(1.0);
    let log = ((one - g * e) / (one - g)).ln();
    let c = iu * real(p.spot.ln() + (p.rate - p.dividend_yield) * p.years)
        + real(p.reversion * p.long_run_variance / xi2)
            * ((b - d) * real(p.years) - real(2.0) * log);
    let dd = (b - d) / real(xi2) * (one - e) / (one - g * e);
    (c + dd * real(p.initial_variance)).exp()
}
fn fourier(
    p: &HestonParameters,
    limit: f64,
    n: usize,
    checkpoint: &mut impl FnMut() -> Result<(), TaError>,
) -> Result<f64, TaError> {
    let eps = 1e-7;
    let step = (limit - eps) / n as f64;
    let mut sums = [0.0, 0.0];
    let forward = p.spot * ((p.rate - p.dividend_yield) * p.years).exp();
    for i in 0..=n {
        if i % 32 == 0 {
            checkpoint()?;
        }
        let u = eps + i as f64 * step;
        let oscillation = C::new(0.0, -u * p.strike.ln()).exp();
        let denominator = C::new(0.0, u);
        let terms = [
            (oscillation * characteristic(p, C::new(u, -1.0)) / denominator / real(forward)).re,
            (oscillation * characteristic(p, real(u)) / denominator).re,
        ];
        let weight = if i == 0 || i == n {
            1.0
        } else if i % 2 == 0 {
            2.0
        } else {
            4.0
        };
        for j in 0..2 {
            sums[j] += weight * terms[j];
        }
    }
    let probabilities = sums.map(|v| 0.5 + v * step / (3.0 * std::f64::consts::PI));
    let price = p.spot * (-p.dividend_yield * p.years).exp() * probabilities[0]
        - p.strike * (-p.rate * p.years).exp() * probabilities[1];
    if !price.is_finite() {
        return Err(err(
            ErrorCode::NumericalFailure,
            "Heston characteristic integration failed",
        ));
    }
    Ok(price)
}
pub(super) fn calculate(
    spec: &StochasticSpec,
    checkpoint: &mut impl FnMut() -> Result<(), TaError>,
) -> Result<StochasticResult, TaError> {
    let mut out=StochasticResult{method_version:VERSION.into(),values:vec![],paths:vec![],converged:true,calibration:None,assumptions:vec!["explicit parameters at fit cutoff; model-conditional calculations, no implicit calibration".into()]};
    match &spec.task {
        StochasticTask::HestonCalibrate { .. } => {
            let fit = calibration::fit(spec, checkpoint)?;
            out.converged = fit.converged;
            out.calibration = Some(fit);
            out.assumptions.push("Explicit bounded calibration to weighted European option prices; active parameter list and bounds supplied by caller; fixed interest/dividend/spot per quote. Convergence is local coordinate-search resolution, not global optimality or identifiability. Pricing integration convergence is required for accepted parameters. Work budget limits quote count, quadrature resolution and optimizer iterations jointly.".into());
        }
        StochasticTask::Ito {
            time_derivative: ft,
            space_derivative: fx,
            second_space_derivative: fxx,
            drift: mu,
            diffusion: sigma,
        } => {
            out.values = vec![
                nv(
                    "transformed_drift",
                    ft + fx * mu + 0.5 * fxx * sigma * sigma,
                ),
                nv("transformed_diffusion", fx * sigma),
            ];
        }
        StochasticTask::Paths {
            process,
            horizon,
            steps,
            paths,
            seed,
        } => {
            let mut rng = ChaCha8Rng::seed_from_u64(*seed);
            let dt = horizon / *steps as f64;
            let mut terminals = vec![];
            for _ in 0..*paths {
                checkpoint()?;
                let mut x = match process {
                    Process::Brownian { initial, .. }
                    | Process::GeometricBrownian { initial, .. } => *initial,
                };
                let mut path = vec![x];
                for _ in 0..*steps {
                    let u: f64 = rng.sample(Open01);
                    let v: f64 = rng.sample(Open01);
                    let z = (-2.0 * u.ln()).sqrt() * (std::f64::consts::TAU * v).cos();
                    x = match process {
                        Process::Brownian {
                            drift, diffusion, ..
                        } => x + drift * dt + diffusion * dt.sqrt() * z,
                        Process::GeometricBrownian {
                            drift, volatility, ..
                        } => {
                            x * ((drift - 0.5 * volatility * volatility) * dt
                                + volatility * dt.sqrt() * z)
                                .exp()
                        }
                    };
                    if !x.is_finite() {
                        return Err(err(ErrorCode::NumericalFailure, "simulated path overflow"));
                    }
                    path.push(x);
                }
                terminals.push(x);
                out.paths.push(path);
            }
            let mean = terminals.iter().sum::<f64>() / *paths as f64;
            out.values.push(nv("terminal_mean", mean));
            out.values.push(if *paths > 1 {
                nv(
                    "terminal_mean_standard_error",
                    (terminals.iter().map(|v| (v - mean).powi(2)).sum::<f64>()
                        / ((*paths - 1) * paths) as f64)
                        .sqrt(),
                )
            } else {
                NamedValue {
                    name: "terminal_mean_standard_error".into(),
                    value: Scalar::InsufficientData,
                }
            });
            out.assumptions.push("ChaCha8 seeded Box-Muller normal draws; Brownian and GBM transitions exact on supplied grid; path discretization does not capture between-grid barrier crossings".into());
        }
        StochasticTask::Heston {
            parameters: p,
            integration_limit,
            intervals,
            tolerance,
        } => {
            let (call, error) = if p.vol_of_variance == 0.0 {
                let integrated = p.long_run_variance * p.years
                    + (p.initial_variance - p.long_run_variance)
                        * (-(-p.reversion * p.years).exp_m1())
                        / p.reversion;
                let s = p.spot * (-p.dividend_yield * p.years).exp();
                let k = p.strike * (-p.rate * p.years).exp();
                if integrated <= 0.0 {
                    ((s - k).max(0.0), 0.0)
                } else {
                    let n = Normal::new(0.0, 1.0).map_err(|_| bad("normal construction failed"))?;
                    let d1 = ((s / k).ln() + integrated / 2.0) / integrated.sqrt();
                    (s * n.cdf(d1) - k * n.cdf(d1 - integrated.sqrt()), 0.0)
                }
            } else {
                let fine = fourier(p, *integration_limit, *intervals, checkpoint)?;
                let coarse = fourier(p, *integration_limit, *intervals / 2, checkpoint)?;
                let extended = fourier(p, integration_limit * 2.0, *intervals * 2, checkpoint)?;
                (extended, (fine - coarse).abs().max((extended - fine).abs()))
            };
            let discounted_spot = p.spot * (-p.dividend_yield * p.years).exp();
            let discounted_strike = p.strike * (-p.rate * p.years).exp();
            out.converged = error <= *tolerance
                && call >= (discounted_spot - discounted_strike).max(0.0) - tolerance
                && call <= discounted_spot + tolerance;
            out.values = vec![
                nv("call", call),
                nv("put", call - discounted_spot + discounted_strike),
                nv("integration_difference", error),
                nv(
                    "feller_margin",
                    2.0 * p.reversion * p.long_run_variance - p.vol_of_variance * p.vol_of_variance,
                ),
            ];
            out.assumptions.push("European Heston affine characteristic function with decaying-exponential branch; composite Simpson at two resolutions and two cutoffs; convergence is an empirical integration check, not a certified error bound; zero vol-of-variance uses exact deterministic-variance limit".into());
        }
    }
    checkpoint()?;
    Ok(out)
}
