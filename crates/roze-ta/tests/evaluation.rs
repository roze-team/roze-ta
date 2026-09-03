use roze_ta::{
    analysis::{evaluation::*, *},
    engine::SeriesIdentity,
    error::{ErrorCode, TaError},
};
fn near(a: f64, b: f64) {
    assert!((a - b).abs() <= 1e-10 * (1. + b.abs()), "{a} != {b}");
}
fn val(s: &Scalar) -> f64 {
    match s {
        Scalar::Ready { value } => *value,
        _ => panic!("{s:?}"),
    }
}
fn request(xs: &[f64], op: Operation) -> Request {
    Request {
        schema_version: 1,
        identity: SeriesIdentity {
            series_id: "eval".into(),
            instrument: "TEST".into(),
            timeframe: "1s".into(),
            source: "manual".into(),
            data_version: "1".into(),
        },
        input_kind: "simple_return".into(),
        units: "ratio".into(),
        as_of_ms: 10000,
        fit_cutoff_ms: 10000,
        points: xs
            .iter()
            .enumerate()
            .map(|(i, &x)| Point {
                at_ms: (i + 1) as i64,
                available_at_ms: (i + 1) as i64,
                x,
                y: None,
            })
            .collect(),
        events: vec![],
        operations: vec![op],
    }
}
fn spec() -> PerformanceSpec {
    PerformanceSpec {
        periods_per_year: 3.,
        risk_free_per_period: 0.01,
        mar_per_period: 0.02,
        ddof: 1,
        tail_level: 0.5,
        include_gaussian_tail: true,
        hac_lags: 1,
        benchmark_id: None,
        return_basis: ReturnBasis::NetOfCosts,
        spacing: ReturnSpacing::ConsecutiveTradingPeriods,
    }
}
fn run(req: &Request) -> PerformanceResult {
    match calculate(req).unwrap().results.remove(0) {
        Output::Performance(p) => *p,
        _ => panic!(),
    }
}
fn trade_spec() -> TradeSpec {
    TradeSpec {
        currency: "CNY".into(),
        cost_definition: "all commission and slippage in same currency".into(),
        trades: [(110., 10.), (-40., 10.), (10., 10.)]
            .into_iter()
            .enumerate()
            .map(|(i, (gross_pnl, costs))| Trade {
                id: format!("t{i}"),
                closed_at_ms: (i + 1) as i64,
                available_at_ms: (i + 1) as i64,
                gross_pnl,
                costs,
            })
            .collect(),
    }
}
fn factor_spec() -> FactorSpec {
    let mut observations = Vec::new();
    for (at, sign) in [(1, 1.), (3, -1.)] {
        for (i, score) in [-1., 0., 1.].into_iter().enumerate() {
            observations.push(FactorObservation {
                asset_id: format!("a{i}"),
                at_ms: at,
                factor_available_at_ms: at,
                label_end_ms: at + 2,
                label_available_at_ms: at + 2,
                score,
                future_return: Some(score * 0.2 * sign),
            });
        }
    }
    FactorSpec {
        factor_id: "score".into(),
        universe_id: "frozen4".into(),
        universe_size: 4,
        label_definition: "2ms terminal simple return".into(),
        horizon_ms: 2,
        minimum_cross_section: 2,
        ddof: 1,
        observations,
    }
}

#[test]
fn performance_matches_compounding_drawdown_and_independent_ratios() {
    let mut s = spec();
    s.benchmark_id = Some("benchmark".into());
    let mut req = request(&[0.1, -0.2, 0.25], Operation::Performance { spec: s });
    for (p, y) in req.points.iter_mut().zip([0., -0.1, 0.1]) {
        p.y = Some(y);
    }
    let r = run(&req);
    near(r.arithmetic_mean_return, 0.05);
    near(val(&r.cumulative_return), 0.1);
    near(val(&r.annualized_return), 0.1);
    near(r.maximum_drawdown, 0.2);
    near(val(&r.calmar), 0.5);
    near(r.annualized_volatility, (0.0525_f64 * 3.).sqrt());
    near(
        val(&r.annualized_sharpe),
        0.04 * 3_f64.sqrt() / 0.0525_f64.sqrt(),
    );
    near(val(&r.annualized_sortino), 0.03 * 3. / 0.22);
    near(
        val(r.annualized_information_ratio.as_ref().unwrap()),
        0.05 * 3_f64.sqrt() / 0.0175_f64.sqrt(),
    );
    near(
        r.annualized_tracking_error.unwrap(),
        (0.0175_f64 * 3.).sqrt(),
    );
    near(r.equity_curve[1].drawdown, 0.2);
    assert_eq!(r.spec.periods_per_year, 3.);
    assert_eq!(r.method_version, roze_ta::analysis::evaluation::VERSION);
    assert_eq!(r.equity_curve[2].underwater_periods, 0);
    assert_eq!(r.maximum_underwater_periods, 1);
}

#[test]
fn historical_tail_uses_fractional_mass_and_gaussian_is_explicit() {
    let req = request(&[0.1, -0.2, 0.25], Operation::Performance { spec: spec() });
    let r = run(&req);
    near(r.historical_tail.value_at_risk, -0.1);
    near(r.historical_tail.expected_shortfall, 0.1);
    let gaussian = r.gaussian_tail.unwrap();
    near(gaussian.value_at_risk, -0.05);
    near(
        gaussian.expected_shortfall,
        -0.05 + 0.0525_f64.sqrt() * (2. / std::f64::consts::PI).sqrt(),
    );
    let mut s = spec();
    s.include_gaussian_tail = false;
    s.tail_level = 0.75;
    let r = run(&request(
        &[-0.1, -0.1, -0.3, -0.3],
        Operation::Performance { spec: s },
    ));
    near(r.historical_tail.expected_shortfall, 0.3);
    near(r.historical_tail.value_at_risk, 0.3);
    assert!(r.gaussian_tail.is_none());
}

#[test]
fn bartlett_hac_reference_and_effective_n_can_exceed_raw_n() {
    let r = run(&request(
        &[0.1, -0.2, 0.25],
        Operation::Performance { spec: spec() },
    ));
    let gamma0 = 0.105 / 3.;
    let gamma1 = -0.0625 / 3.;
    let lrv = gamma0 + gamma1;
    near(val(&r.hac.autocorrelations[0]), gamma1 / gamma0);
    near(val(&r.hac.mean_excess_standard_error), (lrv / 3_f64).sqrt());
    near(
        val(&r.hac.mean_excess_t_statistic),
        0.04 / (lrv / 3_f64).sqrt(),
    );
    near(val(&r.hac.effective_sample_count), 3. * gamma0 / lrv);
    assert!(val(&r.hac.effective_sample_count) > 3.);
}

#[test]
fn constants_zero_denominators_and_overflow_are_typed() {
    let r = run(&request(
        &[0., 0., 0.],
        Operation::Performance { spec: spec() },
    ));
    assert!(matches!(r.annualized_sharpe, Scalar::Undefined { .. }));
    assert!(matches!(r.calmar, Scalar::Undefined { .. }));
    assert!(matches!(
        r.hac.effective_sample_count,
        Scalar::Undefined { .. }
    ));
    near(r.maximum_drawdown, 0.);
    let r = run(&request(
        &[0.1, 0.1, 0.1],
        Operation::Performance { spec: spec() },
    ));
    assert!(matches!(r.annualized_sortino, Scalar::Undefined { .. }));
    let mut s = spec();
    s.periods_per_year = 1e6;
    let r = run(&request(&[1., 1.], Operation::Performance { spec: s }));
    assert!(matches!(r.annualized_return, Scalar::Undefined { .. }));
    assert!(serde_json::to_string(&r).is_ok());
}

#[test]
fn performance_availability_replay_and_contract_checks() {
    let mut req = request(
        &[0.1, -0.2, 0.25, 0.9],
        Operation::Performance { spec: spec() },
    );
    req.fit_cutoff_ms = 3;
    let a = calculate(&req).unwrap();
    req.points.pop();
    let b = calculate(&req).unwrap();
    assert_eq!(
        serde_json::to_value(a.results).unwrap(),
        serde_json::to_value(b.results).unwrap()
    );
    assert_eq!(a.selection_hash, b.selection_hash);
    req.points[1].available_at_ms = 100;
    assert_eq!(calculate(&req).unwrap_err().code, ErrorCode::InvalidTime);
    req.points[1].available_at_ms = 2;
    req.input_kind = "price".into();
    assert_eq!(
        calculate(&req).unwrap_err().code,
        ErrorCode::InvalidParameter
    );
    req.input_kind = "simple_return".into();
    req.points[1].x = -1.;
    assert_eq!(calculate(&req).unwrap_err().code, ErrorCode::InvalidSample);
    req.points[1].x = 0.;
    req.points[0].y = Some(0.);
    assert_eq!(calculate(&req).unwrap_err().code, ErrorCode::InvalidSample);
}

#[test]
fn trade_costs_are_deducted_once_and_flat_trades_remain_in_denominator() {
    let req = request(&[], Operation::TradeSummary { spec: trade_spec() });
    let result = calculate(&req).unwrap();
    let Output::TradeSummary(t) = &result.results[0] else {
        panic!()
    };
    assert_eq!((t.wins, t.losses, t.breakeven), (1, 1, 1));
    near(t.gross_pnl, 80.);
    near(t.costs, 30.);
    near(t.net_pnl, 50.);
    near(t.mean_net_pnl, 50. / 3.);
    near(t.win_rate, 1. / 3.);
    near(val(&t.average_win), 100.);
    near(val(&t.average_loss_magnitude), 50.);
    near(val(&t.payoff_ratio), 2.);
    near(val(&t.profit_factor), 2.);
    assert_eq!(
        (t.maximum_consecutive_wins, t.maximum_consecutive_losses),
        (1, 1)
    );
}

#[test]
fn trade_duplicates_delays_and_empty_loss_sets_are_explicit() {
    let mut s = trade_spec();
    s.trades[1].gross_pnl = 100.;
    s.trades[2].gross_pnl = 100.;
    let req = request(&[], Operation::TradeSummary { spec: s.clone() });
    let result = calculate(&req).unwrap();
    let Output::TradeSummary(t) = &result.results[0] else {
        panic!()
    };
    assert!(matches!(t.profit_factor, Scalar::Undefined { .. }));
    assert_eq!(t.maximum_consecutive_wins, 3);
    s.trades.push(s.trades[0].clone());
    assert_eq!(
        calculate(&request(&[], Operation::TradeSummary { spec: s.clone() }))
            .unwrap_err()
            .code,
        ErrorCode::DuplicateSample
    );
    s.trades.pop();
    s.trades[1].available_at_ms = 20000;
    assert_eq!(
        calculate(&request(&[], Operation::TradeSummary { spec: s }))
            .unwrap_err()
            .code,
        ErrorCode::InvalidTime
    );
}

#[test]
fn factor_ic_is_cross_sectional_and_icir_is_unannualized() {
    let req = request(
        &[],
        Operation::FactorEvaluation {
            spec: factor_spec(),
        },
    );
    let result = calculate(&req).unwrap();
    let Output::FactorEvaluation(f) = &result.results[0] else {
        panic!()
    };
    assert_eq!(f.rows.len(), 2);
    near(val(&f.rows[0].information_coefficient), 1.);
    near(val(&f.rows[1].information_coefficient), -1.);
    near(val(&f.rows[0].rank_information_coefficient), 1.);
    near(f.rows[0].coverage, 0.75);
    assert_eq!(f.rows[0].direction_count, 2);
    assert_eq!(f.rows[0].zero_direction_count, 1);
    near(val(&f.rows[0].direction_hit_rate), 1.);
    near(val(&f.rows[1].direction_hit_rate), 0.);
    near(val(&f.ic.mean), 0.);
    near(val(&f.ic.standard_deviation), 2_f64.sqrt());
    near(val(&f.ic.information_ratio), 0.);
}

#[test]
fn factor_waits_for_whole_group_and_excludes_real_interval_overlap() {
    let mut s = factor_spec();
    let mut overlap = s.observations[..3].to_vec();
    for o in &mut overlap {
        o.at_ms = 2;
        o.factor_available_at_ms = 2;
        o.label_end_ms = 4;
        o.label_available_at_ms = 4;
    }
    s.observations.extend(overlap);
    let mut future = s.observations[..3].to_vec();
    for o in &mut future {
        o.at_ms = 5;
        o.factor_available_at_ms = 5;
        o.label_end_ms = 7;
        o.label_available_at_ms = 7;
    }
    future[0].label_available_at_ms = 20000;
    future[0].future_return = None;
    s.observations.extend(future);
    let result = calculate(&request(
        &[],
        Operation::FactorEvaluation { spec: s.clone() },
    ))
    .unwrap();
    let Output::FactorEvaluation(f) = &result.results[0] else {
        panic!()
    };
    assert_eq!(f.rows.len(), 2);
    assert_eq!(f.excluded_overlapping_groups, 1);
    assert_eq!(f.excluded_immature_groups, 1);
    let hash = f.selection_hash.clone();
    s.observations.last_mut().unwrap().future_return = Some(100.);
    let result = calculate(&request(&[], Operation::FactorEvaluation { spec: s })).unwrap();
    let Output::FactorEvaluation(f) = &result.results[0] else {
        panic!()
    };
    assert_eq!(f.selection_hash, hash);
}

#[test]
fn factor_constant_duplicate_and_future_feature_fail_closed() {
    let mut s = factor_spec();
    for o in &mut s.observations {
        o.score = 1.;
    }
    let result = calculate(&request(
        &[],
        Operation::FactorEvaluation { spec: s.clone() },
    ))
    .unwrap();
    let Output::FactorEvaluation(f) = &result.results[0] else {
        panic!()
    };
    assert_eq!(f.ic.valid_groups, 0);
    assert_eq!(f.ic.mean, Scalar::InsufficientData);
    assert!(matches!(
        f.rows[0].information_coefficient,
        Scalar::Undefined { .. }
    ));
    s.observations.push(s.observations[0].clone());
    assert_eq!(
        calculate(&request(
            &[],
            Operation::FactorEvaluation { spec: s.clone() }
        ))
        .unwrap_err()
        .code,
        ErrorCode::DuplicateSample
    );
    s.observations.pop();
    s.observations[0].factor_available_at_ms = 2;
    assert_eq!(
        calculate(&request(&[], Operation::FactorEvaluation { spec: s }))
            .unwrap_err()
            .code,
        ErrorCode::InvalidTime
    );
}

#[test]
fn evaluation_budget_and_cancellation_are_enforced() {
    let mut s = spec();
    s.hac_lags = 129;
    assert_eq!(
        calculate(&request(&[0., 0., 0.], Operation::Performance { spec: s }))
            .unwrap_err()
            .code,
        ErrorCode::InvalidParameter
    );
    let req = request(&vec![0.01; 100], Operation::Performance { spec: spec() });
    let mut calls = 0;
    let error = calculate_controlled(&req, || {
        calls += 1;
        if calls == 115 {
            Err(TaError::new(ErrorCode::Cancelled, "test"))
        } else {
            Ok(())
        }
    })
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::Cancelled);
    let mut req = request(&vec![0.01; 4096], Operation::Performance { spec: spec() });
    req.operations = vec![Operation::Performance { spec: spec() }; 16];
    assert_eq!(calculate(&req).unwrap_err().code, ErrorCode::LimitExceeded);
}
