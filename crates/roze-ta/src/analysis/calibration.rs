//! Evaluation of frozen binary forecasts; this module does not fit a calibrator.
use super::*;
use std::collections::BTreeSet;

pub const VERSION: &str = "roze-ta-calibration-evaluation-v1";

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct Forecast {
    pub id: String,
    pub at_ms: i64,
    pub prediction_available_at_ms: i64,
    pub label_end_ms: i64,
    pub label_available_at_ms: i64,
    pub probability: f64,
    pub outcome: Option<bool>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct CalibrationSpec {
    pub model_id: String,
    pub model_fit_cutoff_ms: i64,
    pub event_definition: String,
    pub label_definition_version: String,
    pub horizon_ms: i64,
    pub evaluation_start_ms: i64,
    pub evaluation_end_ms: i64,
    pub baseline_probability: f64,
    pub baseline_fit_cutoff_ms: i64,
    pub bin_edges: Vec<f64>,
    pub log_loss_clip: Option<f64>,
    pub forecasts: Vec<Forecast>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct Scores {
    pub brier_score: f64,
    pub log_loss: Scalar,
    pub impossible_outcomes: usize,
    pub clipped_probabilities: usize,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct ReliabilityBin {
    pub lower: f64,
    pub upper: f64,
    pub upper_inclusive: bool,
    pub count: usize,
    pub hits: usize,
    pub mean_probability: Scalar,
    pub observed_frequency: Scalar,
    pub absolute_gap: Scalar,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct CalibrationResult {
    pub method_version: String,
    pub model_id: String,
    pub model_fit_cutoff_ms: i64,
    pub event_definition: String,
    pub label_definition_version: String,
    pub horizon_ms: i64,
    pub evaluation_window_ms: [i64; 2],
    pub baseline_probability: f64,
    pub baseline_fit_cutoff_ms: i64,
    pub log_loss_clip: Option<f64>,
    pub selected_ids: Vec<String>,
    pub selected_range_ms: [i64; 2],
    pub available_at_ms: i64,
    pub selection_hash: String,
    pub excluded_outside_window: usize,
    pub excluded_immature: usize,
    pub excluded_overlap: usize,
    pub sample_count: usize,
    pub hits: usize,
    pub model: Scores,
    pub baseline: Scores,
    pub brier_skill_score: Scalar,
    pub expected_calibration_error: f64,
    pub maximum_calibration_error: f64,
    pub bins: Vec<ReliabilityBin>,
    pub assumptions: Vec<String>,
}

fn probability(p: f64) -> Result<(), TaError> {
    if !p.is_finite() || !(0.0..=1.0).contains(&p) {
        return Err(err(
            ErrorCode::InvalidParameter,
            "probability must be finite in [0,1]",
        ));
    }
    Ok(())
}
impl CalibrationSpec {
    pub(super) fn validate(&self) -> Result<(), TaError> {
        for value in [
            &self.model_id,
            &self.event_definition,
            &self.label_definition_version,
        ] {
            text_field(value)?;
        }
        probability(self.baseline_probability)?;
        if self.horizon_ms <= 0
            || self.evaluation_start_ms <= 0
            || self.evaluation_start_ms >= self.evaluation_end_ms
            || self.model_fit_cutoff_ms <= 0
            || self.baseline_fit_cutoff_ms <= 0
            || self.model_fit_cutoff_ms >= self.evaluation_start_ms
            || self.baseline_fit_cutoff_ms >= self.evaluation_start_ms
        {
            return Err(err(
                ErrorCode::InvalidTime,
                "positive horizon/window; model and baseline fitting must precede evaluation start",
            ));
        }
        if self.bin_edges.len() < 2
            || self.bin_edges.len() > 33
            || self.bin_edges.first() != Some(&0.0)
            || self.bin_edges.last() != Some(&1.0)
        {
            return Err(err(
                ErrorCode::InvalidParameter,
                "provide 2..33 increasing bin edges covering [0,1]",
            ));
        }
        for &p in &self.bin_edges {
            probability(p)?;
        }
        if self.bin_edges.windows(2).any(|p| p[0] >= p[1]) {
            return Err(err(
                ErrorCode::InvalidParameter,
                "bin edges must strictly increase",
            ));
        }
        if self
            .log_loss_clip
            .is_some_and(|e| !e.is_finite() || e <= 0.0 || e >= 0.5 || 1.0 - e == 1.0)
        {
            return Err(err(
                ErrorCode::InvalidParameter,
                "clip must be representable away from endpoints and in (0,0.5)",
            ));
        }
        if self.forecasts.is_empty() || self.forecasts.len() > MAX_SAMPLES {
            return Err(err(
                ErrorCode::LimitExceeded,
                "forecast count must be 1..4096",
            ));
        }
        let mut ids = BTreeSet::new();
        let mut previous = 0;
        for f in &self.forecasts {
            text_field(&f.id)?;
            probability(f.probability)?;
            if !ids.insert(&f.id) {
                return Err(err(ErrorCode::DuplicateSample, "duplicate forecast ID"));
            }
            if f.at_ms <= previous
                || f.prediction_available_at_ms <= 0
                || f.prediction_available_at_ms > f.at_ms
                || f.prediction_available_at_ms <= self.model_fit_cutoff_ms
                || f.at_ms.checked_add(self.horizon_ms) != Some(f.label_end_ms)
                || f.label_available_at_ms < f.label_end_ms
            {
                return Err(err(ErrorCode::InvalidTime,"ordered forecasts must be available after fitting and by prediction time; label horizon must match"));
            }
            previous = f.at_ms;
        }
        Ok(())
    }
}

fn scores(values: impl Iterator<Item = (f64, bool)>, n: usize, clip: Option<f64>) -> Scores {
    let (mut squared, mut loss, mut impossible, mut clipped) = (0.0, 0.0, 0, 0);
    for (raw, hit) in values {
        squared += (raw - f64::from(hit)).powi(2);
        if (hit && raw == 0.0) || (!hit && raw == 1.0) {
            impossible += 1;
        }
        let p = clip.map_or(raw, |e| raw.clamp(e, 1.0 - e));
        clipped += usize::from(p != raw);
        loss += if hit { -p.ln() } else { -(-p).ln_1p() };
    }
    Scores {
        brier_score: squared / n as f64,
        log_loss: if loss.is_finite() {
            Scalar::number(loss / n as f64)
        } else {
            Scalar::undefined("infinite_log_loss")
        },
        impossible_outcomes: impossible,
        clipped_probabilities: clipped,
    }
}

pub(super) fn calculate(
    spec: &CalibrationSpec,
    cutoff: i64,
    checkpoint: &mut impl FnMut() -> Result<(), TaError>,
) -> Result<CalibrationResult, TaError> {
    let (mut outside, mut immature, mut overlap, mut last_end) = (0, 0, 0, 0);
    let mut selected = Vec::new();
    for f in &spec.forecasts {
        checkpoint()?;
        if f.at_ms < spec.evaluation_start_ms || f.at_ms >= spec.evaluation_end_ms {
            outside += 1;
            continue;
        }
        if f.label_available_at_ms > cutoff {
            immature += 1;
            continue;
        }
        if f.outcome.is_none() {
            return Err(err(
                ErrorCode::InvalidSample,
                "mature forecast outcome missing",
            ));
        }
        if f.at_ms < last_end {
            overlap += 1;
            continue;
        }
        last_end = f.label_end_ms;
        selected.push(f);
    }
    let Some((first, last)) = selected.first().zip(selected.last()) else {
        return Err(err(
            ErrorCode::InsufficientData,
            "no mature nonoverlapping forecasts in evaluation window",
        ));
    };
    let n = selected.len();
    let labels: Vec<_> = selected
        .iter()
        .map(|f| {
            f.outcome
                .ok_or_else(|| err(ErrorCode::InvalidSample, "missing outcome"))
        })
        .collect::<Result<_, _>>()?;
    let model = scores(
        selected
            .iter()
            .zip(&labels)
            .map(|(f, &y)| (f.probability, y)),
        n,
        spec.log_loss_clip,
    );
    let baseline = scores(
        labels.iter().map(|&y| (spec.baseline_probability, y)),
        n,
        spec.log_loss_clip,
    );
    let mut bins: Vec<_> = spec
        .bin_edges
        .windows(2)
        .enumerate()
        .map(|(i, p)| ReliabilityBin {
            lower: p[0],
            upper: p[1],
            upper_inclusive: i == spec.bin_edges.len() - 2,
            count: 0,
            hits: 0,
            mean_probability: Scalar::InsufficientData,
            observed_frequency: Scalar::InsufficientData,
            absolute_gap: Scalar::InsufficientData,
        })
        .collect();
    let mut sums = vec![0.0; bins.len()];
    for (f, &hit) in selected.iter().zip(&labels) {
        checkpoint()?;
        let i = spec
            .bin_edges
            .partition_point(|&e| e <= f.probability)
            .saturating_sub(1)
            .min(bins.len() - 1);
        bins[i].count += 1;
        bins[i].hits += usize::from(hit);
        sums[i] += f.probability;
    }
    let (mut ece, mut mce) = (0.0, 0.0_f64);
    for (bin, sum) in bins.iter_mut().zip(sums) {
        if bin.count > 0 {
            let mean = sum / bin.count as f64;
            let observed = bin.hits as f64 / bin.count as f64;
            let gap = (mean - observed).abs();
            bin.mean_probability = Scalar::number(mean);
            bin.observed_frequency = Scalar::number(observed);
            bin.absolute_gap = Scalar::number(gap);
            ece += gap * bin.count as f64 / n as f64;
            mce = mce.max(gap);
        }
    }
    let skill = if baseline.brier_score == 0.0 {
        Scalar::undefined("zero_baseline_brier")
    } else {
        Scalar::number(1.0 - model.brier_score / baseline.brier_score)
    };
    Ok(CalibrationResult{method_version:VERSION.into(),model_id:spec.model_id.clone(),model_fit_cutoff_ms:spec.model_fit_cutoff_ms,event_definition:spec.event_definition.clone(),label_definition_version:spec.label_definition_version.clone(),horizon_ms:spec.horizon_ms,evaluation_window_ms:[spec.evaluation_start_ms,spec.evaluation_end_ms],baseline_probability:spec.baseline_probability,baseline_fit_cutoff_ms:spec.baseline_fit_cutoff_ms,log_loss_clip:spec.log_loss_clip,selected_ids:selected.iter().map(|f|f.id.clone()).collect(),selected_range_ms:[first.at_ms,last.at_ms],available_at_ms:selected.iter().map(|f|f.label_available_at_ms).max().unwrap_or(cutoff),selection_hash:fingerprint::digest(&selected)?,excluded_outside_window:outside,excluded_immature:immature,excluded_overlap:overlap,sample_count:n,hits:labels.iter().filter(|&&y|y).count(),model,baseline,brier_skill_score:skill,expected_calibration_error:ece,maximum_calibration_error:mce,bins,
    assumptions:vec!["frozen binary forecasts; no calibrator is fitted; baseline is supplied from pre-evaluation data".into(),"bin edges are caller-fixed; reliability bins are [lower,upper), final bin includes 1".into(),"Brier/log loss combine calibration and resolution; ECE depends on bins and is not a significance test".into(),"chronological nonoverlapping label intervals selected; availability and model provenance are caller assertions".into()]})
}
