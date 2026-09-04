//! Explicit Gaussian-null simulation and conditional reduced-rank VECM forecasts.
use super::*;
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct VecmResult {
    pub short_run: Vec<Matrix>,
    pub constant: Vec<f64>,
    pub innovation_covariance: Matrix,
    /// Oldest to newest; sufficient starting history for this fitted model.
    pub lagged_levels: Matrix,
    /// Conditional levels at horizons 1..steps after the last supplied observation.
    pub forecasts: Matrix,
    pub forecast_covariances: Vec<Matrix>,
    pub effective_observations: usize,
}

fn base_task(task: &InferenceTask) -> Result<InferenceTask, TaError> {
    Ok(match task {
        InferenceTask::AdfGaussianCalibration {
            samples,
            lags,
            trend,
            ..
        } => InferenceTask::Adf {
            samples: samples.clone(),
            lags: *lags,
            trend: *trend,
        },
        InferenceTask::EngleGrangerGaussianCalibration {
            dependent,
            independent,
            lags,
            ..
        } => InferenceTask::EngleGranger {
            dependent: dependent.clone(),
            independent: independent.clone(),
            lags: *lags,
        },
        InferenceTask::Vecm {
            observations,
            lagged_differences,
            include_constant,
            rank,
            ..
        } => InferenceTask::Johansen {
            observations: observations.clone(),
            lagged_differences: *lagged_differences,
            include_constant: *include_constant,
            rank: (*rank).max(1),
        },
        _ => return Err(bad("expected extended inference task")),
    })
}

pub(super) fn validate(spec: &InferenceSpec, r: &Request) -> Result<(usize, usize), TaError> {
    let base = InferenceSpec {
        available_at_ms: spec.available_at_ms,
        task: base_task(&spec.task)?,
    };
    let (work, _) = base.validate(r)?;
    match &spec.task {
        InferenceTask::AdfGaussianCalibration {
            samples,
            trend,
            replicates,
            ..
        } => {
            if matches!(trend, Trend::None) && samples[0] != 0.0 {
                return Err(bad(
                    "no-trend Gaussian-null calibration requires initial level zero",
                ));
            }
            simulation_budget(work, *replicates)
        }
        InferenceTask::EngleGrangerGaussianCalibration { replicates, .. } => {
            simulation_budget(work, *replicates)
        }
        InferenceTask::Vecm {
            observations,
            lagged_differences,
            steps,
            ..
        } => {
            if !(1..=64).contains(steps) {
                return Err(bad("VECM forecast steps must be 1..64"));
            }
            let d = observations[0].len();
            Ok((
                work * 2 + steps * (lagged_differences + 2) * d.pow(3) * 8,
                512 + steps * (d + d * d),
            ))
        }
        _ => Err(bad("expected extended inference task")),
    }
}
fn simulation_budget(work: usize, replicates: usize) -> Result<(usize, usize), TaError> {
    if !(99..=4095).contains(&replicates) {
        return Err(bad("calibration replicates must be 99..4095"));
    }
    Ok((work.saturating_mul(replicates + 1), 512))
}

fn walk(n: usize, rng: &mut ChaCha8Rng) -> Vec<f64> {
    let mut x = Vec::with_capacity(n);
    x.push(0.0);
    let mut level = 0.0;
    for _ in 1..n {
        let u = 1.0 - rng.random::<f64>();
        let angle = std::f64::consts::TAU * rng.random::<f64>();
        level += (-2.0 * u.ln()).sqrt() * angle.cos();
        x.push(level);
    }
    x
}
fn statistic(out: &InferenceResult) -> Result<f64, TaError> {
    out.values
        .iter()
        .find_map(|v| match v.value {
            Scalar::Ready { value } if v.name == "adf_statistic" && value.is_finite() => {
                Some(value)
            }
            _ => None,
        })
        .ok_or_else(|| linalg::failure("undefined ADF statistic cannot be calibrated"))
}
pub(super) fn calculate_extended(
    spec: &InferenceSpec,
    checkpoint: &mut impl FnMut() -> Result<(), TaError>,
) -> Result<InferenceResult, TaError> {
    let base = InferenceSpec {
        available_at_ms: spec.available_at_ms,
        task: base_task(&spec.task)?,
    };
    let mut out = calculate(&base, checkpoint)?;
    let (replicates, seed) = match &spec.task {
        InferenceTask::Vecm {
            observations,
            lagged_differences,
            include_constant,
            rank,
            steps,
        } => {
            fit_vecm(
                &mut out,
                observations,
                *lagged_differences,
                *include_constant,
                *rank,
                *steps,
                checkpoint,
            )?;
            return Ok(out);
        }
        InferenceTask::AdfGaussianCalibration {
            replicates, seed, ..
        }
        | InferenceTask::EngleGrangerGaussianCalibration {
            replicates, seed, ..
        } => (*replicates, *seed),
        _ => return Err(bad("expected calibrated inference task")),
    };
    let observed = statistic(&out)?;
    let mut rng = ChaCha8Rng::seed_from_u64(seed);
    let mut null = Vec::with_capacity(replicates);
    for _ in 0..replicates {
        checkpoint()?;
        let task = match &base.task {
            InferenceTask::Adf {
                samples,
                lags,
                trend,
            } => InferenceTask::Adf {
                samples: walk(samples.len(), &mut rng),
                lags: *lags,
                trend: *trend,
            },
            InferenceTask::EngleGranger {
                dependent, lags, ..
            } => InferenceTask::EngleGranger {
                dependent: walk(dependent.len(), &mut rng),
                independent: walk(dependent.len(), &mut rng),
                lags: *lags,
            },
            _ => return Err(bad("invalid Gaussian calibration base")),
        };
        null.push(statistic(&calculate(
            &InferenceSpec {
                available_at_ms: spec.available_at_ms,
                task,
            },
            checkpoint,
        )?)?);
    }
    let count = null.iter().filter(|v| **v <= observed).count();
    let p = (count + 1) as f64 / (replicates + 1) as f64;
    null.sort_by(f64::total_cmp);
    out.values.extend([
        nv("gaussian_null_p_value", p),
        nv("replicates", replicates as f64),
        nv("p_value_resolution", 1.0 / (replicates + 1) as f64),
    ]);
    for (name, alpha) in [
        ("critical_01", 0.01),
        ("critical_05", 0.05),
        ("critical_10", 0.10),
    ] {
        out.values.push(nv(
            name,
            null[(alpha * replicates as f64).ceil() as usize - 1],
        ));
    }
    out.assumptions
        .retain(|a| !a.contains("no p-value") && !a.contains("no ordinary Student-t"));
    out.assumptions.push("Finite-sample lower-tail Monte Carlo calibration: zero-start, zero-drift IID Gaussian random walk null, two independent walks for EG; same sample size, fixed lags and deterministic terms as observed regression; EG re-estimates first-stage intercept and slope in every replicate. Not a MacKinnon response surface or a bootstrap robust to serial correlation, drift or heteroskedasticity. No-trend ADF requires observed initial level zero. p=(1+count(null<=observed))/(B+1); critical values are empirical inverse-ECDF quantiles; no data-dependent lag selection. Seeded ChaCha8 with Box-Muller normals.".into());
    checkpoint()?;
    Ok(out)
}

#[allow(clippy::too_many_arguments)]
fn fit_vecm(
    out: &mut InferenceResult,
    x: &Matrix,
    p: usize,
    include_constant: bool,
    rank: usize,
    steps: usize,
    checkpoint: &mut impl FnMut() -> Result<(), TaError>,
) -> Result<(), TaError> {
    let d = x[0].len();
    let pi = if rank == 0 {
        out.cointegration_vectors = vec![vec![]; d];
        out.adjustment = vec![vec![]; d];
        vec![vec![0.0; d]; d]
    } else {
        linalg::multiply(
            &out.adjustment,
            &linalg::transpose(&out.cointegration_vectors),
        )
    };
    let mut z = vec![];
    let mut target = vec![];
    for t in p + 1..x.len() {
        let correction = linalg::matvec(&pi, &x[t - 1]);
        target.push(
            (0..d)
                .map(|j| x[t][j] - x[t - 1][j] - correction[j])
                .collect::<Vec<_>>(),
        );
        let mut row = if include_constant { vec![1.0] } else { vec![] };
        for lag in 1..=p {
            row.extend((0..d).map(|j| x[t - lag][j] - x[t - lag - 1][j]));
        }
        z.push(row);
    }
    let mut nuisance = vec![vec![0.0; usize::from(include_constant) + p * d]; d];
    if !z[0].is_empty() {
        for (j, col) in linalg::transpose(&target).iter().enumerate() {
            checkpoint()?;
            nuisance[j] = linalg::least_squares(&z, col)?.0;
        }
    }
    let residuals: Matrix = target
        .iter()
        .zip(&z)
        .map(|(row, design)| {
            row.iter()
                .zip(&nuisance)
                .map(|(v, b)| v - dot(design, b))
                .collect()
        })
        .collect();
    let sigma = cross(&residuals, &residuals);
    let offset = usize::from(include_constant);
    let constant = nuisance
        .iter()
        .map(|b| if include_constant { b[0] } else { 0.0 })
        .collect::<Vec<_>>();
    let gamma: Vec<Matrix> = (0..p)
        .map(|lag| {
            nuisance
                .iter()
                .map(|b| b[offset + lag * d..offset + (lag + 1) * d].to_vec())
                .collect()
        })
        .collect();
    let initial = x[x.len() - p - 1..].to_vec();
    let mut history = initial.clone();
    let mut forecasts = vec![];
    for _ in 0..steps {
        checkpoint()?;
        let n = history.len();
        let mut change = linalg::matvec(&pi, &history[n - 1]);
        for (j, value) in change.iter_mut().enumerate() {
            *value += constant[j];
        }
        for (lag, g) in gamma.iter().enumerate() {
            let delta = (0..d)
                .map(|j| history[n - lag - 1][j] - history[n - lag - 2][j])
                .collect::<Vec<_>>();
            for (value, extra) in change.iter_mut().zip(linalg::matvec(g, &delta)) {
                *value += extra;
            }
        }
        let next = history[n - 1]
            .iter()
            .zip(change)
            .map(|(v, c)| v + c)
            .collect::<Vec<_>>();
        if next.iter().any(|v| !v.is_finite()) {
            return Err(linalg::failure("VECM forecast overflow"));
        }
        forecasts.push(next.clone());
        history.push(next);
    }
    // Equivalent VAR(p+1): A1=I+Pi+Gamma1, Aj=Gammaj-Gamma(j-1), A(p+1)=-Gammap.
    let mut var = vec![vec![vec![0.0; d]; d]; p + 1];
    for (lag, matrix) in var.iter_mut().enumerate() {
        for (i, row) in matrix.iter_mut().enumerate() {
            for (j, value) in row.iter_mut().enumerate() {
                *value = if lag == 0 {
                    f64::from(i == j) + pi[i][j]
                } else {
                    -gamma[lag - 1][i][j]
                };
                if lag < p {
                    *value += gamma[lag][i][j];
                }
            }
        }
    }
    let mut impulse = vec![linalg::identity(d)];
    let mut covariance = vec![vec![0.0; d]; d];
    let mut forecast_covariances = vec![];
    for h in 0..steps {
        checkpoint()?;
        if h > 0 {
            let mut response = vec![vec![0.0; d]; d];
            for lag in 1..=h.min(p + 1) {
                let term = linalg::multiply(&var[lag - 1], &impulse[h - lag]);
                for (row, extra) in response.iter_mut().zip(term) {
                    for (v, e) in row.iter_mut().zip(extra) {
                        *v += e;
                    }
                }
            }
            impulse.push(response);
        }
        let term = linalg::multiply(
            &linalg::multiply(&impulse[h], &sigma),
            &linalg::transpose(&impulse[h]),
        );
        for (row, extra) in covariance.iter_mut().zip(term) {
            for (v, e) in row.iter_mut().zip(extra) {
                *v += e;
                if !v.is_finite() {
                    return Err(linalg::failure("VECM forecast covariance overflow"));
                }
            }
        }
        forecast_covariances.push(covariance.clone());
    }
    out.vecm = Some(VecmResult {
        short_run: gamma,
        constant,
        innovation_covariance: sigma,
        lagged_levels: initial,
        forecasts,
        forecast_covariances,
        effective_observations: target.len(),
    });
    out.assumptions.push("VECM conditional Gaussian reduced-rank fit; supplied rank 0..dimension, unrestricted constant only; short_run matrices ordered Gamma1..Gammap, row is equation and column is lagged variable; covariance is residual cross-product divided by effective observations. Forecasts assume zero future innovations and fixed parameters; covariance propagates innovations through the equivalent VAR and excludes parameter/rank uncertainty. Horizons are observation steps after the final supplied row, not calendar timestamps; all supplied observations must be known at fit cutoff.".into());
    Ok(())
}
