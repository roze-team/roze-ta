//! Explicit chronological train/calibration/validation partitions with label purging.
use super::*;
use std::collections::BTreeSet;
pub const VERSION: &str = "roze-ta-temporal-split-v1";

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct TimedLabel {
    pub id: String,
    pub at_ms: i64,
    pub features_available_at_ms: i64,
    pub label_end_ms: i64,
    pub label_available_at_ms: i64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct FoldWindow {
    pub train_start_ms: i64,
    pub train_end_ms: i64,
    pub calibration_end_ms: i64,
    pub validation_end_ms: i64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct TemporalSpec {
    pub dataset_id: String,
    pub label_definition: String,
    pub purge_gap_ms: i64,
    pub minimum_partition_samples: usize,
    pub folds: Vec<FoldWindow>,
    pub observations: Vec<TimedLabel>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct Partition {
    pub feature_window_ms: [i64; 2],
    pub labels_known_before_ms: i64,
    pub selected_ids: Vec<String>,
    pub selection_hash: String,
    pub sufficient_samples: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct FoldResult {
    pub window: FoldWindow,
    pub train: Partition,
    pub calibration: Partition,
    pub validation: Partition,
    pub excluded_outside: usize,
    pub excluded_unavailable: usize,
    pub excluded_boundary: usize,
    pub usable: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct TemporalResult {
    pub method_version: String,
    pub dataset_id: String,
    pub label_definition: String,
    pub purge_gap_ms: i64,
    pub folds: Vec<FoldResult>,
    pub assumptions: Vec<String>,
}
impl TemporalSpec {
    pub(super) fn validate(&self) -> Result<(), TaError> {
        text_field(&self.dataset_id)?;
        text_field(&self.label_definition)?;
        if self.purge_gap_ms < 0
            || self.minimum_partition_samples == 0
            || self.minimum_partition_samples > MAX_SAMPLES
        {
            return Err(err(
                ErrorCode::InvalidParameter,
                "nonnegative purge gap and minimum samples 1..4096 required",
            ));
        }
        if self.folds.is_empty()
            || self.folds.len() > 32
            || self.observations.is_empty()
            || self.observations.len() > MAX_SAMPLES
        {
            return Err(err(
                ErrorCode::LimitExceeded,
                "1..32 folds and 1..4096 timed labels required",
            ));
        }
        for f in &self.folds {
            if f.train_start_ms <= 0
                || f.train_start_ms >= f.train_end_ms
                || f.train_end_ms >= f.calibration_end_ms
                || f.calibration_end_ms >= f.validation_end_ms
                || f.train_end_ms - f.train_start_ms <= self.purge_gap_ms
                || f.calibration_end_ms - f.train_end_ms <= self.purge_gap_ms
            {
                return Err(err(ErrorCode::InvalidTime,"ordered positive fold boundaries required; gap must fit each training/calibration window"));
            }
        }
        if self.folds.windows(2).any(|w| {
            w[1].train_start_ms < w[0].train_start_ms
                || w[1].train_end_ms <= w[0].train_end_ms
                || w[1].calibration_end_ms <= w[0].calibration_end_ms
                || w[1].validation_end_ms <= w[0].validation_end_ms
        }) {
            return Err(err(
                ErrorCode::InvalidTime,
                "fold boundaries must advance chronologically; training start may remain fixed",
            ));
        }
        let mut ids = BTreeSet::new();
        for o in &self.observations {
            text_field(&o.id)?;
            if !ids.insert(&o.id) {
                return Err(err(ErrorCode::DuplicateSample, "duplicate timed label ID"));
            }
            if o.at_ms <= 0
                || o.features_available_at_ms <= 0
                || o.features_available_at_ms > o.at_ms
                || o.label_end_ms <= o.at_ms
                || o.label_available_at_ms < o.label_end_ms
            {
                return Err(err(ErrorCode::InvalidTime,"features must be known by event start; labels must end later and become available after end"));
            }
        }
        Ok(())
    }
}
fn partition(
    rows: &[&TimedLabel],
    start: i64,
    end: i64,
    known_before: i64,
    minimum: usize,
) -> Result<Partition, TaError> {
    Ok(Partition {
        feature_window_ms: [start, end],
        labels_known_before_ms: known_before,
        selected_ids: rows.iter().map(|r| r.id.clone()).collect(),
        selection_hash: fingerprint::digest(&rows)?,
        sufficient_samples: rows.len() >= minimum,
    })
}
pub(super) fn calculate(
    spec: &TemporalSpec,
    cutoff: i64,
    checkpoint: &mut impl FnMut() -> Result<(), TaError>,
) -> Result<TemporalResult, TaError> {
    let mut ordered: Vec<_> = spec.observations.iter().collect();
    ordered.sort_by(|a, b| (a.at_ms, &a.id).cmp(&(b.at_ms, &b.id)));
    let mut folds = Vec::with_capacity(spec.folds.len());
    for f in &spec.folds {
        checkpoint()?;
        let (mut train, mut calibration, mut validation) = (Vec::new(), Vec::new(), Vec::new());
        let (mut outside, mut unavailable, mut boundary) = (0, 0, 0);
        for o in &ordered {
            checkpoint()?;
            if o.at_ms < f.train_start_ms || o.at_ms >= f.validation_end_ms {
                outside += 1;
                continue;
            }
            if o.label_available_at_ms > cutoff {
                unavailable += 1;
                continue;
            }
            let (rows, before) = if o.at_ms < f.train_end_ms {
                (&mut train, f.train_end_ms - spec.purge_gap_ms)
            } else if o.at_ms < f.calibration_end_ms {
                (&mut calibration, f.calibration_end_ms - spec.purge_gap_ms)
            } else {
                (&mut validation, f.validation_end_ms)
            };
            // Strict availability boundary prevents same-timestamp fitting/prediction ambiguity.
            if o.label_end_ms > before || o.label_available_at_ms >= before {
                boundary += 1;
                continue;
            }
            rows.push(*o);
        }
        let train = partition(
            &train,
            f.train_start_ms,
            f.train_end_ms,
            f.train_end_ms - spec.purge_gap_ms,
            spec.minimum_partition_samples,
        )?;
        let calibration = partition(
            &calibration,
            f.train_end_ms,
            f.calibration_end_ms,
            f.calibration_end_ms - spec.purge_gap_ms,
            spec.minimum_partition_samples,
        )?;
        let validation = partition(
            &validation,
            f.calibration_end_ms,
            f.validation_end_ms,
            f.validation_end_ms,
            spec.minimum_partition_samples,
        )?;
        let usable = train.sufficient_samples
            && calibration.sufficient_samples
            && validation.sufficient_samples;
        folds.push(FoldResult {
            window: f.clone(),
            train,
            calibration,
            validation,
            excluded_outside: outside,
            excluded_unavailable: unavailable,
            excluded_boundary: boundary,
            usable,
        });
    }
    Ok(TemporalResult{method_version:VERSION.into(),dataset_id:spec.dataset_id.clone(),label_definition:spec.label_definition.clone(),purge_gap_ms:spec.purge_gap_ms,folds,assumptions:vec!["explicit chronological windows support rolling or expanding training; no random split and no fitting".into(),"purge uses actual label end/availability before next role boundary minus the declared gap; not a general combinatorial purged-CV or post-test embargo implementation".into(),"labels within a partition or across different folds may overlap; fold metrics cannot be treated as independent replicates".into(),"IDs and times are caller-provided; feature computation leakage and data provenance are not detectable from metadata".into()]})
}
