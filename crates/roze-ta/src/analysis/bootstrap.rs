//! Explicit IID or non-circular moving-block resampling of the sample mean.
use super::*;
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
pub const VERSION: &str = "roze-ta-bootstrap-mean-v1/chacha8-rand_chacha-0.9.0/rand-0.9.5";

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(tag = "scheme", rename_all = "snake_case", deny_unknown_fields)]
pub enum BootstrapScheme {
    Iid {
        assume_independent: bool,
    },
    MovingBlock {
        block_length: usize,
        assume_stationary: bool,
    },
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct BootstrapSpec {
    pub scheme: BootstrapScheme,
    pub seed: u64,
    pub replicates: usize,
    pub interval_level: f64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct BootstrapResult {
    pub method_version: String,
    pub rng_algorithm: String,
    pub spec: BootstrapSpec,
    pub sample_count: usize,
    pub completed_replicates: usize,
    pub original_mean: f64,
    pub bootstrap_mean: f64,
    pub estimated_bias: f64,
    pub standard_error: f64,
    pub monte_carlo_standard_error_of_bootstrap_mean: f64,
    pub interval: Interval,
    pub replicate_means: Vec<f64>,
    pub assumptions: Vec<String>,
}
impl BootstrapSpec {
    pub(super) fn validate(&self, request: &Request, points: &[Point]) -> Result<(), TaError> {
        level(self.interval_level)?;
        if !(2..=4096).contains(&self.replicates) {
            return Err(err(
                ErrorCode::LimitExceeded,
                "bootstrap replicates must be 2..4096",
            ));
        }
        if points.len() < 2 {
            return Err(err(
                ErrorCode::InsufficientData,
                "bootstrap requires at least two selected observations",
            ));
        }
        match self.scheme {
            BootstrapScheme::Iid {
                assume_independent: true,
            } => {}
            BootstrapScheme::MovingBlock {
                block_length,
                assume_stationary: true,
            } => {
                if block_length == 0 || block_length > points.len() {
                    return Err(err(
                        ErrorCode::InvalidParameter,
                        "block length must be 1..selected sample count",
                    ));
                }
                if request
                    .points
                    .iter()
                    .take_while(|p| p.available_at_ms <= request.fit_cutoff_ms)
                    .count()
                    != points.len()
                {
                    return Err(err(ErrorCode::InvalidTime,"moving blocks require an available prefix without omitted interior observations"));
                }
            }
            _ => {
                return Err(err(
                    ErrorCode::InvalidParameter,
                    "explicit IID independence or block stationarity assumption required",
                ))
            }
        }
        Ok(())
    }
}
pub(super) fn calculate(
    spec: &BootstrapSpec,
    points: &[Point],
    checkpoint: &mut impl FnMut() -> Result<(), TaError>,
) -> Result<BootstrapResult, TaError> {
    let n = points.len();
    let original = points.iter().map(|p| p.x).sum::<f64>() / n as f64;
    let block = match spec.scheme {
        BootstrapScheme::Iid { .. } => 1,
        BootstrapScheme::MovingBlock { block_length, .. } => block_length,
    };
    let mut rng = ChaCha8Rng::seed_from_u64(spec.seed);
    let mut means = Vec::with_capacity(spec.replicates);
    for _ in 0..spec.replicates {
        checkpoint()?;
        let (mut sum, mut count) = (0.0, 0);
        while count < n {
            checkpoint()?;
            // u64 range fixes index sampling across 32/64-bit usize widths.
            let start = rng.random_range(0..=(n - block) as u64) as usize;
            let take = block.min(n - count);
            for p in &points[start..start + take] {
                sum += p.x;
            }
            count += take;
        }
        means.push(sum / n as f64);
    }
    let average = means.iter().sum::<f64>() / means.len() as f64;
    let se = (means.iter().map(|x| (x - average).powi(2)).sum::<f64>() / (means.len() - 1) as f64)
        .sqrt();
    let sorted = statistics::sorted(means.iter().copied());
    let alpha = (1.0 - spec.interval_level) / 2.0;
    Ok(BootstrapResult{method_version:VERSION.into(),rng_algorithm:"ChaCha8Rng::seed_from_u64 + rand-0.9.5 uniform u64 indices".into(),spec:spec.clone(),sample_count:n,completed_replicates:means.len(),original_mean:original,bootstrap_mean:average,estimated_bias:average-original,standard_error:se,monte_carlo_standard_error_of_bootstrap_mean:se/(means.len() as f64).sqrt(),interval:Interval{interval_type:"frequentist_bootstrap_percentile".into(),method:"type_7_quantiles_of_resampled_means".into(),level:spec.interval_level,lower:statistics::quantile(&sorted,alpha),upper:statistics::quantile(&sorted,1.0-alpha),assumptions:vec!["percentile interval; no BCa/studentization or finite-sample coverage guarantee".into()]},replicate_means:means,
    assumptions:vec!["IID explicitly assumes independent observations; moving blocks assume stationary consecutive observations supplied by caller".into(),"moving blocks: uniform start among n-L+1 overlapping non-circular blocks, concatenate and truncate final block to length n".into(),"bootstrap standard error estimates sampling uncertainty; Monte Carlo standard error describes simulation mean precision, not confidence-interval endpoint error".into(),"L=n or constant data yields a degenerate resampling distribution; not proof of zero real-world uncertainty".into(),"same request/seed/version is reproducible on verified platform; no cross-platform floating-point promise".into()]})
}
