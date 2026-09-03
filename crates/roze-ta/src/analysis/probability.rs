use super::*;
use statrs::distribution::{Beta, ContinuousCDF, Normal};
use std::collections::BTreeSet;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct Event {
    pub id: String,
    /// Half-open label interval [start,end); labels must be mature before fitting.
    pub start_ms: i64,
    pub end_ms: i64,
    pub available_at_ms: i64,
    pub condition_known_at_ms: i64,
    pub condition_matches: bool,
    pub hit: Option<bool>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct BetaPrior {
    pub alpha: f64,
    pub beta: f64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum BernoulliAssumption {
    IidAfterNonoverlapSelection,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ProbabilityTask {
    pub event_definition: String,
    pub conditioning: String,
    pub horizon_ms: i64,
    pub label_definition_version: String,
    pub interval_level: f64,
    pub prior: BetaPrior,
    pub assumption: BernoulliAssumption,
}
impl ProbabilityTask {
    pub(super) fn validate(&self) -> Result<(), TaError> {
        text_field(&self.event_definition)?;
        text_field(&self.conditioning)?;
        text_field(&self.label_definition_version)?;
        level(self.interval_level)?;
        if self.horizon_ms <= 0
            || [self.prior.alpha, self.prior.beta]
                .iter()
                .any(|x| !x.is_finite() || !(0.05..=10_000.0).contains(x))
        {
            return Err(err(
                ErrorCode::InvalidParameter,
                "positive horizon required; Beta prior shapes must be 0.05..10000",
            ));
        }
        Ok(())
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct BetaArtifact {
    pub artifact_version: u32,
    pub method_version: String,
    pub input_schema: String,
    pub identity: SeriesIdentity,
    pub definition: ProbabilityTask,
    pub fit_cutoff_ms: i64,
    pub training_range_ms: [i64; 2],
    pub input_hash: String,
    pub selection_hash: String,
    pub sample_count: usize,
    pub hits: usize,
    pub posterior: BetaPrior,
    pub artifact_hash: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct BetaInference {
    pub as_of_ms: i64,
    pub artifact_hash: String,
    /// Posterior predictive mean for a single exchangeable Bernoulli event.
    pub probability: f64,
    /// Equal-tail credible interval for the latent Bernoulli parameter, not the next label.
    pub parameter_credible_interval: Interval,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct ProbabilityResult {
    pub method: String,
    pub definition: ProbabilityTask,
    pub supplied_events: usize,
    pub excluded_immature: usize,
    pub excluded_condition: usize,
    pub excluded_overlap: usize,
    pub selected_ids: Vec<String>,
    pub sample_count: usize,
    pub effective_sample_count: Option<f64>,
    pub effective_sample_method: String,
    pub hits: usize,
    pub empirical_probability: f64,
    pub confidence_interval: Interval,
    pub artifact: BetaArtifact,
    pub inference: BetaInference,
}
pub(super) fn validate_events(events: &[Event]) -> Result<(), TaError> {
    let mut ids = BTreeSet::new();
    for event in events {
        text_field(&event.id)?;
        if !ids.insert(&event.id) {
            return Err(err(
                ErrorCode::DuplicateSample,
                "duplicate event ID; samples cannot be consumed twice",
            ));
        }
        if event.start_ms <= 0
            || event.end_ms <= event.start_ms
            || event.available_at_ms < event.end_ms
            || event.condition_known_at_ms <= 0
            || event.condition_known_at_ms > event.start_ms
        {
            return Err(err(
                ErrorCode::InvalidTime,
                "events require 0 < condition_known <= start < end <= available",
            ));
        }
    }
    Ok(())
}
pub(super) fn calculate(
    task: &ProbabilityTask,
    request: &Request,
    input_hash: &str,
    checkpoint: &mut impl FnMut() -> Result<(), TaError>,
) -> Result<ProbabilityResult, TaError> {
    let mut candidates: Vec<_> = request.events.iter().collect();
    candidates.sort_by(|a, b| (a.start_ms, a.end_ms, &a.id).cmp(&(b.start_ms, b.end_ms, &b.id)));
    let mut selected = Vec::new();
    let (mut immature, mut condition, mut overlap, mut last_end, mut hits) = (0, 0, 0, 0, 0);
    for event in candidates {
        checkpoint()?;
        if event.end_ms - event.start_ms != task.horizon_ms {
            return Err(err(
                ErrorCode::InvalidSample,
                "event interval does not match explicit horizon_ms",
            ));
        }
        if event.available_at_ms > request.fit_cutoff_ms {
            immature += 1;
            continue;
        }
        if !event.condition_matches {
            condition += 1;
            continue;
        }
        let hit = event.hit.ok_or_else(|| {
            err(
                ErrorCode::InvalidSample,
                "mature qualifying event has no label",
            )
        })?;
        if event.start_ms < last_end {
            overlap += 1;
            continue;
        }
        hits += usize::from(hit);
        last_end = event.end_ms;
        selected.push(event);
    }
    let n = selected.len();
    let Some((first, last)) = selected.first().zip(selected.last()) else {
        return Err(err(
            ErrorCode::InsufficientData,
            "no mature, qualifying nonoverlapping events",
        ));
    };
    let p = hits as f64 / n as f64;
    let normal = Normal::new(0.0, 1.0)
        .map_err(|_| err(ErrorCode::NumericalFailure, "normal construction failed"))?;
    let z = normal.inverse_cdf(0.5 + task.interval_level / 2.0);
    let z2 = z * z;
    let denom = 1.0 + z2 / n as f64;
    let center = (p + z2 / (2.0 * n as f64)) / denom;
    let margin = z / denom * (p * (1.0 - p) / n as f64 + z2 / (4.0 * (n * n) as f64)).sqrt();
    let interval = Interval {
        interval_type: "frequentist_confidence".into(),
        method: "wilson_score".into(),
        level: task.interval_level,
        lower: (center - margin).max(0.0),
        upper: (center + margin).min(1.0),
        assumptions: vec![
            "IID Bernoulli after explicit condition and chronological nonoverlap selection".into(),
            "nonoverlap alone does not establish independence; no serial-dependence correction"
                .into(),
        ],
    };
    let mut artifact = BetaArtifact {
        artifact_version: 1,
        method_version: VERSION.into(),
        input_schema: "analysis-v1/mature-event-interval".into(),
        identity: request.identity.clone(),
        definition: task.clone(),
        fit_cutoff_ms: request.fit_cutoff_ms,
        training_range_ms: [first.start_ms, last.end_ms],
        input_hash: input_hash.into(),
        selection_hash: fingerprint::digest(&selected)?,
        sample_count: n,
        hits,
        posterior: BetaPrior {
            alpha: task.prior.alpha + hits as f64,
            beta: task.prior.beta + (n - hits) as f64,
        },
        artifact_hash: String::new(),
    };
    artifact.artifact_hash = fingerprint::digest(&artifact)?;
    let inference = infer_beta(&artifact, request.as_of_ms)?;
    Ok(ProbabilityResult {
        method: "empirical_wilson_beta_binomial".into(),
        definition: task.clone(),
        supplied_events: request.events.len(),
        excluded_immature: immature,
        excluded_condition: condition,
        excluded_overlap: overlap,
        selected_ids: selected.iter().map(|e| e.id.clone()).collect(),
        sample_count: n,
        effective_sample_count: None,
        effective_sample_method:
            "not_estimated; intervals assume IID and use selected sample count".into(),
        hits,
        empirical_probability: p,
        confidence_interval: interval,
        artifact,
        inference,
    })
}
/// Read an immutable fitted posterior. This never accepts observations or refits.
/// Hashes detect accidental corruption, not malicious edits with recomputed hashes.
pub fn infer_beta(artifact: &BetaArtifact, as_of_ms: i64) -> Result<BetaInference, TaError> {
    artifact.definition.validate()?;
    artifact.identity.validate()?;
    let hash_valid = |s: &str| {
        s.len() == 64
            && s.bytes()
                .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
    };
    if artifact.artifact_version != 1
        || artifact.method_version != VERSION
        || artifact.input_schema != "analysis-v1/mature-event-interval"
        || artifact.sample_count == 0
        || artifact.sample_count > MAX_SAMPLES
        || artifact.hits > artifact.sample_count
        || artifact.training_range_ms[0] <= 0
        || artifact.training_range_ms[1] <= artifact.training_range_ms[0]
        || artifact.training_range_ms[1] > artifact.fit_cutoff_ms
        || !hash_valid(&artifact.input_hash)
        || !hash_valid(&artifact.selection_hash)
        || !hash_valid(&artifact.artifact_hash)
        || artifact.posterior.alpha != artifact.definition.prior.alpha + artifact.hits as f64
        || artifact.posterior.beta
            != artifact.definition.prior.beta + (artifact.sample_count - artifact.hits) as f64
    {
        return Err(err(
            ErrorCode::IncompatibleArtifact,
            "invalid Beta artifact version, counts, parameters or provenance",
        ));
    }
    if as_of_ms < artifact.fit_cutoff_ms {
        return Err(err(
            ErrorCode::InvalidTime,
            "inference as_of precedes artifact fit_cutoff",
        ));
    }
    let mut canonical = artifact.clone();
    canonical.artifact_hash.clear();
    if fingerprint::digest(&canonical)? != artifact.artifact_hash {
        return Err(err(
            ErrorCode::IncompatibleArtifact,
            "Beta artifact checksum mismatch",
        ));
    }
    let beta = Beta::new(artifact.posterior.alpha, artifact.posterior.beta)
        .map_err(|_| err(ErrorCode::NumericalFailure, "Beta construction failed"))?;
    let tail = (1.0 - artifact.definition.interval_level) / 2.0;
    let lower = distribution::bounded_quantile(|x| beta.cdf(x), tail, 0.0, 1.0, &mut || Ok(()))?;
    let upper =
        distribution::bounded_quantile(|x| beta.cdf(x), 1.0 - tail, 0.0, 1.0, &mut || Ok(()))?;
    if !lower.is_finite() || !upper.is_finite() || lower > upper || lower < 0.0 || upper > 1.0 {
        return Err(err(
            ErrorCode::NumericalFailure,
            "invalid Beta credible interval",
        ));
    }
    Ok(BetaInference{as_of_ms,artifact_hash:artifact.artifact_hash.clone(),probability:artifact.posterior.alpha/(artifact.posterior.alpha+artifact.posterior.beta),
        parameter_credible_interval:Interval{interval_type:"bayesian_credible".into(),method:"beta_equal_tail_parameter".into(),level:artifact.definition.interval_level,lower,upper,
            assumptions:vec!["explicit Beta prior; exchangeable IID Bernoulli labels under unchanged event definition".into(),"parameter uncertainty; excludes model misspecification and temporal drift".into()]}})
}
