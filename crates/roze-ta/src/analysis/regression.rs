//! Bounded in-sample regression and PCA with explicit fit cutoff.
use super::*;
use linalg::{dot, Matrix};
use research::{NamedValue, ResearchRow};
use statrs::distribution::{ContinuousCDF, StudentsT};
pub const VERSION: &str = "roze-ta-regression-v1";
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum RegressionMethod {
    Ols,
    Ridge {
        lambda: f64,
    },
    Lasso {
        lambda: f64,
        max_iterations: usize,
        tolerance: f64,
    },
    Logistic {
        l2: f64,
        max_iterations: usize,
        tolerance: f64,
    },
    Pca {
        components: usize,
    },
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct RegressionSpec {
    /// First value is the target for regressions, all columns are features for PCA.
    pub rows: Vec<ResearchRow>,
    pub intercept: bool,
    pub method: RegressionMethod,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct RegressionResult {
    pub method_version: String,
    pub fit_cutoff_ms: i64,
    pub selected_samples: usize,
    pub selection_hash: String,
    pub coefficients: Vec<f64>,
    pub standard_errors: Vec<Scalar>,
    pub coefficient_p_values: Vec<Scalar>,
    pub fitted: Vec<f64>,
    pub residuals: Vec<f64>,
    pub diagnostics: Vec<NamedValue>,
    pub loadings: Matrix,
    pub feature_means: Vec<f64>,
    pub eigenvalues: Vec<f64>,
    pub converged: bool,
    pub iterations: usize,
    pub assumptions: Vec<String>,
}
impl RegressionSpec {
    pub(super) fn validate(&self, request: &Request) -> Result<(usize, usize), TaError> {
        if !request.points.is_empty()
            || !request.events.is_empty()
            || request.input_kind != "regression_matrix"
        {
            return Err(err(
                ErrorCode::InvalidParameter,
                "regression requires regression_matrix and empty points/events",
            ));
        }
        let width = self.rows.first().map_or(0, |r| r.values.len());
        let pca = matches!(self.method, RegressionMethod::Pca { .. });
        if self.rows.len() > MAX_SAMPLES
            || width < if pca { 1 } else { 2 }
            || width > if pca { 16 } else { 17 }
        {
            return Err(err(
                ErrorCode::LimitExceeded,
                "regression requires 1..16 features, optional target, at most 4096 rows",
            ));
        }
        let (mut previous, mut selected, mut unavailable) = (0, 0, false);
        for row in &self.rows {
            if row.at_ms <= previous || row.available_at_ms < row.at_ms {
                return Err(err(
                    ErrorCode::InvalidTime,
                    "regression requires increasing positive timestamps",
                ));
            }
            previous = row.at_ms;
            if row.values.len() != width
                || row.values.iter().any(|v| !v.is_finite() || v.abs() > 1e30)
            {
                return Err(err(
                    ErrorCode::InvalidSample,
                    "regression rows must be finite, bounded and rectangular",
                ));
            }
            if matches!(self.method, RegressionMethod::Logistic { .. })
                && row.values[0] != 0.0
                && row.values[0] != 1.0
            {
                return Err(err(
                    ErrorCode::InvalidSample,
                    "logistic target must be 0 or 1",
                ));
            }
            if row.available_at_ms <= request.fit_cutoff_ms {
                if unavailable {
                    return Err(err(
                        ErrorCode::InvalidTime,
                        "regression requires available prefix",
                    ));
                }
                selected += 1;
            } else {
                unavailable = true;
            }
        }
        if selected <= width {
            return Err(err(
                ErrorCode::InsufficientData,
                "more selected rows than columns required",
            ));
        }
        let iterations = match self.method {
            RegressionMethod::Ridge { lambda } => {
                penalty(lambda)?;
                1
            }
            RegressionMethod::Lasso {
                lambda,
                max_iterations,
                tolerance,
            } => {
                penalty(lambda)?;
                iteration_limit(max_iterations, tolerance)?;
                max_iterations
            }
            RegressionMethod::Logistic {
                l2,
                max_iterations,
                tolerance,
            } => {
                penalty(l2)?;
                iteration_limit(max_iterations, tolerance)?;
                max_iterations
            }
            RegressionMethod::Pca { components } => {
                if components == 0 || components > width || !self.intercept {
                    return Err(err(
                        ErrorCode::InvalidParameter,
                        "PCA requires centered data (intercept=true), components 1..features",
                    ));
                }
                1
            }
            RegressionMethod::Ols => 1,
        };
        Ok((
            selected * width * width * iterations + width.pow(4) * if pca { 100 } else { 1 },
            selected * 3 + width * width * 2 + 64,
        ))
    }
}
fn penalty(v: f64) -> Result<(), TaError> {
    if v.is_finite() && (0.0..=1e12).contains(&v) {
        Ok(())
    } else {
        Err(err(
            ErrorCode::InvalidParameter,
            "penalty must be finite in [0,1e12]",
        ))
    }
}
fn iteration_limit(n: usize, t: f64) -> Result<(), TaError> {
    if (1..=1000).contains(&n) && t.is_finite() && (1e-12..=1e-3).contains(&t) {
        Ok(())
    } else {
        Err(err(
            ErrorCode::InvalidParameter,
            "iterations 1..1000 and tolerance 1e-12..1e-3 required",
        ))
    }
}
fn scalar(name: &str, v: f64) -> NamedValue {
    NamedValue {
        name: name.into(),
        value: Scalar::number(v),
    }
}
fn sigmoid(v: f64) -> f64 {
    if v >= 0.0 {
        1.0 / (1.0 + (-v).exp())
    } else {
        let e = v.exp();
        e / (1.0 + e)
    }
}

pub(super) fn calculate(
    spec: &RegressionSpec,
    cutoff: i64,
    checkpoint: &mut impl FnMut() -> Result<(), TaError>,
) -> Result<RegressionResult, TaError> {
    let rows: Vec<_> = spec
        .rows
        .iter()
        .filter(|r| r.available_at_ms <= cutoff)
        .collect();
    let n = rows.len();
    let mut out=RegressionResult{method_version:VERSION.into(),fit_cutoff_ms:cutoff,selected_samples:n,selection_hash:fingerprint::digest(&rows)?,coefficients:vec![],standard_errors:vec![],coefficient_p_values:vec![],fitted:vec![],residuals:vec![],diagnostics:vec![],loadings:vec![],feature_means:vec![],eigenvalues:vec![],converged:true,iterations:1,assumptions:vec!["in-sample fit only; no historical prediction backfill; features are not silently scaled; coefficients follow supplied column order, intercept first when requested".into()]};
    if let RegressionMethod::Pca { components } = spec.method {
        let matrix: Matrix = rows.iter().map(|r| r.values.clone()).collect();
        let (means, cov) = linalg::covariance(&matrix, 1);
        let (eigen, vectors) = linalg::symmetric_eigen(&cov)?;
        out.feature_means = means;
        out.eigenvalues = eigen;
        out.loadings = vectors.iter().map(|r| r[..components].to_vec()).collect();
        let total = out.eigenvalues.iter().sum::<f64>();
        out.diagnostics.push(if total > 0.0 {
            scalar(
                "explained_variance_ratio",
                out.eigenvalues[..components].iter().sum::<f64>() / total,
            )
        } else {
            NamedValue {
                name: "explained_variance_ratio".into(),
                value: Scalar::undefined("zero_total_variance"),
            }
        });
        return Ok(out);
    }
    let y: Vec<_> = rows.iter().map(|r| r.values[0]).collect();
    let x: Matrix = rows
        .iter()
        .map(|r| {
            let mut v = Vec::new();
            if spec.intercept {
                v.push(1.0);
            }
            v.extend_from_slice(&r.values[1..]);
            v
        })
        .collect();
    let p = x[0].len();
    let mut covariance = None;
    match spec.method {
        RegressionMethod::Ols => {
            let (b, c) = linalg::least_squares(&x, &y)?;
            out.coefficients = b;
            covariance = Some(c);
        }
        RegressionMethod::Ridge { lambda } => {
            // Augmented design avoids squaring the condition number.
            let mut design = x.clone();
            let mut target = y.clone();
            for j in usize::from(spec.intercept)..p {
                let mut row = vec![0.0; p];
                row[j] = lambda.sqrt();
                design.push(row);
                target.push(0.0);
            }
            out.coefficients = linalg::least_squares(&design, &target)?.0;
        }
        RegressionMethod::Lasso {
            lambda,
            max_iterations,
            tolerance,
        } => {
            let mut b = vec![0.0; p];
            let mut residual = y.clone();
            out.converged = false;
            for it in 0..max_iterations {
                checkpoint()?;
                let mut delta: f64 = 0.0;
                for j in 0..p {
                    let z = x.iter().map(|r| r[j] * r[j]).sum::<f64>() / n as f64;
                    let rho = x
                        .iter()
                        .zip(&residual)
                        .map(|(r, e)| r[j] * (e + r[j] * b[j]))
                        .sum::<f64>()
                        / n as f64;
                    let threshold = if spec.intercept && j == 0 {
                        0.0
                    } else {
                        lambda
                    };
                    let next = if z == 0.0 {
                        0.0
                    } else {
                        rho.signum() * (rho.abs() - threshold).max(0.0) / z
                    };
                    let change = next - b[j];
                    for (i, r) in x.iter().enumerate() {
                        residual[i] -= r[j] * change;
                    }
                    delta = delta.max(change.abs());
                    b[j] = next;
                }
                out.iterations = it + 1;
                if delta <= tolerance * (1.0 + b.iter().map(|v| v.abs()).fold(0.0, f64::max)) {
                    out.converged = true;
                    break;
                }
            }
            out.coefficients = b;
            out.assumptions.push("Lasso minimizes SSE/(2n)+lambda*L1; intercept unpenalized; cyclic coordinate descent; convergence flag must be checked".into());
        }
        RegressionMethod::Logistic {
            l2,
            max_iterations,
            tolerance,
        } => {
            let mut b = vec![0.0; p];
            out.converged = false;
            // Global Lipschitz step, stable even under complete separation.
            let lipschitz = x.iter().map(|r| dot(r, r)).sum::<f64>() / (4.0 * n as f64) + l2;
            if lipschitz == 0.0 {
                return Err(linalg::failure("zero logistic design"));
            }
            for it in 0..max_iterations {
                checkpoint()?;
                let probabilities: Vec<_> = x.iter().map(|r| sigmoid(dot(r, &b))).collect();
                let mut gradient = vec![0.0; p];
                for (i, r) in x.iter().enumerate() {
                    for j in 0..p {
                        gradient[j] += (probabilities[i] - y[i]) * r[j] / n as f64;
                    }
                }
                for j in usize::from(spec.intercept)..p {
                    gradient[j] += l2 * b[j];
                }
                let norm = gradient.iter().map(|v| v.abs()).fold(0.0, f64::max);
                out.iterations = it + 1;
                if norm <= tolerance {
                    out.converged = true;
                    break;
                }
                for j in 0..p {
                    b[j] -= gradient[j] / lipschitz;
                }
            }
            out.coefficients = b;
            out.assumptions.push("Logistic minimizes mean binary log loss + L2/2; intercept unpenalized; bounded gradient descent may not converge, especially for unscaled or separable data".into());
        }
        RegressionMethod::Pca { .. } => return Err(linalg::failure("unexpected PCA path")),
    }
    if out.coefficients.iter().any(|v| !v.is_finite()) {
        return Err(linalg::failure("non-finite fitted coefficients"));
    }
    let logistic = matches!(spec.method, RegressionMethod::Logistic { .. });
    out.fitted = x
        .iter()
        .map(|r| {
            let v = dot(r, &out.coefficients);
            if logistic {
                sigmoid(v)
            } else {
                v
            }
        })
        .collect();
    out.residuals = y.iter().zip(&out.fitted).map(|(a, b)| a - b).collect();
    let sse = dot(&out.residuals, &out.residuals);
    out.diagnostics.push(scalar("sse", sse));
    if !logistic {
        let center = if spec.intercept {
            y.iter().sum::<f64>() / n as f64
        } else {
            0.0
        };
        let tss = y.iter().map(|v| (v - center).powi(2)).sum::<f64>();
        if tss > 0.0 {
            let r2 = 1.0 - sse / tss;
            out.diagnostics.push(scalar("r_squared", r2));
            out.diagnostics.push(scalar(
                "adjusted_r_squared",
                1.0 - (1.0 - r2) * (n - usize::from(spec.intercept)) as f64 / (n - p) as f64,
            ));
        }
        if let Some(cov) = covariance {
            let variance = sse / (n - p) as f64;
            let student = StudentsT::new(0.0, 1.0, (n - p) as f64)
                .map_err(|_| linalg::failure("invalid t distribution"))?;
            for (j, row) in cov.iter().enumerate() {
                let se = (variance * row[j]).sqrt();
                out.standard_errors.push(Scalar::number(se));
                out.coefficient_p_values.push(if se > 0.0 {
                    Scalar::number(2.0 * student.sf((out.coefficients[j] / se).abs()))
                } else {
                    Scalar::undefined("zero_residual_variance")
                });
            }
            out.assumptions.push("OLS standard errors and two-sided t tests assume IID homoskedastic normal errors; not HAC factor-alpha inference".into());
        }
    }
    checkpoint()?;
    Ok(out)
}
