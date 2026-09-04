//! Small, bounded dense linear algebra used by statistical models.
use super::*;
pub(super) type Matrix = Vec<Vec<f64>>;
pub(super) fn failure(reason: &str) -> TaError {
    err(ErrorCode::NumericalFailure, reason)
}
pub(super) fn identity(n: usize) -> Matrix {
    (0..n)
        .map(|i| (0..n).map(|j| if i == j { 1.0 } else { 0.0 }).collect())
        .collect()
}
pub(super) fn transpose(a: &Matrix) -> Matrix {
    (0..a[0].len())
        .map(|j| a.iter().map(|r| r[j]).collect())
        .collect()
}
pub(super) fn multiply(a: &Matrix, b: &Matrix) -> Matrix {
    a.iter()
        .map(|r| {
            (0..b[0].len())
                .map(|j| r.iter().enumerate().map(|(k, x)| x * b[k][j]).sum())
                .collect()
        })
        .collect()
}
pub(super) fn matvec(a: &Matrix, x: &[f64]) -> Vec<f64> {
    a.iter()
        .map(|r| r.iter().zip(x).map(|(a, b)| a * b).sum())
        .collect()
}
pub(super) fn dot(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(a, b)| a * b).sum()
}
/// Gaussian elimination with scaled partial pivoting, for small nonsingular systems.
pub(super) fn solve(a: &Matrix, b: &[f64]) -> Result<Vec<f64>, TaError> {
    let n = a.len();
    let mut a = a.clone();
    let mut b = b.to_vec();
    let scale: Vec<_> = a
        .iter()
        .map(|r| r.iter().map(|v| v.abs()).fold(0.0, f64::max))
        .collect();
    let mut scale = scale;
    for k in 0..n {
        let mut pivot = k;
        for i in k + 1..n {
            if a[i][k].abs() / scale[i] > a[pivot][k].abs() / scale[pivot] {
                pivot = i;
            }
        }
        if scale[pivot] == 0.0 || a[pivot][k].abs() <= 1e-12 * scale[pivot] {
            return Err(failure("singular or ill-conditioned matrix"));
        }
        a.swap(k, pivot);
        b.swap(k, pivot);
        scale.swap(k, pivot);
        for i in k + 1..n {
            let f = a[i][k] / a[k][k];
            let (before, after) = a.split_at_mut(i);
            for (value, pivot_value) in after[0][k + 1..].iter_mut().zip(&before[k][k + 1..]) {
                *value -= f * pivot_value;
            }
            a[i][k] = 0.0;
            b[i] -= f * b[k];
        }
    }
    let mut x = vec![0.0; n];
    for i in (0..n).rev() {
        x[i] = (b[i] - dot(&a[i][i + 1..], &x[i + 1..])) / a[i][i];
    }
    if x.iter().any(|v| !v.is_finite()) {
        return Err(failure("linear solve overflow"));
    }
    Ok(x)
}
pub(super) fn inverse(a: &Matrix) -> Result<Matrix, TaError> {
    let n = a.len();
    let mut columns = vec![];
    for j in 0..n {
        let mut e = vec![0.0; n];
        e[j] = 1.0;
        columns.push(solve(a, &e)?);
    }
    Ok(transpose(&columns))
}
/// Reorthogonalized modified Gram-Schmidt; no normal equations for ordinary OLS.
pub(super) fn least_squares(x: &Matrix, y: &[f64]) -> Result<(Vec<f64>, Matrix), TaError> {
    let p = x[0].len();
    let mut cols = transpose(x);
    let mut r = vec![vec![0.0; p]; p];
    for j in 0..p {
        let original = dot(&cols[j], &cols[j]).sqrt();
        for _ in 0..2 {
            for i in 0..j {
                let projection = dot(&cols[i], &cols[j]);
                r[i][j] += projection;
                let (before, after) = cols.split_at_mut(j);
                for (v, q) in after[0].iter_mut().zip(&before[i]) {
                    *v -= projection * q;
                }
            }
        }
        let norm = dot(&cols[j], &cols[j]).sqrt();
        if norm <= 1e-12 * original.max(f64::MIN_POSITIVE) {
            return Err(failure("rank deficient regression design"));
        }
        r[j][j] = norm;
        for v in &mut cols[j] {
            *v /= norm;
        }
    }
    let qy: Vec<_> = cols.iter().map(|q| dot(q, y)).collect();
    let beta = solve(&r, &qy)?;
    let ri = inverse(&r)?;
    Ok((beta, multiply(&ri, &transpose(&ri))))
}
/// Jacobi rotations of a symmetric matrix; descending eigenvalues, vectors in columns.
pub(super) fn symmetric_eigen(a: &Matrix) -> Result<(Vec<f64>, Matrix), TaError> {
    let n = a.len();
    let mut a = a.clone();
    let mut vectors = identity(n);
    let scale = a.iter().flatten().map(|v| v.abs()).fold(0.0, f64::max);
    if scale == 0.0 {
        return Ok((vec![0.0; n], vectors));
    }
    for row in &mut a {
        for v in row {
            *v /= scale;
        }
    }
    let mut converged = n == 1;
    for _ in 0..(100 * n * n) {
        let (mut p, mut q, mut largest) = (0, 0, 0.0);
        for (i, row) in a.iter().enumerate() {
            for (j, &v) in row.iter().enumerate().skip(i + 1) {
                if v.abs() > largest {
                    largest = v.abs();
                    p = i;
                    q = j;
                }
            }
        }
        if largest < 1e-13 {
            converged = true;
            break;
        }
        let angle = 0.5 * (2.0 * a[p][q]).atan2(a[q][q] - a[p][p]);
        let (c, s) = (angle.cos(), angle.sin());
        let (app, aqq, apq) = (a[p][p], a[q][q], a[p][q]);
        for k in 0..n {
            if k != p && k != q {
                let (akp, akq) = (a[k][p], a[k][q]);
                a[k][p] = c * akp - s * akq;
                a[p][k] = a[k][p];
                a[k][q] = s * akp + c * akq;
                a[q][k] = a[k][q];
            }
            let (vkp, vkq) = (vectors[k][p], vectors[k][q]);
            vectors[k][p] = c * vkp - s * vkq;
            vectors[k][q] = s * vkp + c * vkq;
        }
        a[p][p] = c * c * app - 2.0 * s * c * apq + s * s * aqq;
        a[q][q] = s * s * app + 2.0 * s * c * apq + c * c * aqq;
        a[p][q] = 0.0;
        a[q][p] = 0.0;
    }
    if !converged {
        return Err(failure("symmetric eigenvalue iteration did not converge"));
    }
    let mut order: Vec<_> = (0..n).collect();
    order.sort_by(|&i, &j| a[j][j].total_cmp(&a[i][i]));
    let values = order.iter().map(|&i| a[i][i] * scale).collect();
    let vectors = (0..n)
        .map(|i| order.iter().map(|&j| vectors[i][j]).collect())
        .collect();
    Ok((values, vectors))
}
pub(super) fn covariance(rows: &Matrix, ddof: usize) -> (Vec<f64>, Matrix) {
    let n = rows.len();
    let p = rows[0].len();
    let means: Vec<_> = (0..p)
        .map(|j| rows.iter().map(|r| r[j]).sum::<f64>() / n as f64)
        .collect();
    let cov = (0..p)
        .map(|i| {
            (0..p)
                .map(|j| {
                    rows.iter()
                        .map(|r| (r[i] - means[i]) * (r[j] - means[j]))
                        .sum::<f64>()
                        / (n - ddof) as f64
                })
                .collect()
        })
        .collect();
    (means, cov)
}
