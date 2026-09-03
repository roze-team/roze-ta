use roze_ta::{
    analysis::*,
    engine::SeriesIdentity,
    error::{ErrorCode, TaError},
};

fn near(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-10 * (1.0 + b.abs()), "{a} != {b}");
}
fn value(x: &Scalar) -> f64 {
    match x {
        Scalar::Ready { value } => *value,
        _ => panic!("unexpected {x:?}"),
    }
}
fn request(xs: &[f64], operations: Vec<Operation>) -> Request {
    Request {
        schema_version: 1,
        identity: SeriesIdentity {
            series_id: "fixture".into(),
            instrument: "TEST".into(),
            timeframe: "1ms".into(),
            source: "manual".into(),
            data_version: "1".into(),
        },
        input_kind: "price".into(),
        units: "unit".into(),
        as_of_ms: 10_000,
        fit_cutoff_ms: 10_000,
        points: xs
            .iter()
            .enumerate()
            .map(|(i, &x)| Point {
                at_ms: i as i64 + 1,
                available_at_ms: i as i64 + 1,
                x,
                y: None,
            })
            .collect(),
        events: vec![],
        operations,
    }
}
fn describe() -> Operation {
    Operation::Describe {
        ddof: 1,
        quantiles: vec![0.0, 0.25, 0.5, 0.75, 1.0],
        trim_fraction: 0.2,
        interval_level: 0.95,
    }
}
fn distribution(d: Distribution) -> Operation {
    Operation::Distribution {
        task: DistributionTask {
            distribution: d,
            evaluate_at: vec![0.0, 0.5, 1.0, 2.0],
            quantiles: vec![0.25, 0.5, 0.75],
            sampling: None,
        },
    }
}
fn probability() -> Operation {
    Operation::Probability {
        task: ProbabilityTask {
            event_definition: "terminal return > 0".into(),
            conditioning: "all observations".into(),
            horizon_ms: 2,
            label_definition_version: "close/simple_return/no_cost/v1".into(),
            interval_level: 0.95,
            prior: BetaPrior {
                alpha: 1.0,
                beta: 1.0,
            },
            assumption: BernoulliAssumption::IidAfterNonoverlapSelection,
        },
    }
}
fn events(hits: &[bool]) -> Vec<Event> {
    hits.iter()
        .enumerate()
        .map(|(i, &hit)| {
            let start = i as i64 * 2 + 1;
            Event {
                id: format!("e{i}"),
                start_ms: start,
                end_ms: start + 2,
                available_at_ms: start + 2,
                condition_known_at_ms: start,
                condition_matches: true,
                hit: Some(hit),
            }
        })
        .collect()
}

#[test]
fn descriptive_hand_values_and_robust_semantics() {
    let result = calculate(&request(&[1., 2., 3., 4., 5.], vec![describe()])).unwrap();
    let Output::Description(d) = &result.results[0] else {
        panic!()
    };
    near(d.mean, 3.);
    near(d.median, 3.);
    near(value(&d.variance), 2.5);
    near(value(&d.standard_deviation), 2.5_f64.sqrt());
    near(value(&d.moment_skewness), 0.);
    near(value(&d.excess_moment_kurtosis), -1.3);
    assert_eq!(
        d.quantiles,
        vec![[0., 1.], [0.25, 2.], [0.5, 3.], [0.75, 4.], [1., 5.]]
    );
    near(d.mad_unscaled, 1.);
    near(d.iqr, 2.);
    near(d.trimmed_mean, 3.);
    assert_eq!(d.trimmed_sample_count, 3);
    near(d.winsorized_values[0], 1.8);
    near(d.winsorized_values[4], 4.2);
    assert_eq!(d.winsorized_sample_count, 5);
    // Student t table: df=4, two-sided 95% critical value 2.7764451051977987.
    let ci = d.mean_confidence_interval.as_ref().unwrap();
    near(ci.lower, 3. - 2.776_445_105_197_798_7 * (0.5_f64).sqrt());
    assert_eq!(ci.interval_type, "frequentist_confidence");
    let result = calculate(&request(&[1., 2., 2., 4.], vec![describe()])).unwrap();
    let Output::Description(d) = &result.results[0] else {
        panic!()
    };
    assert_eq!(d.ecdf, vec![[1., 0.25], [2., 0.75], [4., 1.]]);
}

#[test]
fn constant_small_and_invalid_samples_are_distinct() {
    let result = calculate(&request(&[5.], vec![describe()])).unwrap();
    let Output::Description(d) = &result.results[0] else {
        panic!()
    };
    assert_eq!(d.variance, Scalar::InsufficientData);
    assert!(matches!(d.moment_skewness, Scalar::Undefined { .. }));
    assert!(d.mean_confidence_interval.is_none());
    let result = calculate(&request(
        &[5., 5., 5.],
        vec![Operation::RollingZscore {
            window: 2,
            ddof: 1,
            robust: false,
        }],
    ))
    .unwrap();
    let Output::Zscore(z) = &result.results[0] else {
        panic!()
    };
    assert_eq!(z[0].value, Scalar::InsufficientData);
    assert!(matches!(z[1].value, Scalar::Undefined { .. }));
    assert_eq!(
        calculate(&request(&[], vec![describe()])).unwrap_err().code,
        ErrorCode::InsufficientData
    );
    for invalid in [f64::NAN, f64::INFINITY, 1e51] {
        assert_eq!(
            calculate(&request(&[invalid], vec![describe()]))
                .unwrap_err()
                .code,
            ErrorCode::InvalidSample
        );
    }
}

#[test]
fn return_transforms_and_current_window_have_no_lookahead() {
    let mut req = request(
        &[100., 110., 121.],
        vec![
            Operation::Transform {
                kind: Transform::SimpleReturn,
                lag: 1,
            },
            Operation::Transform {
                kind: Transform::LogReturn,
                lag: 1,
            },
            Operation::Transform {
                kind: Transform::CumulativeReturn,
                lag: 1,
            },
            Operation::Transform {
                kind: Transform::Difference,
                lag: 2,
            },
            Operation::Transform {
                kind: Transform::Lag,
                lag: 1,
            },
        ],
    );
    let result = calculate(&req).unwrap();
    let Output::Transform(simple) = &result.results[0] else {
        panic!()
    };
    assert_eq!(simple[0].value, Scalar::InsufficientData);
    near(value(&simple[2].value), 0.1);
    let Output::Transform(log) = &result.results[1] else {
        panic!()
    };
    near(value(&log[2].value), 1.1_f64.ln());
    let Output::Transform(cum) = &result.results[2] else {
        panic!()
    };
    near(value(&cum[0].value), 0.);
    near(value(&cum[2].value), 0.21);
    let Output::Transform(diff) = &result.results[3] else {
        panic!()
    };
    near(value(&diff[2].value), 21.);
    let Output::Transform(lag) = &result.results[4] else {
        panic!()
    };
    near(value(&lag[2].value), 110.);
    req.points[0].x = 0.;
    assert_eq!(calculate(&req).unwrap_err().code, ErrorCode::InvalidSample);
    let req = request(
        &[1., 2., 3., 100.],
        vec![
            Operation::RollingZscore {
                window: 3,
                ddof: 0,
                robust: false,
            },
            Operation::RollingZscore {
                window: 3,
                ddof: 0,
                robust: true,
            },
        ],
    );
    let result = calculate(&req).unwrap();
    let Output::Zscore(z) = &result.results[0] else {
        panic!()
    };
    near(value(&z[2].value), (1.5_f64).sqrt());
    let Output::Zscore(z) = &result.results[1] else {
        panic!()
    };
    near(value(&z[2].value), 0.674_489_750_196_081_7);
}

#[test]
fn pair_hand_ols_covariance_and_rank_ties() {
    let mut req = request(
        &[1., 2., 3.],
        vec![
            Operation::Pair {
                ddof: 1,
                window: None,
            },
            Operation::Pair {
                ddof: 1,
                window: Some(2),
            },
        ],
    );
    for p in &mut req.points {
        p.y = Some(p.x * 2.);
    }
    let result = calculate(&req).unwrap();
    let Output::Pair(rows) = &result.results[0] else {
        panic!()
    };
    let p = &rows[0];
    near(value(&p.pearson), 1.);
    near(value(&p.spearman), 1.);
    near(value(&p.slope_beta_y_on_x), 2.);
    near(value(&p.intercept), 0.);
    near(value(&p.r_squared), 1.);
    near(value(&p.covariance_matrix[0][0]), 1.);
    near(value(&p.covariance_matrix[1][1]), 4.);
    near(value(&p.covariance_matrix[0][1]), 2.);
    assert!(p.residuals.iter().all(|r| value(r) == 0.));
    assert_eq!(p.degrees_of_freedom, Some(1));
    let Output::Pair(rows) = &result.results[1] else {
        panic!()
    };
    assert_eq!(rows[0].pearson, Scalar::InsufficientData);
    near(value(&rows[2].slope_beta_y_on_x), 2.);
    let mut req = request(
        &[1., 2., 2., 4.],
        vec![Operation::Pair {
            ddof: 0,
            window: None,
        }],
    );
    for (p, y) in req.points.iter_mut().zip([4., 1., 1., 2.]) {
        p.y = Some(y);
    }
    let result = calculate(&req).unwrap();
    let Output::Pair(rows) = &result.results[0] else {
        panic!()
    };
    near(value(&rows[0].spearman), -1. / 3.);
    req.points[0].y = None;
    assert_eq!(calculate(&req).unwrap_err().code, ErrorCode::InvalidSample);
    for p in &mut req.points {
        p.x = 1.;
        p.y = Some(2.);
    }
    let result = calculate(&req).unwrap();
    let Output::Pair(rows) = &result.results[0] else {
        panic!()
    };
    assert!(matches!(
        rows[0].slope_beta_y_on_x,
        Scalar::Undefined { .. }
    ));
}

#[test]
fn independent_distribution_closed_form_references() {
    let result = calculate(&request(
        &[1., 2., 2., 4.],
        vec![
            distribution(Distribution::Normal {
                mean: 0.,
                standard_deviation: 1.,
            }),
            distribution(Distribution::StudentT {
                location: 0.,
                scale: 1.,
                degrees_of_freedom: 1.,
            }),
            distribution(Distribution::Beta {
                alpha: 2.,
                beta: 2.,
            }),
            distribution(Distribution::Binomial {
                trials: 4,
                probability: 0.5,
            }),
            distribution(Distribution::Empirical),
        ],
    ))
    .unwrap();
    let ds: Vec<_> = result
        .results
        .iter()
        .map(|o| match o {
            Output::Distribution(d) => d,
            _ => panic!(),
        })
        .collect();
    near(ds[0].evaluations[0].cdf, 0.5);
    near(
        value(&ds[0].evaluations[0].pdf_or_pmf),
        1. / (2. * std::f64::consts::PI).sqrt(),
    );
    // Student-t(1) equals the standard Cauchy distribution.
    near(ds[1].evaluations[2].cdf, 0.75);
    near(
        value(&ds[1].evaluations[2].pdf_or_pmf),
        1. / (2. * std::f64::consts::PI),
    );
    near(value(&ds[1].quantiles[2]), 1.);
    near(ds[2].evaluations[1].cdf, 0.5);
    near(value(&ds[2].evaluations[1].pdf_or_pmf), 1.5);
    near(ds[3].evaluations[3].cdf, 11. / 16.);
    near(value(&ds[3].evaluations[3].pdf_or_pmf), 6. / 16.);
    near(ds[4].evaluations[3].cdf, 0.75);
    near(value(&ds[4].evaluations[3].pdf_or_pmf), 0.5);
    near(value(&ds[4].quantiles[1]), 2.);
}

#[test]
fn distribution_inversion_monotonicity_and_boundaries() {
    for family in [
        Distribution::Normal {
            mean: 2.,
            standard_deviation: 3.,
        },
        Distribution::StudentT {
            location: 2.,
            scale: 3.,
            degrees_of_freedom: 5.,
        },
        Distribution::Beta {
            alpha: 0.5,
            beta: 3.,
        },
    ] {
        let ps = vec![0.01, 0.1, 0.25, 0.5, 0.75, 0.9, 0.99];
        let mut req = request(
            &[],
            vec![Operation::Distribution {
                task: DistributionTask {
                    distribution: family.clone(),
                    evaluate_at: vec![],
                    quantiles: ps.clone(),
                    sampling: None,
                },
            }],
        );
        let result = calculate(&req).unwrap();
        let Output::Distribution(d) = &result.results[0] else {
            panic!()
        };
        let xs: Vec<_> = d.quantiles.iter().map(value).collect();
        assert!(xs.windows(2).all(|w| w[0] < w[1]));
        req.operations = vec![Operation::Distribution {
            task: DistributionTask {
                distribution: family,
                evaluate_at: xs,
                quantiles: vec![],
                sampling: None,
            },
        }];
        let result = calculate(&req).unwrap();
        let Output::Distribution(d) = &result.results[0] else {
            panic!()
        };
        for (e, p) in d.evaluations.iter().zip(ps) {
            near(e.cdf, p);
        }
    }
    let bad = request(
        &[],
        vec![distribution(Distribution::Normal {
            mean: 0.,
            standard_deviation: 0.,
        })],
    );
    assert_eq!(
        calculate(&bad).unwrap_err().code,
        ErrorCode::InvalidParameter
    );
    let req = request(
        &[],
        vec![Operation::Distribution {
            task: DistributionTask {
                distribution: Distribution::Normal {
                    mean: 0.,
                    standard_deviation: 1.,
                },
                evaluate_at: vec![],
                quantiles: vec![0., 1.],
                sampling: None,
            },
        }],
    );
    let result = calculate(&req).unwrap();
    let Output::Distribution(d) = &result.results[0] else {
        panic!()
    };
    assert!(d
        .quantiles
        .iter()
        .all(|q| matches!(q, Scalar::Undefined { .. })));
}

#[test]
fn seeded_sampling_is_reproducible_and_cancellable() {
    let mut req = request(
        &[],
        vec![Operation::Distribution {
            task: DistributionTask {
                distribution: Distribution::Normal {
                    mean: 0.,
                    standard_deviation: 1.,
                },
                evaluate_at: vec![],
                quantiles: vec![],
                sampling: Some(Sampling {
                    seed: 42,
                    samples: 1000,
                }),
            },
        }],
    );
    let a = calculate(&req).unwrap();
    let b = calculate(&req).unwrap();
    assert_eq!(a.output_hash, b.output_hash);
    let Output::Distribution(d) = &a.results[0] else {
        panic!()
    };
    assert_eq!(d.completed_samples, 1000);
    assert!(d.samples.iter().all(|x| x.is_finite()));
    if let Operation::Distribution { task } = &mut req.operations[0] {
        task.sampling.as_mut().unwrap().seed = 43;
    }
    let c = calculate(&req).unwrap();
    let Output::Distribution(e) = &c.results[0] else {
        panic!()
    };
    assert_ne!(d.samples, e.samples);
    let mut calls = 0;
    let error = calculate_controlled(&req, || {
        calls += 1;
        if calls == 20 {
            Err(TaError::new(ErrorCode::Cancelled, "test cancellation"))
        } else {
            Ok(())
        }
    })
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::Cancelled);
    assert_eq!(calls, 20);
}

#[test]
fn wilson_and_beta_hand_values_and_boundary_events() {
    let mut req = request(&[], vec![probability()]);
    req.events = events(&[true, true, false]);
    let result = calculate(&req).unwrap();
    let Output::Probability(p) = &result.results[0] else {
        panic!()
    };
    assert_eq!(p.sample_count, 3);
    assert_eq!(p.hits, 2);
    near(p.empirical_probability, 2. / 3.);
    near(p.artifact.posterior.alpha, 3.);
    near(p.artifact.posterior.beta, 2.);
    near(p.inference.probability, 0.6);
    assert_eq!(
        p.confidence_interval.interval_type,
        "frequentist_confidence"
    );
    assert_eq!(
        p.inference.parameter_credible_interval.interval_type,
        "bayesian_credible"
    );
    assert_eq!(p.effective_sample_count, None);
    // Wilson score formula, n=3, k=2, z=1.959963984540054.
    near(p.confidence_interval.lower, 0.207_659_600_802_047_7);
    near(p.confidence_interval.upper, 0.938_508_055_279_603_7);
    let later = infer_beta(&p.artifact, 20_000).unwrap();
    near(later.probability, 0.6);
    assert_eq!(
        infer_beta(&p.artifact, 9_999).unwrap_err().code,
        ErrorCode::InvalidTime
    );
    let mut corrupt = p.artifact.clone();
    corrupt.hits = 1;
    assert_eq!(
        infer_beta(&corrupt, 20_000).unwrap_err().code,
        ErrorCode::IncompatibleArtifact
    );
    corrupt = p.artifact.clone();
    corrupt.artifact_version = 2;
    assert_eq!(
        infer_beta(&corrupt, 20_000).unwrap_err().code,
        ErrorCode::IncompatibleArtifact
    );
    for hit in [false, true] {
        req.events = events(&[hit; 10]);
        let result = calculate(&req).unwrap();
        let Output::Probability(p) = &result.results[0] else {
            panic!()
        };
        assert!(p.confidence_interval.upper > 0. && p.confidence_interval.lower < 1.);
        assert!(p.confidence_interval.upper > p.confidence_interval.lower);
        assert!(p.inference.probability > 0. && p.inference.probability < 1.);
    }
}

#[test]
fn event_intervals_maturity_condition_and_duplicate_isolation() {
    let mut req = request(&[], vec![probability()]);
    req.fit_cutoff_ms = 7;
    req.events = events(&[true, false, true, false]);
    // e0=[1,3), overlapping=[2,4), e1=[3,5), e2=[5,7), e3=[7,9).
    req.events.push(Event {
        id: "overlap".into(),
        start_ms: 2,
        end_ms: 4,
        available_at_ms: 4,
        condition_known_at_ms: 2,
        condition_matches: true,
        hit: Some(false),
    });
    req.events[1].condition_matches = false;
    let result = calculate(&req).unwrap();
    let Output::Probability(p) = &result.results[0] else {
        panic!()
    };
    assert_eq!(p.selected_ids, vec!["e0", "e2"]);
    assert_eq!(p.excluded_overlap, 1);
    assert_eq!(p.excluded_condition, 1);
    assert_eq!(p.excluded_immature, 1);
    let artifact_hash = p.artifact.selection_hash.clone();
    req.events[3].hit = Some(true);
    let result = calculate(&req).unwrap();
    let Output::Probability(p) = &result.results[0] else {
        panic!()
    };
    assert_eq!(p.artifact.selection_hash, artifact_hash);
    near(p.empirical_probability, 1.);
    req.events.push(req.events[0].clone());
    assert_eq!(
        calculate(&req).unwrap_err().code,
        ErrorCode::DuplicateSample
    );
    req.events.pop();
    req.events[0].condition_known_at_ms = 2;
    assert_eq!(calculate(&req).unwrap_err().code, ErrorCode::InvalidTime);
}

#[test]
fn future_points_hash_identity_and_replay() {
    let mut full = request(&[1., 2., 3., 100.], vec![describe()]);
    full.fit_cutoff_ms = 3;
    let first = calculate(&full).unwrap();
    assert_eq!(first.selected_points, 3);
    let mut prefix = full.clone();
    prefix.points.pop();
    let second = calculate(&prefix).unwrap();
    assert_eq!(first.selection_hash, second.selection_hash);
    assert_eq!(
        serde_json::to_value(&first.results).unwrap(),
        serde_json::to_value(&second.results).unwrap()
    );
    assert_ne!(first.input_hash, second.input_hash);
    prefix.identity.data_version = "2".into();
    assert_ne!(calculate(&prefix).unwrap().input_hash, second.input_hash);
    full.points[1].available_at_ms = 20_000;
    assert_eq!(calculate(&full).unwrap().selected_points, 2);
    full.points[1].at_ms = 1;
    assert_eq!(calculate(&full).unwrap_err().code, ErrorCode::InvalidTime);
}

#[test]
fn native_budgets_and_serialized_contract_are_enforced() {
    let mut req = request(
        &vec![1.; 4096],
        vec![Operation::RollingZscore {
            window: 4096,
            ddof: 0,
            robust: true,
        }],
    );
    assert_eq!(calculate(&req).unwrap_err().code, ErrorCode::LimitExceeded);
    req.operations = vec![describe()];
    req.points.push(Point {
        at_ms: 4097,
        available_at_ms: 4097,
        x: 1.,
        y: None,
    });
    assert_eq!(calculate(&req).unwrap_err().code, ErrorCode::LimitExceeded);
    let mut json = serde_json::to_value(request(&[1.], vec![describe()])).unwrap();
    json["operations"][0]["unknown"] = serde_json::json!(1);
    assert!(serde_json::from_value::<Request>(json).is_err());
    let mut req = request(&[1.], vec![describe()]);
    if let Operation::Describe { ddof, .. } = &mut req.operations[0] {
        *ddof = 2;
    }
    assert_eq!(
        calculate(&req).unwrap_err().code,
        ErrorCode::InvalidParameter
    );
}

#[test]
fn immutable_inference_rejects_corruption_and_ignores_new_labels() {
    let mut req = request(&[], vec![probability()]);
    req.events = events(&[true, false]);
    let result = calculate(&req).unwrap();
    let Output::Probability(p) = &result.results[0] else {
        panic!()
    };
    let mut artifact = p.artifact.clone();
    artifact.selection_hash.replace_range(
        ..1,
        if artifact.selection_hash.starts_with('0') {
            "1"
        } else {
            "0"
        },
    );
    assert_eq!(
        infer_beta(&artifact, 10_000).unwrap_err().code,
        ErrorCode::IncompatibleArtifact
    );
    artifact = p.artifact.clone();
    artifact.posterior.alpha = f64::NAN;
    assert_eq!(
        infer_beta(&artifact, 10_000).unwrap_err().code,
        ErrorCode::IncompatibleArtifact
    );
    req.operations = vec![Operation::InferBeta {
        artifact: Box::new(p.artifact.clone()),
    }];
    req.events = events(&[false; 20]);
    let result = calculate(&req).unwrap();
    let Output::BetaInference(inference) = &result.results[0] else {
        panic!()
    };
    near(inference.probability, 0.5);
}

#[test]
fn all_distribution_sampling_and_binomial_degenerate_support() {
    for family in [
        Distribution::Normal {
            mean: 0.,
            standard_deviation: 1.,
        },
        Distribution::StudentT {
            location: 0.,
            scale: 1.,
            degrees_of_freedom: 4.,
        },
        Distribution::Beta {
            alpha: 2.,
            beta: 3.,
        },
        Distribution::Binomial {
            trials: 10,
            probability: 0.3,
        },
        Distribution::Empirical,
    ] {
        let req = request(
            &[1., 2., 2., 4.],
            vec![Operation::Distribution {
                task: DistributionTask {
                    distribution: family,
                    evaluate_at: vec![],
                    quantiles: vec![],
                    sampling: Some(Sampling {
                        seed: 7,
                        samples: 64,
                    }),
                },
            }],
        );
        let result = calculate(&req).unwrap();
        let Output::Distribution(d) = &result.results[0] else {
            panic!()
        };
        assert_eq!(d.completed_samples, 64);
        assert!(d.samples.iter().all(|x| x.is_finite()));
        assert_eq!(result.output_hash, calculate(&req).unwrap().output_hash);
    }
    for (trials, p, expected) in [(0, 0.5, 0.), (5, 0., 0.), (5, 1., 5.)] {
        let req = request(
            &[],
            vec![Operation::Distribution {
                task: DistributionTask {
                    distribution: Distribution::Binomial {
                        trials,
                        probability: p,
                    },
                    evaluate_at: vec![-1., 0., 0.5, 5.],
                    quantiles: vec![0.1, 0.5, 0.9],
                    sampling: Some(Sampling {
                        seed: 1,
                        samples: 4,
                    }),
                },
            }],
        );
        let result = calculate(&req).unwrap();
        let Output::Distribution(d) = &result.results[0] else {
            panic!()
        };
        assert!(d.samples.iter().all(|x| *x == expected));
        assert!(d.quantiles.iter().all(|x| value(x) == expected));
        near(d.evaluations[0].cdf, 0.);
        near(value(&d.evaluations[2].pdf_or_pmf), 0.);
    }
}

#[test]
fn delayed_availability_and_bounded_numerical_failure_are_explicit() {
    let mut req = request(
        &[1., 2., 3.],
        vec![
            Operation::RollingZscore {
                window: 3,
                ddof: 0,
                robust: false,
            },
            Operation::Transform {
                kind: Transform::Difference,
                lag: 2,
            },
        ],
    );
    req.points[0].available_at_ms = 20;
    let result = calculate(&req).unwrap();
    let Output::Zscore(z) = &result.results[0] else {
        panic!()
    };
    assert_eq!(z[2].available_at_ms, 20);
    let Output::Transform(t) = &result.results[1] else {
        panic!()
    };
    assert_eq!(t[2].available_at_ms, 20);
    // A very narrow distribution around a large location cannot resolve this quantile in f64.
    req.operations = vec![Operation::Distribution {
        task: DistributionTask {
            distribution: Distribution::StudentT {
                location: 1e12,
                scale: 1e-12,
                degrees_of_freedom: 2.,
            },
            evaluate_at: vec![],
            quantiles: vec![0.9],
            sampling: None,
        },
    }];
    assert_eq!(
        calculate(&req).unwrap_err().code,
        ErrorCode::NumericalFailure
    );
}
