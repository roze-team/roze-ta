use roze_ta::{
    analysis::{bootstrap::*, calibration::*, temporal::*, *},
    error::{ErrorCode, TaError},
};
fn request() -> Request {
    serde_json::from_str(include_str!(
        "../../../docs/usage/validation-request-v1.json"
    ))
    .unwrap()
}
fn near(a: f64, b: f64) {
    assert!((a - b).abs() <= 1e-10 * (1.0 + b.abs()), "{a} != {b}");
}
fn val(s: &Scalar) -> f64 {
    if let Scalar::Ready { value } = s {
        *value
    } else {
        panic!("{s:?}")
    }
}
fn calibration_spec() -> CalibrationSpec {
    let Operation::CalibrationEvaluation { spec } = request().operations.remove(0) else {
        panic!()
    };
    spec
}
fn temporal_spec() -> TemporalSpec {
    let Operation::TemporalSplit { spec } = request().operations.remove(2) else {
        panic!()
    };
    spec
}
fn one(op: Operation) -> Request {
    let mut r = request();
    r.operations = vec![op];
    r
}
fn cal(spec: CalibrationSpec) -> CalibrationResult {
    let Output::CalibrationEvaluation(r) =
        calculate(&one(Operation::CalibrationEvaluation { spec }))
            .unwrap()
            .results
            .remove(0)
    else {
        panic!()
    };
    *r
}
fn boot(spec: BootstrapSpec) -> BootstrapResult {
    let Output::BootstrapMean(r) = calculate(&one(Operation::BootstrapMean { spec }))
        .unwrap()
        .results
        .remove(0)
    else {
        panic!()
    };
    *r
}
fn boot_spec() -> BootstrapSpec {
    BootstrapSpec {
        scheme: BootstrapScheme::Iid {
            assume_independent: true,
        },
        seed: 42,
        replicates: 4096,
        interval_level: 0.95,
    }
}

#[test]
fn probability_scores_match_manual_values_and_baseline() {
    let r = cal(calibration_spec());
    near(r.model.brier_score, 0.175);
    near(r.baseline.brier_score, 0.25);
    near(val(&r.brier_skill_score), 0.3);
    near(val(&r.model.log_loss), -(0.25_f64 * 0.5 * 0.75).ln() / 5.0);
    near(val(&r.baseline.log_loss), 2_f64.ln());
    near(r.expected_calibration_error, 0.2);
    near(r.maximum_calibration_error, 0.375);
    assert_eq!(r.sample_count, 5);
    assert_eq!(r.hits, 3);
    assert_eq!(r.bins[0].count, 2);
    assert_eq!(r.bins[1].count, 3);
    near(val(&r.bins[0].mean_probability), 0.125);
    near(val(&r.bins[1].observed_frequency), 2.0 / 3.0);
    assert_eq!(r.model.clipped_probabilities, 0);
    assert_eq!(r.available_at_ms, 55);
}

#[test]
fn log_loss_endpoint_impossibility_and_clipping_are_explicit() {
    let mut s = calibration_spec();
    s.forecasts[0].outcome = Some(true);
    s.forecasts[4].outcome = Some(false);
    let r = cal(s.clone());
    assert!(matches!(r.model.log_loss, Scalar::Undefined { .. }));
    assert_eq!(r.model.impossible_outcomes, 2);
    s.log_loss_clip = Some(0.1);
    let clipped = cal(s);
    near(clipped.model.brier_score, r.model.brier_score);
    near(
        clipped.expected_calibration_error,
        r.expected_calibration_error,
    );
    near(
        val(&clipped.model.log_loss),
        -(0.1_f64 * 0.25 * 0.5 * 0.75 * 0.1).ln() / 5.0,
    );
    assert_eq!(clipped.model.clipped_probabilities, 2);
    assert_eq!(clipped.model.impossible_outcomes, 2);
    let mut s = calibration_spec();
    s.baseline_probability = 0.;
    for f in &mut s.forecasts {
        f.outcome = Some(false);
    }
    assert!(matches!(cal(s).brier_skill_score, Scalar::Undefined { .. }));
}

#[test]
fn empty_reliability_bins_and_exact_edges_are_not_filled_with_zero() {
    let mut s = calibration_spec();
    s.bin_edges = vec![0., 0.1, 0.2, 0.5, 1.];
    let r = cal(s);
    assert_eq!(r.bins[1].count, 0);
    assert_eq!(r.bins[1].mean_probability, Scalar::InsufficientData);
    assert_eq!(r.bins[3].count, 3);
    assert!(r.bins[3].upper_inclusive);
}

#[test]
fn calibration_excludes_immature_and_overlap_and_replays_cutoff() {
    let mut s = calibration_spec();
    s.forecasts[1].at_ms = 12;
    s.forecasts[1].prediction_available_at_ms = 12;
    s.forecasts[1].label_end_ms = 17;
    s.forecasts[1].label_available_at_ms = 17;
    s.forecasts[4].label_available_at_ms = 150;
    s.forecasts[4].outcome = None;
    let a = cal(s.clone());
    assert_eq!(a.excluded_overlap, 1);
    assert_eq!(a.excluded_immature, 1);
    assert_eq!(a.sample_count, 3);
    s.forecasts[4].outcome = Some(false);
    s.forecasts[4].probability = 0.2;
    let b = cal(s);
    assert_eq!(a.selection_hash, b.selection_hash);
    near(a.model.brier_score, b.model.brier_score);
    let s = calibration_spec();
    let mut r = one(Operation::CalibrationEvaluation { spec: s.clone() });
    r.fit_cutoff_ms = 35;
    let Output::CalibrationEvaluation(a) = calculate(&r).unwrap().results.remove(0) else {
        panic!()
    };
    let mut truncated = s;
    truncated.forecasts.truncate(3);
    let b = cal(truncated);
    assert_eq!(a.selection_hash, b.selection_hash);
    near(a.model.brier_score, b.model.brier_score);
}

#[test]
fn calibration_rejects_leakage_duplicates_invalid_probabilities_and_missing_labels() {
    let mut cases = Vec::new();
    let mut s = calibration_spec();
    s.model_fit_cutoff_ms = 10;
    cases.push((s, ErrorCode::InvalidTime));
    let mut s = calibration_spec();
    s.baseline_fit_cutoff_ms = 10;
    cases.push((s, ErrorCode::InvalidTime));
    let mut s = calibration_spec();
    s.forecasts[0].prediction_available_at_ms = 11;
    cases.push((s, ErrorCode::InvalidTime));
    let mut s = calibration_spec();
    s.forecasts[0].outcome = None;
    cases.push((s, ErrorCode::InvalidSample));
    let mut s = calibration_spec();
    s.forecasts[1].id = s.forecasts[0].id.clone();
    cases.push((s, ErrorCode::DuplicateSample));
    let mut s = calibration_spec();
    s.forecasts[0].probability = 1.1;
    cases.push((s, ErrorCode::InvalidParameter));
    let mut s = calibration_spec();
    s.bin_edges = vec![0., 0.5, 0.5, 1.];
    cases.push((s, ErrorCode::InvalidParameter));
    let mut s = calibration_spec();
    s.log_loss_clip = Some(f64::MIN_POSITIVE);
    cases.push((s, ErrorCode::InvalidParameter));
    for (spec, expected) in cases {
        assert_eq!(
            calculate(&one(Operation::CalibrationEvaluation { spec }))
                .unwrap_err()
                .code,
            expected
        );
    }
}

#[test]
fn iid_bootstrap_agrees_with_exhaustive_two_point_distribution() {
    // All n=2 draws from [1,3]: means [1,2,2,3], E=2, variance=0.5.
    let r = boot(boot_spec());
    near(r.original_mean, 2.);
    assert_eq!(r.completed_replicates, 4096);
    assert!((r.bootstrap_mean - 2.).abs() < 0.06);
    assert!((r.standard_error - 0.5_f64.sqrt()).abs() < 0.06);
    assert!(r.replicate_means.iter().all(|x| [1., 2., 3.].contains(x)));
    near(r.interval.lower, 1.);
    near(r.interval.upper, 3.);
    near(
        r.monte_carlo_standard_error_of_bootstrap_mean,
        r.standard_error / 64.,
    );
    let rerun = boot(boot_spec());
    assert_eq!(r.replicate_means, rerun.replicate_means);
    let mut different = boot_spec();
    different.seed = 43;
    assert_ne!(r.replicate_means, boot(different).replicate_means);
}

#[test]
fn moving_blocks_preserve_order_truncate_and_have_defined_degeneracy() {
    let mut s = boot_spec();
    s.scheme = BootstrapScheme::MovingBlock {
        block_length: 2,
        assume_stationary: true,
    };
    let r = boot(s.clone());
    assert!(r.replicate_means.iter().all(|x| *x == 2.));
    near(r.standard_error, 0.);
    near(r.interval.lower, 2.);
    let mut req = one(Operation::BootstrapMean { spec: s.clone() });
    req.points = vec![1., 2., 10.]
        .into_iter()
        .enumerate()
        .map(|(i, x)| Point {
            at_ms: i as i64 + 1,
            available_at_ms: i as i64 + 1,
            x,
            y: None,
        })
        .collect();
    let Output::BootstrapMean(r) = calculate(&req).unwrap().results.remove(0) else {
        panic!()
    };
    // Ordered blocks [1,2] or [2,10], then one item [1] or [2].
    let possible = [4. / 3., 5. / 3., 13. / 3., 14. / 3.];
    assert!(r.replicate_means.iter().all(|x| possible.contains(x)));
    let mut iid = boot_spec();
    iid.replicates = 100;
    s.replicates = 100;
    s.scheme = BootstrapScheme::MovingBlock {
        block_length: 1,
        assume_stationary: true,
    };
    assert_eq!(boot(iid).replicate_means, boot(s).replicate_means);
}

#[test]
fn bootstrap_time_gaps_short_samples_and_assumptions_are_checked() {
    let mut s = boot_spec();
    s.scheme = BootstrapScheme::Iid {
        assume_independent: false,
    };
    assert_eq!(
        calculate(&one(Operation::BootstrapMean { spec: s.clone() }))
            .unwrap_err()
            .code,
        ErrorCode::InvalidParameter
    );
    s.scheme = BootstrapScheme::MovingBlock {
        block_length: 3,
        assume_stationary: true,
    };
    assert_eq!(
        calculate(&one(Operation::BootstrapMean { spec: s.clone() }))
            .unwrap_err()
            .code,
        ErrorCode::InvalidParameter
    );
    s.scheme = BootstrapScheme::MovingBlock {
        block_length: 1,
        assume_stationary: true,
    };
    let mut req = one(Operation::BootstrapMean { spec: s });
    req.points.push(Point {
        at_ms: 3,
        available_at_ms: 3,
        x: 4.,
        y: None,
    });
    req.points[1].available_at_ms = 101;
    assert_eq!(calculate(&req).unwrap_err().code, ErrorCode::InvalidTime);
    req.points.truncate(1);
    assert_eq!(
        calculate(&req).unwrap_err().code,
        ErrorCode::InsufficientData
    );
}

#[test]
fn temporal_partitions_purge_actual_intervals_and_delayed_labels() {
    let result = calculate(&one(Operation::TemporalSplit {
        spec: temporal_spec(),
    }))
    .unwrap();
    let Output::TemporalSplit(r) = &result.results[0] else {
        panic!()
    };
    let f = &r.folds[0];
    assert!(f.usable);
    assert_eq!(f.excluded_boundary, 2);
    assert_eq!(f.train.selected_ids, vec!["train"]);
    assert_eq!(f.calibration.selected_ids, vec!["calibration"]);
    assert_eq!(f.validation.selected_ids, vec!["validation"]);
    assert_eq!(f.train.labels_known_before_ms, 18);
    assert_eq!(f.calibration.labels_known_before_ms, 38);
    let mut s = temporal_spec();
    s.observations[0].label_end_ms = 18;
    s.observations[0].label_available_at_ms = 18;
    let Output::TemporalSplit(r) = calculate(&one(Operation::TemporalSplit { spec: s }))
        .unwrap()
        .results
        .remove(0)
    else {
        panic!()
    };
    assert!(!r.folds[0].usable);
    assert!(r.folds[0].train.selected_ids.is_empty());
}

#[test]
fn temporal_selection_is_order_independent_and_supports_expanding_and_rolling() {
    let mut s = temporal_spec();
    s.folds.push(FoldWindow {
        train_start_ms: 1,
        train_end_ms: 30,
        calibration_end_ms: 50,
        validation_end_ms: 70,
    });
    s.folds.push(FoldWindow {
        train_start_ms: 20,
        train_end_ms: 40,
        calibration_end_ms: 60,
        validation_end_ms: 80,
    });
    let Output::TemporalSplit(a) = calculate(&one(Operation::TemporalSplit { spec: s.clone() }))
        .unwrap()
        .results
        .remove(0)
    else {
        panic!()
    };
    s.observations.reverse();
    let Output::TemporalSplit(b) = calculate(&one(Operation::TemporalSplit { spec: s }))
        .unwrap()
        .results
        .remove(0)
    else {
        panic!()
    };
    assert_eq!(
        serde_json::to_value(&a.folds).unwrap(),
        serde_json::to_value(&b.folds).unwrap()
    );
    assert!(a.folds[1]
        .train
        .selected_ids
        .contains(&"calibration".into()));
    assert!(!a.folds[2].train.selected_ids.contains(&"train".into()));
}

#[test]
fn temporal_unknown_labels_and_invalid_windows_fail_explicitly() {
    let mut r = one(Operation::TemporalSplit {
        spec: temporal_spec(),
    });
    r.fit_cutoff_ms = 40;
    let Output::TemporalSplit(result) = calculate(&r).unwrap().results.remove(0) else {
        panic!()
    };
    assert!(!result.folds[0].usable);
    assert_eq!(result.folds[0].excluded_unavailable, 1);
    let mut s = temporal_spec();
    s.purge_gap_ms = 19;
    assert_eq!(
        calculate(&one(Operation::TemporalSplit { spec: s }))
            .unwrap_err()
            .code,
        ErrorCode::InvalidTime
    );
    let mut s = temporal_spec();
    s.observations.push(s.observations[0].clone());
    assert_eq!(
        calculate(&one(Operation::TemporalSplit { spec: s }))
            .unwrap_err()
            .code,
        ErrorCode::DuplicateSample
    );
    let mut s = temporal_spec();
    s.observations[0].features_available_at_ms = 3;
    assert_eq!(
        calculate(&one(Operation::TemporalSplit { spec: s }))
            .unwrap_err()
            .code,
        ErrorCode::InvalidTime
    );
}

#[test]
fn stochastic_budget_and_in_loop_cancellation_are_enforced() {
    let mut req = one(Operation::BootstrapMean { spec: boot_spec() });
    req.points = (1..=1000)
        .map(|i| Point {
            at_ms: i,
            available_at_ms: i,
            x: 1.,
            y: None,
        })
        .collect();
    req.as_of_ms = 2000;
    req.fit_cutoff_ms = 2000;
    assert_eq!(calculate(&req).unwrap_err().code, ErrorCode::LimitExceeded);
    let req = one(Operation::BootstrapMean { spec: boot_spec() });
    let mut checks = 0;
    let error = calculate_controlled(&req, || {
        checks += 1;
        if checks == 20 {
            Err(TaError::new(ErrorCode::Cancelled, "test"))
        } else {
            Ok(())
        }
    })
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::Cancelled);
    assert_eq!(checks, 20);
}
