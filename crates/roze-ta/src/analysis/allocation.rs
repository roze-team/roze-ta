//! Portfolio mathematics on explicit, already available inputs.
use super::*;
use linalg::{dot, matvec, Matrix};
use research::NamedValue;
pub const VERSION: &str = "roze-ta-allocation-v1";
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum AllocationTask {
    RiskParity {
        covariance: Matrix,
        max_iterations: usize,
        tolerance: f64,
    },
    MinimumVariance {
        covariance: Matrix,
        means: Vec<f64>,
        target_return: Option<f64>,
    },
    MeanVariance {
        covariance: Matrix,
        means: Vec<f64>,
        risk_aversion: f64,
    },
    MaximumSharpe {
        covariance: Matrix,
        means: Vec<f64>,
        risk_free: f64,
    },
    InverseVolatility {
        volatilities: Vec<f64>,
    },
    TargetVolatility {
        current_volatility: f64,
        target_volatility: f64,
        maximum_leverage: f64,
    },
    LedoitWolf {
        observations: Matrix,
    },
    BlackLitterman {
        covariance: Matrix,
        market_weights: Vec<f64>,
        risk_aversion: f64,
        tau: f64,
        views: Matrix,
        view_returns: Vec<f64>,
        view_covariance: Matrix,
    },
    CvarOptimize {
        returns: Matrix,
        confidence: f64,
        max_iterations: usize,
        tolerance: f64,
    },
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct AllocationSpec {
    pub available_at_ms: i64,
    pub task: AllocationTask,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct AllocationResult {
    pub method_version: String,
    pub weights: Vec<f64>,
    pub means: Vec<f64>,
    pub covariance: Matrix,
    pub mean_uncertainty: Matrix,
    pub diagnostics: Vec<NamedValue>,
    pub converged: bool,
    pub iterations: usize,
    pub assumptions: Vec<String>,
}
fn bad(s: &str) -> TaError {
    err(ErrorCode::InvalidParameter, s)
}
fn finite(v: f64) -> bool {
    v.is_finite() && v.abs() <= 1e12
}
fn positive(v: f64) -> bool {
    finite(v) && v > 0.0
}
fn vector(v: &[f64], n: usize) -> Result<(), TaError> {
    if v.len() == n && v.iter().all(|&v| finite(v)) {
        Ok(())
    } else {
        Err(bad(
            "vector shape mismatch or nonfinite/out-of-bounds value",
        ))
    }
}
fn matrix(m: &Matrix, rows: usize, columns: usize) -> Result<(), TaError> {
    if m.len() != rows {
        return Err(bad("matrix shape mismatch"));
    }
    for r in m {
        vector(r, columns)?;
    }
    Ok(())
}
fn cov(m: &Matrix) -> Result<(), TaError> {
    let n = m.len();
    if !(1..=8).contains(&n) {
        return Err(bad("allocation covariance supports 1..8 assets"));
    }
    matrix(m, n, n)?;
    for (i, row) in m.iter().enumerate() {
        for (j, _) in row.iter().enumerate() {
            if (m[i][j] - m[j][i]).abs() > 1e-12 * m[i][j].abs().max(m[j][i].abs()).max(1e-30) {
                return Err(bad("covariance must be symmetric"));
            }
        }
    }
    Ok(())
}
fn spd(m: &Matrix) -> Result<(), TaError> {
    let (e, _) = linalg::symmetric_eigen(m)?;
    if e.iter().any(|v| *v <= 1e-12 * e[0].abs()) {
        return Err(linalg::failure("positive definite covariance required"));
    }
    Ok(())
}
fn observations(m: &Matrix) -> Result<usize, TaError> {
    let n = m.len();
    let p = m.first().map_or(0, Vec::len);
    if !(2..=4096).contains(&n) || !(1..=8).contains(&p) {
        return Err(bad("observations require 2..4096 rows and 1..8 columns"));
    }
    matrix(m, n, p)?;
    Ok(p)
}
impl AllocationSpec {
    pub(super) fn validate(&self, r: &Request) -> Result<(usize, usize), TaError> {
        if r.input_kind != "allocation_parameters" || !r.points.is_empty() || !r.events.is_empty() {
            return Err(bad(
                "allocation requires allocation_parameters and empty points/events",
            ));
        }
        if self.available_at_ms <= 0 || self.available_at_ms > r.fit_cutoff_ms {
            return Err(err(
                ErrorCode::InvalidTime,
                "allocation inputs must be available by fit cutoff",
            ));
        }
        let mut work = 500_000;
        match &self.task {
            AllocationTask::RiskParity {
                covariance,
                max_iterations,
                tolerance,
            } => {
                cov(covariance)?;
                if !(1..=4096).contains(max_iterations)
                    || !tolerance.is_finite()
                    || !(1e-12..=1e-3).contains(tolerance)
                {
                    return Err(bad(
                        "risk parity iterations 1..4096 and tolerance 1e-12..1e-3 required",
                    ));
                }
                work += max_iterations * covariance.len().pow(2) * 8;
            }
            AllocationTask::MinimumVariance {
                covariance,
                means,
                target_return,
            } => {
                cov(covariance)?;
                vector(means, covariance.len())?;
                if target_return.is_some_and(|v| !finite(v)) {
                    return Err(bad("target return must be finite"));
                }
            }
            AllocationTask::MeanVariance {
                covariance,
                means,
                risk_aversion,
            } => {
                cov(covariance)?;
                vector(means, covariance.len())?;
                if !positive(*risk_aversion) {
                    return Err(bad("positive risk aversion required"));
                }
            }
            AllocationTask::MaximumSharpe {
                covariance,
                means,
                risk_free,
            } => {
                cov(covariance)?;
                vector(means, covariance.len())?;
                if !finite(*risk_free) {
                    return Err(bad("finite risk-free return required"));
                }
            }
            AllocationTask::InverseVolatility { volatilities } => {
                if volatilities.is_empty()
                    || volatilities.len() > 8
                    || volatilities.iter().any(|&v| !positive(v))
                {
                    return Err(bad("1..8 positive volatilities required"));
                }
                work = 64;
            }
            AllocationTask::TargetVolatility {
                current_volatility,
                target_volatility,
                maximum_leverage,
            } => {
                if !positive(*current_volatility)
                    || !positive(*target_volatility)
                    || !positive(*maximum_leverage)
                {
                    return Err(bad("positive volatility and leverage parameters required"));
                }
                work = 64;
            }
            AllocationTask::LedoitWolf { observations: m } => {
                let p = observations(m)?;
                work = m.len() * p * p * 4;
            }
            AllocationTask::BlackLitterman {
                covariance,
                market_weights,
                risk_aversion,
                tau,
                views,
                view_returns,
                view_covariance,
            } => {
                cov(covariance)?;
                let n = covariance.len();
                vector(market_weights, n)?;
                if !positive(*risk_aversion)
                    || !positive(*tau)
                    || views.is_empty()
                    || views.len() > 8
                {
                    return Err(bad("positive risk aversion/tau and 1..8 views required"));
                }
                if market_weights.iter().any(|v| *v < 0.0)
                    || (market_weights.iter().sum::<f64>() - 1.0).abs() > 1e-10
                {
                    return Err(bad("market weights must be nonnegative and sum to one"));
                }
                matrix(views, views.len(), n)?;
                vector(view_returns, views.len())?;
                cov(view_covariance)?;
                if view_covariance.len() != views.len() {
                    return Err(bad("view covariance size mismatch"));
                }
            }
            AllocationTask::CvarOptimize {
                returns,
                confidence,
                max_iterations,
                tolerance,
            } => {
                let p = observations(returns)?;
                if !confidence.is_finite()
                    || !(0.5..0.999).contains(confidence)
                    || !(1..=4096).contains(max_iterations)
                    || !tolerance.is_finite()
                    || !(1e-12..=1.0).contains(tolerance)
                {
                    return Err(bad("CVaR requires confidence [0.5,0.999), iterations 1..4096 and tolerance 1e-12..1"));
                }
                work = returns.len() * p * max_iterations * 4;
            }
        }
        Ok((work, 512))
    }
}
fn nv(n: &str, v: f64) -> NamedValue {
    NamedValue {
        name: n.into(),
        value: Scalar::number(v),
    }
}
fn normalize(w: &mut [f64]) -> Result<(), TaError> {
    let sum = w.iter().sum::<f64>();
    if !sum.is_finite() || sum.abs() < 1e-12 * w.iter().map(|v| v.abs()).sum::<f64>() {
        return Err(linalg::failure("zero/unstable weight normalization"));
    }
    for v in w {
        *v /= sum;
    }
    Ok(())
}
fn simplex(x: &[f64]) -> Vec<f64> {
    let mut u = x.to_vec();
    u.sort_by(|a, b| b.total_cmp(a));
    let (mut total, mut theta) = (0.0, 0.0);
    for (j, &v) in u.iter().enumerate() {
        total += v;
        let t = (total - 1.0) / (j + 1) as f64;
        if v > t {
            theta = t;
        }
    }
    x.iter().map(|v| (v - theta).max(0.0)).collect()
}

pub(super) fn calculate(
    spec: &AllocationSpec,
    checkpoint: &mut impl FnMut() -> Result<(), TaError>,
) -> Result<AllocationResult, TaError> {
    let mut out=AllocationResult{method_version:VERSION.into(),weights:vec![],means:vec![],covariance:vec![],mean_uncertainty:vec![],diagnostics:vec![],converged:true,iterations:1,assumptions:vec!["frozen explicit input estimates; common return horizon and currency; no account or execution access".into()]};
    match &spec.task {
        AllocationTask::RiskParity {
            covariance: c,
            max_iterations,
            tolerance,
        } => {
            spd(c)?;
            let n = c.len();
            let budget = 1.0 / n as f64;
            let mut x = vec![1.0; n];
            out.converged = false;
            for iteration in 0..*max_iterations {
                checkpoint()?;
                for j in 0..n {
                    let cross = dot(&c[j], &x) - c[j][j] * x[j];
                    let root = cross.hypot((4.0 * c[j][j] * budget).sqrt());
                    x[j] = if cross >= 0.0 {
                        2.0 * budget / (root + cross)
                    } else {
                        (root - cross) / (2.0 * c[j][j])
                    };
                }
                let marginal = matvec(c, &x);
                let error = x
                    .iter()
                    .zip(marginal)
                    .map(|(x, m)| (x * m - budget).abs())
                    .fold(0.0, f64::max);
                out.iterations = iteration + 1;
                if error <= *tolerance {
                    out.converged = true;
                    break;
                }
            }
            normalize(&mut x)?;
            let marginal = matvec(c, &x);
            let variance = dot(&x, &marginal);
            for j in 0..n {
                out.diagnostics.push(nv(
                    &format!("risk_fraction_{j}"),
                    x[j] * marginal[j] / variance,
                ));
            }
            out.weights = x;
            out.assumptions.push("equal risk budgets, positive weights, SPD covariance; cyclic exact coordinate minimization of x'Sigma*x/2 - sum(log(x_i))/n; convergence must be checked".into());
        }
        AllocationTask::MinimumVariance {
            covariance: c,
            means: m,
            target_return,
        } => {
            spd(c)?;
            let one = vec![1.0; c.len()];
            let a = linalg::solve(c, &one)?;
            out.weights = if let Some(target) = target_return {
                let b = linalg::solve(c, m)?;
                let aa = dot(&one, &a);
                let ab = dot(&one, &b);
                let bb = dot(m, &b);
                let determinant = aa * bb - ab * ab;
                if determinant <= 1e-12 * (aa * bb).abs() {
                    return Err(linalg::failure(
                        "target return constraint redundant or ill-conditioned",
                    ));
                }
                a.iter()
                    .zip(&b)
                    .map(|(a, b)| {
                        a * (bb - ab * target) / determinant + b * (aa * target - ab) / determinant
                    })
                    .collect()
            } else {
                let mut w = a;
                normalize(&mut w)?;
                w
            };
            out.means = m.clone();
            out.covariance = c.clone();
            out.assumptions.push("fully invested analytical minimum variance; shorting allowed; optional exact mean-return equality".into());
        }
        AllocationTask::MeanVariance {
            covariance: c,
            means: m,
            risk_aversion: d,
        } => {
            spd(c)?;
            out.weights = linalg::solve(c, m)?.iter().map(|v| v / d).collect();
            out.means = m.clone();
            out.covariance = c.clone();
            out.assumptions
                .push("unconstrained mean-variance utility; weights are not normalized".into());
        }
        AllocationTask::MaximumSharpe {
            covariance: c,
            means: m,
            risk_free,
        } => {
            spd(c)?;
            let excess: Vec<_> = m.iter().map(|v| v - risk_free).collect();
            out.weights = linalg::solve(c, &excess)?;
            if out.weights.iter().sum::<f64>() <= 0.0 {
                return Err(linalg::failure(
                    "no positive fully-invested tangency normalization",
                ));
            }
            normalize(&mut out.weights)?;
            out.means = m.clone();
            out.covariance = c.clone();
            out.assumptions.push("fully invested unconstrained tangency portfolio, shorting allowed, positive normalization required".into());
        }
        AllocationTask::InverseVolatility { volatilities } => {
            out.weights = volatilities.iter().map(|v| 1.0 / v).collect();
            normalize(&mut out.weights)?;
        }
        AllocationTask::TargetVolatility {
            current_volatility,
            target_volatility,
            maximum_leverage,
        } => {
            out.diagnostics.push(nv(
                "leverage",
                (target_volatility / current_volatility).min(*maximum_leverage),
            ));
        }
        AllocationTask::LedoitWolf { observations: rows } => {
            let n = rows.len();
            let p = rows[0].len();
            let (means, cov) = linalg::covariance(rows, 0);
            let mu = (0..p).map(|i| cov[i][i]).sum::<f64>() / p as f64;
            let mut delta = 0.0;
            for (i, row) in cov.iter().enumerate() {
                for (j, &v) in row.iter().enumerate() {
                    delta += (v - if i == j { mu } else { 0.0 }).powi(2);
                }
            }
            let mut beta = 0.0;
            for row in rows {
                checkpoint()?;
                for i in 0..p {
                    for j in 0..p {
                        beta += ((row[i] - means[i]) * (row[j] - means[j]) - cov[i][j]).powi(2);
                    }
                }
            }
            beta /= (n * n) as f64;
            let shrinkage = if delta > 0.0 {
                (beta / delta).clamp(0.0, 1.0)
            } else {
                0.0
            };
            out.means = means;
            out.covariance = (0..p)
                .map(|i| {
                    (0..p)
                        .map(|j| {
                            (1.0 - shrinkage) * cov[i][j]
                                + if i == j { shrinkage * mu } else { 0.0 }
                        })
                        .collect()
                })
                .collect();
            out.diagnostics.push(nv("shrinkage", shrinkage));
            out.assumptions.push("Ledoit-Wolf scaled-identity target, centered covariance divisor n; differs from constant-correlation target in Honey I Shrunk".into());
        }
        AllocationTask::BlackLitterman {
            covariance: c,
            market_weights,
            risk_aversion: d,
            tau,
            views: p,
            view_returns: q,
            view_covariance: omega,
        } => {
            spd(c)?;
            spd(omega)?;
            let prior: Vec<_> = matvec(c, market_weights).iter().map(|v| d * v).collect();
            let a: Matrix = c
                .iter()
                .map(|r| r.iter().map(|v| tau * v).collect())
                .collect();
            let apt = linalg::multiply(&a, &linalg::transpose(p));
            let mut middle = linalg::multiply(p, &apt);
            for i in 0..middle.len() {
                for j in 0..middle.len() {
                    middle[i][j] += omega[i][j];
                }
            }
            let implied = matvec(p, &prior);
            let diff: Vec<_> = q.iter().zip(&implied).map(|(q, v)| q - v).collect();
            let update = matvec(&apt, &linalg::solve(&middle, &diff)?);
            out.means = prior.iter().zip(update).map(|(a, b)| a + b).collect();
            let reduction = linalg::multiply(
                &linalg::multiply(&apt, &linalg::inverse(&middle)?),
                &linalg::transpose(&apt),
            );
            out.mean_uncertainty = (0..c.len())
                .map(|i| (0..c.len()).map(|j| a[i][j] - reduction[i][j]).collect())
                .collect();
            out.covariance = c.clone();
            out.weights = linalg::solve(c, &out.means)?
                .iter()
                .map(|v| v / d)
                .collect();
            out.assumptions.push("posterior mean uncertainty is reported separately from asset return covariance; implied prior is delta*Sigma*market_weights; unconstrained posterior utility weights".into());
        }
        AllocationTask::CvarOptimize {
            returns,
            confidence,
            max_iterations,
            tolerance,
        } => {
            let n = returns.len();
            let p = returns[0].len();
            let alpha = 1.0 - confidence;
            let mut weights = vec![1.0 / p as f64; p];
            let lower = returns
                .iter()
                .flatten()
                .map(|v| -v)
                .fold(f64::INFINITY, f64::min);
            let upper = returns
                .iter()
                .flatten()
                .map(|v| -v)
                .fold(f64::NEG_INFINITY, f64::max);
            let mut z = (lower + upper) / 2.0;
            let diameter = (2.0 + (upper - lower).powi(2)).sqrt();
            let max_norm = returns.iter().map(|r| dot(r, r).sqrt()).fold(0.0, f64::max);
            let lipschitz = ((max_norm / alpha).powi(2) + (1.0 + 1.0 / alpha).powi(2)).sqrt();
            let (mut best, mut bound, mut best_z) = (f64::INFINITY, f64::NEG_INFINITY, z);
            out.converged = false;
            for it in 0..*max_iterations {
                checkpoint()?;
                let mut objective = z;
                let mut gw = vec![0.0; p];
                let mut gz = 1.0;
                for row in returns {
                    let loss = -dot(row, &weights);
                    if loss > z {
                        objective += (loss - z) / (n as f64 * alpha);
                        gz -= 1.0 / (n as f64 * alpha);
                        for j in 0..p {
                            gw[j] -= row[j] / (n as f64 * alpha);
                        }
                    }
                }
                if objective < best {
                    best = objective;
                    out.weights = weights.clone();
                    best_z = z;
                }
                let support = objective - dot(&gw, &weights) - gz * z
                    + gw.iter().copied().fold(f64::INFINITY, f64::min)
                    + gz * if gz >= 0.0 { lower } else { upper };
                bound = bound.max(support);
                out.iterations = it + 1;
                if best - bound <= *tolerance {
                    out.converged = true;
                    break;
                }
                let step = diameter / (lipschitz * ((it + 1) as f64).sqrt());
                let trial: Vec<_> = weights.iter().zip(&gw).map(|(w, g)| w - step * g).collect();
                weights = simplex(&trial);
                z = (z - step * gz).clamp(lower, upper);
            }
            out.diagnostics.extend([
                nv("cvar_objective", best),
                nv("threshold", best_z),
                nv("objective_lower_bound", bound),
                nv("optimality_gap", (best - bound).max(0.0)),
            ]);
            out.assumptions.push("Rockafellar-Uryasev empirical CVaR; long-only unit simplex, no target-return constraint; bounded projected subgradient with convex supporting-plane lower bound; inspect converged and optimality_gap".into());
        }
    }
    if !out.covariance.is_empty() && !out.weights.is_empty() {
        out.diagnostics.extend([
            nv("expected_return", dot(&out.weights, &out.means)),
            nv(
                "variance",
                dot(&out.weights, &matvec(&out.covariance, &out.weights)),
            ),
        ]);
    }
    if out
        .weights
        .iter()
        .chain(&out.means)
        .chain(out.covariance.iter().flatten())
        .chain(out.mean_uncertainty.iter().flatten())
        .any(|v| !v.is_finite())
    {
        return Err(linalg::failure("non-finite allocation output"));
    }
    checkpoint()?;
    Ok(out)
}
