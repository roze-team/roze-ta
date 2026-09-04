//! Bounded, deterministic conditional likelihood fitting using the core recursions.
use super::*;
use dynamics::{DynamicsResult, DynamicsSpec, DynamicsTask};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct FitResult {
    pub parameters: Vec<NamedValue>,
    pub objective: f64,
    pub initial_objective: f64,
    pub iterations: usize,
    pub evaluations: usize,
    pub converged: bool,
    pub normalized_step: f64,
    pub bound_hits: Vec<String>,
    pub model: DynamicsTask,
    pub model_result: DynamicsResult,
    pub assumptions: Vec<String>,
}

fn parameters(model: &DynamicsTask) -> Result<Vec<(String, f64)>, TaError> {
    let mut result = vec![];
    match model {
        DynamicsTask::Garch {
            omega, alpha, beta, ..
        } => {
            result.push(("omega".into(), *omega));
            result.extend(
                alpha
                    .iter()
                    .enumerate()
                    .map(|(i, v)| (format!("alpha_{i}"), *v)),
            );
            result.extend(
                beta.iter()
                    .enumerate()
                    .map(|(i, v)| (format!("beta_{i}"), *v)),
            );
        }
        DynamicsTask::Arima {
            intercept, ar, ma, ..
        } => {
            result.push(("intercept".into(), *intercept));
            result.extend(ar.iter().enumerate().map(|(i, v)| (format!("ar_{i}"), *v)));
            result.extend(ma.iter().enumerate().map(|(i, v)| (format!("ma_{i}"), *v)));
        }
        DynamicsTask::Dcc { a, b, .. } => result.extend([("a".into(), *a), ("b".into(), *b)]),
        DynamicsTask::Hawkes {
            baseline,
            alpha,
            beta,
            ..
        } => {
            result.extend(
                baseline
                    .iter()
                    .enumerate()
                    .map(|(i, v)| (format!("baseline_{i}"), *v)),
            );
            for (name, matrix) in [("alpha", alpha), ("beta", beta)] {
                for (i, row) in matrix.iter().enumerate() {
                    result.extend(
                        row.iter()
                            .enumerate()
                            .map(|(j, v)| (format!("{name}_{i}_{j}"), *v)),
                    );
                }
            }
        }
        _ => {
            return Err(bad(
                "dynamics_fit supports GARCH, ARIMA, DCC and exponential Hawkes",
            ))
        }
    }
    Ok(result)
}
fn set_parameters(model: &mut DynamicsTask, values: &[f64]) -> bool {
    match model {
        DynamicsTask::Garch {
            omega, alpha, beta, ..
        } => {
            *omega = values[0];
            let p = alpha.len();
            alpha.copy_from_slice(&values[1..1 + p]);
            beta.copy_from_slice(&values[1 + p..]);
            alpha.iter().chain(beta.iter()).sum::<f64>() < 1.0
        }
        DynamicsTask::Arima {
            intercept, ar, ma, ..
        } => {
            *intercept = values[0];
            let p = ar.len();
            ar.copy_from_slice(&values[1..1 + p]);
            ma.copy_from_slice(&values[1 + p..]);
            true
        }
        DynamicsTask::Dcc { a, b, .. } => {
            *a = values[0];
            *b = values[1];
            *a + *b < 1.0
        }
        DynamicsTask::Hawkes {
            baseline,
            alpha,
            beta,
            ..
        } => {
            let d = baseline.len();
            baseline.copy_from_slice(&values[..d]);
            for (i, row) in alpha.iter_mut().enumerate() {
                row.copy_from_slice(&values[d + i * d..d + (i + 1) * d]);
            }
            for (i, row) in beta.iter_mut().enumerate() {
                row.copy_from_slice(&values[d + d * d + i * d..d + d * d + (i + 1) * d]);
            }
            true
        }
        _ => false,
    }
}
pub(super) fn validate(spec: &InferenceSpec, r: &Request) -> Result<(usize, usize), TaError> {
    let InferenceTask::DynamicsFit {
        model,
        lower,
        upper,
        max_iterations,
        tolerance,
    } = &spec.task
    else {
        return Err(bad("expected dynamics_fit"));
    };
    let params = parameters(model)?;
    let k = params.len();
    if k > 12
        || lower.len() != k
        || upper.len() != k
        || !(1..=1000).contains(max_iterations)
        || !tolerance.is_finite()
        || !(1e-8..=0.01).contains(tolerance)
    {
        return Err(bad("fit requires <=12 parameters, matching bounds, iterations 1..1000, normalized tolerance 1e-8..0.01"));
    }
    for (i, ((_, initial), (lo, hi))) in params.iter().zip(lower.iter().zip(upper)).enumerate() {
        if !lo.is_finite()
            || !hi.is_finite()
            || lo.abs() > 1e12
            || hi.abs() > 1e12
            || lo >= hi
            || initial < lo
            || initial > hi
        {
            return Err(bad(
                "finite ordered bounds <=1e12 must contain initial parameters",
            ));
        }
        let positive = match model.as_ref() {
            DynamicsTask::Garch { .. } => i == 0,
            DynamicsTask::Hawkes { baseline, .. } => {
                i < baseline.len() || i >= baseline.len() + baseline.len().pow(2)
            }
            _ => false,
        };
        if (positive && *lo <= 0.0)
            || (!matches!(model.as_ref(), DynamicsTask::Arima { .. }) && *lo < 0.0)
        {
            return Err(bad("fit bounds must preserve positive scale/baseline/decay and nonnegative variance/kernel parameters"));
        }
    }
    let mut request = r.clone();
    request.input_kind = "dynamics_parameters".into();
    let (base, output) = DynamicsSpec {
        available_at_ms: spec.available_at_ms,
        task: model.as_ref().clone(),
    }
    .validate(&request)?;
    let (n, extra) = match model.as_ref() {
        DynamicsTask::Garch { residuals, .. } => (residuals.len(), residuals.len() * 16),
        DynamicsTask::Arima {
            observations,
            ar,
            differences,
            ..
        } => (
            observations.len() - ar.len() - differences,
            observations.len() * 8,
        ),
        DynamicsTask::Dcc {
            standardized_residuals,
            long_run_q,
            ..
        } => {
            if long_run_q.len() < 2 {
                return Err(bad("DCC fitting requires at least two variables"));
            }
            (
                standardized_residuals.len(),
                standardized_residuals.len() * long_run_q.len().pow(3) * 8,
            )
        }
        DynamicsTask::Hawkes { arrivals, .. } => (arrivals.len(), arrivals.len() * 8),
        _ => return Err(bad("unsupported fitted model")),
    };
    enough(n, k + 2)?;
    let evaluations = 2 * k * max_iterations + 2;
    Ok(((base + extra).saturating_mul(evaluations), output * 2 + 512))
}
fn scalar(out: &DynamicsResult, name: &str) -> Result<f64, TaError> {
    out.values
        .iter()
        .find_map(|v| match v.value {
            Scalar::Ready { value } if v.name == name && value.is_finite() => Some(value),
            _ => None,
        })
        .ok_or_else(|| linalg::failure("undefined likelihood"))
}
// Cholesky yields log determinant and quadratic form without an explicit inverse.
fn gaussian_correlation(r: &Matrix, z: &[f64]) -> Result<f64, TaError> {
    let d = z.len();
    let mut l = vec![vec![0.0; d]; d];
    let mut logdet = 0.0;
    for i in 0..d {
        for j in 0..=i {
            let v = r[i][j] - (0..j).map(|k| l[i][k] * l[j][k]).sum::<f64>();
            l[i][j] = if i == j {
                if v <= 1e-14 || !v.is_finite() {
                    return Err(linalg::failure("singular fitted correlation"));
                }
                logdet += v.ln();
                v.sqrt()
            } else {
                v / l[j][j]
            };
        }
    }
    let mut solved = vec![0.0; d];
    for i in 0..d {
        solved[i] = (z[i] - (0..i).map(|j| l[i][j] * solved[j]).sum::<f64>()) / l[i][i];
    }
    Ok(0.5 * (logdet + dot(&solved, &solved)))
}
fn objective(model: &DynamicsTask, out: &DynamicsResult) -> Result<f64, TaError> {
    let value = match model {
        DynamicsTask::Garch { residuals, .. } => {
            let mut total = 0.0;
            for (e, row) in residuals.iter().zip(&out.series) {
                let h = row[0];
                if h <= 0.0 {
                    return Err(linalg::failure("nonpositive fitted variance"));
                }
                total += 0.5 * (std::f64::consts::TAU.ln() + h.ln() + e * e / h);
            }
            total
        }
        DynamicsTask::Arima { .. } => out.series.iter().map(|row| row[1].powi(2)).sum::<f64>(),
        DynamicsTask::Hawkes { .. } => -scalar(out, "log_likelihood")?,
        DynamicsTask::Dcc {
            standardized_residuals: z,
            initial_q,
            ..
        } => {
            let d = initial_q.len();
            let initial: Matrix = (0..d)
                .map(|i| {
                    (0..d)
                        .map(|j| initial_q[i][j] / (initial_q[i][i] * initial_q[j][j]).sqrt())
                        .collect()
                })
                .collect();
            let mut total = 0.0;
            for (t, row) in z.iter().enumerate() {
                let correlation = if t == 0 {
                    initial.clone()
                } else {
                    out.series[t - 1].chunks(d).map(|r| r.to_vec()).collect()
                };
                total += gaussian_correlation(&correlation, row)?;
            }
            total
        }
        _ => return Err(bad("unsupported likelihood")),
    };
    if value.is_finite() {
        Ok(value)
    } else {
        Err(linalg::failure("nonfinite fit objective"))
    }
}

pub(super) fn fit(
    spec: &InferenceSpec,
    checkpoint: &mut impl FnMut() -> Result<(), TaError>,
) -> Result<FitResult, TaError> {
    let InferenceTask::DynamicsFit {
        model,
        lower,
        upper,
        max_iterations,
        tolerance,
    } = &spec.task
    else {
        return Err(bad("expected dynamics_fit"));
    };
    let params = parameters(model)?;
    let mut best: Vec<_> = params
        .iter()
        .enumerate()
        .map(|(i, (_, v))| (v - lower[i]) / (upper[i] - lower[i]))
        .collect();
    let mut trial_model = model.as_ref().clone();
    let mut evaluations = 0;
    let mut evaluate = |normalized: &[f64]| -> Result<f64, TaError> {
        checkpoint()?;
        evaluations += 1;
        let physical: Vec<_> = normalized
            .iter()
            .enumerate()
            .map(|(i, v)| lower[i] + v * (upper[i] - lower[i]))
            .collect();
        if !set_parameters(&mut trial_model, &physical) {
            return Ok(f64::INFINITY);
        }
        let result = dynamics::calculate(
            &DynamicsSpec {
                available_at_ms: spec.available_at_ms,
                task: trial_model.clone(),
            },
            checkpoint,
        )
        .and_then(|out| objective(&trial_model, &out));
        match result {
            Err(e) if e.code == ErrorCode::NumericalFailure => Ok(f64::INFINITY),
            other => other,
        }
    };
    let mut score = evaluate(&best)?;
    if !score.is_finite() {
        return Err(linalg::failure("initial model has no finite objective"));
    }
    let initial_objective = score;
    let mut step = 0.25;
    let mut iterations = 0;
    for iteration in 0..*max_iterations {
        iterations = iteration + 1;
        let mut candidate = best.clone();
        let mut candidate_score = score;
        for i in 0..best.len() {
            for sign in [-1.0, 1.0] {
                let mut trial = best.clone();
                trial[i] = (trial[i] + sign * step).clamp(0.0, 1.0);
                let value = evaluate(&trial)?;
                if value < candidate_score {
                    candidate = trial;
                    candidate_score = value;
                }
            }
        }
        if candidate_score < score {
            best = candidate;
            score = candidate_score;
        } else {
            step *= 0.5;
        }
        if step <= *tolerance {
            break;
        }
    }
    let physical: Vec<_> = best
        .iter()
        .enumerate()
        .map(|(i, v)| lower[i] + v * (upper[i] - lower[i]))
        .collect();
    if !set_parameters(&mut trial_model, &physical) {
        return Err(linalg::failure("fitted model violated constraints"));
    }
    let mut result = dynamics::calculate(
        &DynamicsSpec {
            available_at_ms: spec.available_at_ms,
            task: trial_model.clone(),
        },
        checkpoint,
    )?;
    result
        .assumptions
        .retain(|a| !a.contains("no stationarity/invertibility claim or parameter fitting"));
    Ok(FitResult {
        parameters:params.iter().zip(&physical).map(|((name,_),v)|nv(name,*v)).collect(),
        objective:score,initial_objective,iterations,evaluations,converged:step<=*tolerance,normalized_step:step,
        bound_hits:params.iter().zip(&best).filter(|(_,v)|**v==0.0 || **v==1.0).map(|((name,_),_)|name.clone()).collect(),
        model:trial_model,model_result:result,
        assumptions:vec!["Deterministic bounded coordinate pattern search from supplied initial parameters; coordinates scaled by explicit bounds. Converged means unsuccessful coordinate polls reduced normalized step to tolerance; not a global optimum or statistical identification guarantee. At iteration limit, converged=false and best feasible parameters are returned. Initial states, sample order and bounds are frozen; parameters at bounds are reported.".into(),
            "GARCH objective is conditional Gaussian negative log likelihood for supplied zero-mean residuals, with sum(alpha,beta)<1. ARIMA minimizes conditional squared errors with zero initial MA residuals, fixed p,d,q and intercept in differenced units; no automatic order selection or stationarity/invertibility guarantee. DCC minimizes correlation Gaussian quasi-likelihood with supplied standardized residuals and fixed Qbar/Q0; each observation is scored against its pre-observation correlation, with a+b<1. Hawkes minimizes negative event log likelihood on the supplied finite horizon with empty starting history; stationarity bounds are diagnostic, not imposed. All inference uncertainty and mean/volatility preprocessing remain explicit caller responsibilities.".into()],
    })
}
