//! Conditional time-series recursions with explicit initial state and parameters.
use super::*;
use linalg::{dot, matvec, Matrix};
use research::NamedValue;
pub const VERSION: &str = "roze-ta-dynamics-v1";
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct Arrival {
    pub time: f64,
    pub channel: usize,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum DynamicsTask {
    Ar1Moments {
        intercept: f64,
        phi: f64,
        innovation_variance: f64,
    },
    Ewma {
        returns: Vec<f64>,
        initial_variance: f64,
        decay: f64,
    },
    Garch {
        residuals: Vec<f64>,
        omega: f64,
        alpha: Vec<f64>,
        beta: Vec<f64>,
        initial_squared_residuals: Vec<f64>,
        initial_variances: Vec<f64>,
    },
    Dcc {
        standardized_residuals: Matrix,
        a: f64,
        b: f64,
        long_run_q: Matrix,
        initial_q: Matrix,
    },
    HarRv {
        realized_variances: Vec<f64>,
        intercept: f64,
        daily: f64,
        weekly: f64,
        monthly: f64,
    },
    RealizedVariance {
        log_returns: Vec<f64>,
    },
    Parkinson {
        high: Vec<f64>,
        low: Vec<f64>,
        periods_per_year: f64,
    },
    OrnsteinUhlenbeck {
        current: f64,
        long_run_mean: f64,
        reversion: f64,
        diffusion: f64,
        elapsed: f64,
    },
    Arima {
        observations: Vec<f64>,
        ar: Vec<f64>,
        ma: Vec<f64>,
        intercept: f64,
        differences: usize,
    },
    Kalman {
        observations: Matrix,
        transition: Matrix,
        observation: Matrix,
        process_covariance: Matrix,
        observation_covariance: Matrix,
        initial_state: Vec<f64>,
        initial_covariance: Matrix,
    },
    Hawkes {
        baseline: Vec<f64>,
        alpha: Matrix,
        beta: Matrix,
        arrivals: Vec<Arrival>,
        horizon: f64,
    },
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct DynamicsSpec {
    pub available_at_ms: i64,
    pub task: DynamicsTask,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct DynamicsResult {
    pub method_version: String,
    pub values: Vec<NamedValue>,
    pub series: Matrix,
    pub final_matrix: Matrix,
    pub assumptions: Vec<String>,
}
fn bad(s: &str) -> TaError {
    err(ErrorCode::InvalidParameter, s)
}
fn bounded(x: f64) -> bool {
    x.is_finite() && x.abs() <= 1e12
}
fn positive(x: f64) -> bool {
    bounded(x) && x > 0.0
}
fn vector(v: &[f64]) -> Result<(), TaError> {
    if v.len() > MAX_SAMPLES || v.iter().any(|&v| !bounded(v)) {
        Err(bad(
            "dynamics vectors require finite magnitude <=1e12 and length <=4096",
        ))
    } else {
        Ok(())
    }
}
fn nonnegative(v: &[f64]) -> Result<(), TaError> {
    vector(v)?;
    if v.iter().any(|v| *v < 0.0) {
        Err(bad("negative variance or intensity"))
    } else {
        Ok(())
    }
}
fn shape(m: &Matrix, r: usize, c: usize) -> Result<(), TaError> {
    if m.len() != r || m.iter().any(|v| v.len() != c) {
        return Err(bad("matrix shape mismatch"));
    }
    for v in m {
        vector(v)?;
    }
    Ok(())
}
fn symmetric(m: &Matrix, n: usize) -> Result<(), TaError> {
    shape(m, n, n)?;
    for (i, row) in m.iter().enumerate() {
        for (j, _) in row.iter().enumerate() {
            if (m[i][j] - m[j][i]).abs() > 1e-12 * m[i][j].abs().max(m[j][i].abs()).max(1e-30) {
                return Err(bad("covariance must be symmetric"));
            }
        }
    }
    Ok(())
}
fn psd(m: &Matrix, strict: bool) -> Result<(), TaError> {
    let (e, _) = linalg::symmetric_eigen(m)?;
    let scale = e[0].abs();
    if e.iter().any(|&v| {
        if strict {
            v <= 1e-12 * scale
        } else {
            v < -1e-12 * scale
        }
    }) {
        return Err(linalg::failure("covariance positivity check failed"));
    }
    Ok(())
}
fn nv(n: &str, v: f64) -> NamedValue {
    NamedValue {
        name: n.into(),
        value: Scalar::number(v),
    }
}
impl DynamicsSpec {
    pub(super) fn validate(&self, r: &Request) -> Result<(usize, usize), TaError> {
        if r.input_kind != "dynamics_parameters" || !r.points.is_empty() || !r.events.is_empty() {
            return Err(bad(
                "dynamics requires dynamics_parameters and empty points/events",
            ));
        }
        if self.available_at_ms <= 0 || self.available_at_ms > r.fit_cutoff_ms {
            return Err(err(
                ErrorCode::InvalidTime,
                "dynamics input/state must be available by fit cutoff",
            ));
        }
        let (mut work, mut output) = (64, 64);
        match &self.task {
            DynamicsTask::Ar1Moments {
                intercept,
                phi,
                innovation_variance,
            } => {
                vector(&[*intercept, *phi, *innovation_variance])?;
                if phi.abs() >= 1.0 || *innovation_variance < 0.0 {
                    return Err(bad("AR1 stationary moments require abs(phi)<1 and nonnegative innovation variance"));
                }
            }
            DynamicsTask::Ewma {
                returns,
                initial_variance,
                decay,
            } => {
                vector(returns)?;
                if !bounded(*initial_variance)
                    || *initial_variance < 0.0
                    || !decay.is_finite()
                    || !(0.0..1.0).contains(decay)
                {
                    return Err(bad("EWMA nonnegative variance and decay [0,1) required"));
                }
                work += returns.len();
                output += returns.len();
            }
            DynamicsTask::Garch {
                residuals,
                omega,
                alpha,
                beta,
                initial_squared_residuals,
                initial_variances,
            } => {
                vector(residuals)?;
                nonnegative(alpha)?;
                nonnegative(beta)?;
                nonnegative(initial_squared_residuals)?;
                nonnegative(initial_variances)?;
                if !positive(*omega)
                    || alpha.is_empty()
                    || alpha.len() > 32
                    || beta.len() > 32
                    || initial_squared_residuals.len() != alpha.len()
                    || initial_variances.len() != beta.len()
                    || alpha.iter().chain(beta).sum::<f64>() >= 1.0
                {
                    return Err(bad("stationary ARCH/GARCH requires omega>0, 1..32 ARCH lags, 0..32 GARCH lags, matching initial newest-first histories and sum(alpha,beta)<1"));
                }
                work += residuals.len() * (alpha.len() + beta.len() + 4);
                output += residuals.len();
            }
            DynamicsTask::Dcc {
                standardized_residuals: z,
                a,
                b,
                long_run_q,
                initial_q,
            } => {
                let d = long_run_q.len();
                if !(1..=8).contains(&d)
                    || z.len() > MAX_SAMPLES
                    || !bounded(*a)
                    || !bounded(*b)
                    || *a < 0.0
                    || *b < 0.0
                    || a + b >= 1.0
                {
                    return Err(bad(
                        "DCC requires dimension 1..8, nonnegative a,b and a+b<1",
                    ));
                }
                symmetric(long_run_q, d)?;
                symmetric(initial_q, d)?;
                shape(z, z.len(), d)?;
                work += 100 * d.pow(4) + z.len() * d * d * 4;
                output += z.len() * d * d;
            }
            DynamicsTask::HarRv {
                realized_variances,
                intercept,
                daily,
                weekly,
                monthly,
            } => {
                nonnegative(realized_variances)?;
                vector(&[*intercept, *daily, *weekly, *monthly])?;
                if realized_variances.len() < 22 {
                    return Err(err(
                        ErrorCode::InsufficientData,
                        "HAR requires at least 22 daily realized variances",
                    ));
                }
                work += realized_variances.len();
            }
            DynamicsTask::RealizedVariance { log_returns } => {
                vector(log_returns)?;
                work += log_returns.len();
            }
            DynamicsTask::Parkinson {
                high,
                low,
                periods_per_year,
            } => {
                vector(high)?;
                vector(low)?;
                if high.len() != low.len()
                    || !positive(*periods_per_year)
                    || high.iter().zip(low).any(|(h, l)| *l <= 0.0 || h < l)
                {
                    return Err(bad("Parkinson needs aligned positive high>=low and positive annualization factor"));
                }
                work += high.len();
            }
            DynamicsTask::OrnsteinUhlenbeck {
                current,
                long_run_mean,
                reversion,
                diffusion,
                elapsed,
            } => {
                vector(&[*current, *long_run_mean, *diffusion, *elapsed])?;
                if !positive(*reversion) || *diffusion < 0.0 || *elapsed < 0.0 {
                    return Err(bad(
                        "OU requires positive reversion, nonnegative diffusion/time",
                    ));
                }
            }
            DynamicsTask::Arima {
                observations,
                ar,
                ma,
                intercept,
                differences,
            } => {
                vector(observations)?;
                vector(ar)?;
                vector(ma)?;
                if !bounded(*intercept)
                    || ar.len() > 32
                    || ma.len() > 32
                    || *differences > 2
                    || observations.len() <= ar.len() + differences
                {
                    return Err(bad(
                        "ARIMA needs sufficient data, p/q<=32,d<=2 and finite intercept",
                    ));
                }
                work += observations.len() * (ar.len() + ma.len() + 4);
                output += observations.len();
            }
            DynamicsTask::Kalman {
                observations: z,
                transition: f,
                observation: h,
                process_covariance: q,
                observation_covariance: v,
                initial_state: x,
                initial_covariance: p,
            } => {
                let d = x.len();
                let k = h.len();
                if !(1..=8).contains(&d) || !(1..=8).contains(&k) || z.len() > MAX_SAMPLES {
                    return Err(bad(
                        "Kalman state/observation dimensions 1..8, observations <=4096",
                    ));
                }
                vector(x)?;
                shape(f, d, d)?;
                shape(h, k, d)?;
                shape(z, z.len(), k)?;
                symmetric(q, d)?;
                symmetric(v, k)?;
                symmetric(p, d)?;
                work += z.len() * d.max(k).pow(3) * 32 + 100 * d.max(k).pow(4);
                output += z.len() * d;
            }
            DynamicsTask::Hawkes {
                baseline,
                alpha,
                beta,
                arrivals,
                horizon,
            } => {
                let d = baseline.len();
                if !(1..=8).contains(&d)
                    || arrivals.len() > MAX_SAMPLES
                    || !positive(*horizon)
                    || baseline.iter().any(|&v| !positive(v))
                {
                    return Err(bad("Hawkes requires 1..8 positive baseline intensities, positive horizon and <=4096 arrivals"));
                }
                shape(alpha, d, d)?;
                shape(beta, d, d)?;
                if alpha.iter().flatten().any(|v| *v < 0.0)
                    || beta.iter().flatten().any(|v| *v <= 0.0)
                {
                    return Err(bad("Hawkes needs alpha>=0 and beta>0"));
                }
                let mut previous = 0.0;
                for e in arrivals {
                    if !e.time.is_finite()
                        || e.time <= previous
                        || e.time > *horizon
                        || e.channel >= d
                    {
                        return Err(bad("Hawkes arrivals require strictly increasing times in (0,horizon] and valid channels"));
                    }
                    previous = e.time;
                }
                work += arrivals.len() * d * d * 4 + 1000 * d * d;
                output += arrivals.len();
            }
        }
        Ok((work, output))
    }
}
fn add(a: &Matrix, b: &Matrix) -> Matrix {
    a.iter()
        .zip(b)
        .map(|(a, b)| a.iter().zip(b).map(|(a, b)| a + b).collect())
        .collect()
}
pub(super) fn calculate(
    spec: &DynamicsSpec,
    checkpoint: &mut impl FnMut() -> Result<(), TaError>,
) -> Result<DynamicsResult, TaError> {
    let mut out=DynamicsResult{method_version:VERSION.into(),values:vec![],series:vec![],final_matrix:vec![],assumptions:vec!["conditional evaluation of supplied parameters and initial state; no implicit parameter estimation; all observations are known by fit cutoff".into()]};
    match &spec.task {
        DynamicsTask::Ar1Moments {
            intercept,
            phi,
            innovation_variance,
        } => {
            out.values = vec![
                nv("stationary_mean", intercept / (1.0 - phi)),
                nv(
                    "stationary_variance",
                    innovation_variance / (1.0 - phi * phi),
                ),
            ];
            out.values.push(if *phi > 0.0 {
                nv("half_life_periods", -2f64.ln() / phi.ln())
            } else {
                NamedValue {
                    name: "half_life_periods".into(),
                    value: Scalar::undefined("nonpositive_ar_coefficient"),
                }
            });
        }
        DynamicsTask::Ewma {
            returns,
            initial_variance,
            decay,
        } => {
            let mut v = *initial_variance;
            for r in returns {
                checkpoint()?;
                out.series.push(vec![v]);
                v = decay * v + (1.0 - decay) * r * r;
            }
            out.values.push(nv("next_variance", v));
            out.assumptions.push(
                "zero-mean innovations; each row is variance before that row's return".into(),
            );
        }
        DynamicsTask::Garch {
            residuals,
            omega,
            alpha,
            beta,
            initial_squared_residuals,
            initial_variances,
        } => {
            let mut shocks = initial_squared_residuals.clone();
            let mut variances = initial_variances.clone();
            let mut next = omega + dot(alpha, &shocks) + dot(beta, &variances);
            for e in residuals {
                checkpoint()?;
                out.series.push(vec![next]);
                shocks.rotate_right(1);
                shocks[0] = e * e;
                if !variances.is_empty() {
                    variances.rotate_right(1);
                    variances[0] = next;
                }
                next = omega + dot(alpha, &shocks) + dot(beta, &variances);
            }
            out.values.extend([
                nv("next_variance", next),
                nv(
                    "unconditional_variance",
                    omega / (1.0 - alpha.iter().chain(beta).sum::<f64>()),
                ),
            ]);
            out.assumptions.push("ARCH(q) when beta is empty; histories newest-first and precede the first supplied residual".into());
        }
        DynamicsTask::Dcc {
            standardized_residuals: z,
            a,
            b,
            long_run_q,
            initial_q,
        } => {
            psd(long_run_q, true)?;
            psd(initial_q, true)?;
            let d = long_run_q.len();
            let mut q = initial_q.clone();
            for row in z {
                checkpoint()?;
                for i in 0..d {
                    for j in 0..d {
                        q[i][j] =
                            (1.0 - a - b) * long_run_q[i][j] + a * row[i] * row[j] + b * q[i][j];
                    }
                }
                let mut corr = vec![];
                for i in 0..d {
                    for j in 0..d {
                        corr.push(q[i][j] / (q[i][i] * q[j][j]).sqrt());
                    }
                }
                out.series.push(corr);
            }
            out.final_matrix = q;
            out.assumptions.push("Engle scalar DCC(1,1); rows contain next-step correlations flattened row-major; standardized innovations supplied by caller's volatility model".into());
        }
        DynamicsTask::HarRv {
            realized_variances: v,
            intercept,
            daily,
            weekly,
            monthly,
        } => {
            let n = v.len();
            out.values.push(nv(
                "next_realized_variance",
                intercept
                    + daily * v[n - 1]
                    + weekly * v[n - 5..].iter().sum::<f64>() / 5.0
                    + monthly * v[n - 22..].iter().sum::<f64>() / 22.0,
            ));
            out.assumptions.push("HAR uses overlapping 1/5/22 day averages; unconstrained coefficients may produce negative forecasts, which are reported unchanged".into());
        }
        DynamicsTask::RealizedVariance { log_returns } => {
            out.values.push(if log_returns.is_empty() {
                NamedValue {
                    name: "realized_variance".into(),
                    value: Scalar::InsufficientData,
                }
            } else {
                nv("realized_variance", dot(log_returns, log_returns))
            });
        }
        DynamicsTask::Parkinson {
            high,
            low,
            periods_per_year,
        } => {
            if high.is_empty() {
                out.values.push(NamedValue {
                    name: "annualized_volatility".into(),
                    value: Scalar::InsufficientData,
                });
            } else {
                let variance = high
                    .iter()
                    .zip(low)
                    .map(|(h, l)| (h / l).ln().powi(2))
                    .sum::<f64>()
                    / (4.0 * 2f64.ln() * high.len() as f64);
                out.values.extend([
                    nv("period_variance", variance),
                    nv(
                        "annualized_volatility",
                        (variance * periods_per_year).sqrt(),
                    ),
                ]);
            }
        }
        DynamicsTask::OrnsteinUhlenbeck {
            current,
            long_run_mean,
            reversion: k,
            diffusion: s,
            elapsed: t,
        } => {
            let decay = (-k * t).exp();
            out.values.extend([
                nv(
                    "conditional_mean",
                    long_run_mean + (current - long_run_mean) * decay,
                ),
                nv(
                    "conditional_variance",
                    s * s * (-(-2.0 * k * t).exp_m1()) / (2.0 * k),
                ),
                nv("half_life", 2f64.ln() / k),
            ]);
        }
        DynamicsTask::Arima {
            observations,
            ar,
            ma,
            intercept,
            differences,
        } => {
            let mut levels = vec![observations.clone()];
            for _ in 0..*differences {
                let last = &levels[levels.len() - 1];
                levels.push(last.windows(2).map(|p| p[1] - p[0]).collect());
            }
            let data = &levels[levels.len() - 1];
            let p = ar.len();
            let mut residuals = vec![0.0; data.len()];
            for t in p..data.len() {
                checkpoint()?;
                let mut fit = *intercept;
                for (j, a) in ar.iter().enumerate() {
                    fit += a * data[t - 1 - j];
                }
                for (j, b) in ma.iter().enumerate() {
                    if t > j {
                        fit += b * residuals[t - 1 - j];
                    }
                }
                residuals[t] = data[t] - fit;
                out.series.push(vec![fit, residuals[t]]);
            }
            let n = data.len();
            let mut next = *intercept;
            for (j, a) in ar.iter().enumerate() {
                next += a * data[n - 1 - j];
            }
            for (j, b) in ma.iter().enumerate() {
                if n > j {
                    next += b * residuals[n - 1 - j];
                }
            }
            for level in levels[..*differences].iter().rev() {
                next += level[level.len() - 1];
            }
            out.values.push(nv("next_level_forecast", next));
            out.assumptions.push("conditional ARIMA recursion, initial MA residuals zero, first p differenced observations seed AR history; series starts at p+d; no stationarity/invertibility claim or parameter fitting".into());
        }
        DynamicsTask::Kalman {
            observations: z,
            transition: f,
            observation: h,
            process_covariance: q,
            observation_covariance: v,
            initial_state,
            initial_covariance,
        } => {
            psd(q, false)?;
            psd(v, true)?;
            psd(initial_covariance, false)?;
            let mut x = initial_state.clone();
            let mut p = initial_covariance.clone();
            let ft = linalg::transpose(f);
            let ht = linalg::transpose(h);
            for observation in z {
                checkpoint()?;
                let predicted = matvec(f, &x);
                let pp = add(&linalg::multiply(&linalg::multiply(f, &p), &ft), q);
                let cross = linalg::multiply(&pp, &ht);
                let s = add(&linalg::multiply(h, &cross), v);
                let k = linalg::multiply(&cross, &linalg::inverse(&s)?);
                let hx = matvec(h, &predicted);
                let innovation: Vec<_> = observation.iter().zip(hx).map(|(a, b)| a - b).collect();
                let update = matvec(&k, &innovation);
                x = predicted.iter().zip(update).map(|(a, b)| a + b).collect();
                let kh = linalg::multiply(&k, h);
                let identity = linalg::identity(x.len());
                let ikh: Matrix = identity
                    .iter()
                    .zip(kh)
                    .map(|(r, k)| r.iter().zip(k).map(|(a, b)| a - b).collect())
                    .collect();
                p = add(
                    &linalg::multiply(&linalg::multiply(&ikh, &pp), &linalg::transpose(&ikh)),
                    &linalg::multiply(&linalg::multiply(&k, v), &linalg::transpose(&k)),
                );
                out.series.push(x.clone());
            }
            out.final_matrix = p;
            for (i, v) in x.iter().enumerate() {
                out.values.push(nv(&format!("final_state_{i}"), *v));
            }
            out.assumptions.push("predict then update for each observation; Joseph covariance update; filtering only, no future-data smoother".into());
        }
        DynamicsTask::Hawkes {
            baseline,
            alpha,
            beta,
            arrivals,
            horizon,
        } => {
            let d = baseline.len();
            let mut state = vec![vec![0.0; d]; d];
            let mut log_likelihood = 0.0;
            let mut previous = 0.0;
            for arrival in arrivals {
                checkpoint()?;
                let dt = arrival.time - previous;
                for i in 0..d {
                    for j in 0..d {
                        state[i][j] *= (-beta[i][j] * dt).exp();
                    }
                }
                let intensity =
                    baseline[arrival.channel] + state[arrival.channel].iter().sum::<f64>();
                log_likelihood += intensity.ln();
                out.series.push(vec![intensity]);
                for i in 0..d {
                    state[i][arrival.channel] += alpha[i][arrival.channel];
                }
                previous = arrival.time;
            }
            let mut integral = baseline.iter().sum::<f64>() * horizon;
            for arrival in arrivals {
                for i in 0..d {
                    let j = arrival.channel;
                    integral += alpha[i][j] / beta[i][j]
                        * (-(-beta[i][j] * (horizon - arrival.time)).exp_m1());
                }
            }
            out.values
                .push(nv("log_likelihood", log_likelihood - integral));
            for i in 0..d {
                let intensity = baseline[i]
                    + (0..d)
                        .map(|j| state[i][j] * (-beta[i][j] * (horizon - previous)).exp())
                        .sum::<f64>();
                out.values
                    .push(nv(&format!("terminal_intensity_{i}"), intensity));
            }
            let kernel: Matrix = (0..d)
                .map(|i| (0..d).map(|j| alpha[i][j] / beta[i][j]).collect())
                .collect();
            // Collatz-Wielandt bounds on a shifted nonnegative matrix also handle reducibility.
            let mut x = vec![1.0; d];
            let (mut lower, mut upper) = (0.0, f64::INFINITY);
            for _ in 0..1000 {
                checkpoint()?;
                let y: Vec<_> = matvec(&kernel, &x)
                    .iter()
                    .zip(&x)
                    .map(|(a, b)| a + b)
                    .collect();
                lower = y
                    .iter()
                    .zip(&x)
                    .map(|(a, b)| a / b - 1.0)
                    .fold(f64::INFINITY, f64::min);
                upper = y
                    .iter()
                    .zip(&x)
                    .map(|(a, b)| a / b - 1.0)
                    .fold(f64::NEG_INFINITY, f64::max);
                if upper - lower < 1e-10 {
                    break;
                }
                let scale = y.iter().copied().fold(0.0, f64::max);
                x = y.iter().map(|v| (v / scale).max(1e-100)).collect();
            }
            out.values.extend([
                nv("branching_radius_lower_bound", lower.max(0.0)),
                nv("branching_radius_upper_bound", upper.max(0.0)),
            ]);
            out.final_matrix = kernel;
            out.assumptions.push("multivariate exponential kernels; empty history at t=0; event intensity is left limit, terminal intensity includes arrivals at horizon; stationary only if spectral upper bound <1; bounded spectral estimate may remain inconclusive".into());
        }
    }
    if out
        .series
        .iter()
        .flatten()
        .chain(out.final_matrix.iter().flatten())
        .any(|v| !v.is_finite())
    {
        return Err(linalg::failure("non-finite dynamic state"));
    }
    checkpoint()?;
    Ok(out)
}
