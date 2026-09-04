use roze_ta::{
    analysis::{self, portfolio::PortfolioSpec, Operation, Output, Request, Scalar},
    error::{ErrorCode, TaError},
};

fn request() -> Request {
    serde_json::from_str(include_str!(
        "../../../docs/usage/portfolio-request-v1.json"
    ))
    .unwrap()
}
fn spec(r: &mut Request) -> &mut PortfolioSpec {
    match &mut r.operations[0] {
        Operation::PortfolioRisk { spec } => spec,
        _ => panic!("wrong operation"),
    }
}
fn run(r: &Request) -> analysis::portfolio::PortfolioResult {
    match analysis::calculate(r).unwrap().results.remove(0) {
        Output::PortfolioRisk(r) => *r,
        _ => panic!("wrong result"),
    }
}
fn value(s: &Scalar) -> f64 {
    match s {
        Scalar::Ready { value } => *value,
        _ => panic!("{s:?}"),
    }
}
fn near(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-10 * (1.0 + b.abs()), "{a} != {b}");
}

#[test]
fn hand_calculated_covariance_contributions_concentration_and_stress() {
    let result = run(&request());
    // Covariance [[.01,-.005],[-.005,.01]], weights [.6,.4].
    // Portfolio returns [-.06,.04,.02], variance .0028.
    near(value(&result.covariance_per_period[0][0]), 0.01);
    near(value(&result.covariance_per_period[0][1]), -0.005);
    near(value(&result.correlation[0][1]), -0.5);
    near(
        value(&result.annualized_volatility),
        (0.0028_f64 * 252.0).sqrt(),
    );
    near(
        value(&result.assets[0].volatility_contribution),
        0.0024 / 0.0028_f64.sqrt() * 252.0_f64.sqrt(),
    );
    near(
        result
            .assets
            .iter()
            .map(|a| value(&a.volatility_contribution))
            .sum(),
        value(&result.annualized_volatility),
    );
    near(value(&result.concentration_hhi), 0.52);
    near(value(&result.effective_asset_count), 1.0 / 0.52);
    near(result.gross_exposure_ratio, 1.0);
    near(result.scenarios[0].pnl, -160.0);
    near(result.scenarios[0].equity_after, 840.0);
    near(result.scenarios[0].return_on_equity, -0.16);
}

#[test]
fn hedge_and_zero_exposure_do_not_fabricate_ratios() {
    let mut r = request();
    let s = spec(&mut r);
    s.assets[0].weight = 1.0;
    s.assets[1].weight = -1.0;
    for row in &mut s.observations {
        row.returns[1] = row.returns[0];
    }
    let result = run(&r);
    near(result.gross_exposure_ratio, 2.0);
    near(result.net_exposure_ratio, 0.0);
    near(value(&result.annualized_volatility), 0.0);
    assert!(matches!(
        result.diversification_ratio,
        Scalar::Undefined { .. }
    ));
    assert!(matches!(
        result.assets[0].volatility_contribution,
        Scalar::Undefined { .. }
    ));
    for a in &mut spec(&mut r).assets {
        a.weight = 0.0;
    }
    assert!(matches!(
        run(&r).concentration_hhi,
        Scalar::Undefined { .. }
    ));
}

#[test]
fn insufficient_history_preserves_exposures_and_stress() {
    let mut r = request();
    spec(&mut r).observations.clear();
    let result = run(&r);
    assert!(matches!(
        result.annualized_volatility,
        Scalar::InsufficientData
    ));
    assert!(matches!(result.correlation[0][0], Scalar::InsufficientData));
    near(result.scenarios[0].loss, 160.0);
    near(result.assets[0].signed_exposure, 600.0);
}

#[test]
fn cutoff_filters_suffix_rejects_gaps_and_future_scenario_inputs() {
    let mut r = request();
    r.fit_cutoff_ms = 10;
    spec(&mut r).observations[2].available_at_ms = 11;
    let result = run(&r);
    assert_eq!(result.selected_observations, 2);
    let selected_hash = result.selection_hash;
    spec(&mut r).observations[2].returns[0] = 1.0;
    assert_eq!(selected_hash, run(&r).selection_hash);
    spec(&mut r).observations[0].available_at_ms = 11;
    assert_eq!(
        analysis::calculate(&r).unwrap_err().code,
        ErrorCode::InvalidTime
    );
    let mut r = request();
    spec(&mut r).weights_available_at_ms = 101;
    assert_eq!(
        analysis::calculate(&r).unwrap_err().code,
        ErrorCode::InvalidTime
    );
    let mut r = request();
    spec(&mut r).scenarios[0].available_at_ms = 101;
    assert_eq!(
        analysis::calculate(&r).unwrap_err().code,
        ErrorCode::InvalidTime
    );
}

#[test]
fn invalid_dimensions_duplicates_domains_and_budget_are_rejected() {
    let mut r = request();
    spec(&mut r).observations[0].returns.pop();
    assert_eq!(
        analysis::calculate(&r).unwrap_err().code,
        ErrorCode::InvalidSample
    );
    let mut r = request();
    spec(&mut r).assets[1].id = "A".into();
    assert_eq!(
        analysis::calculate(&r).unwrap_err().code,
        ErrorCode::InvalidParameter
    );
    for bad in [f64::NAN, f64::INFINITY, -1.01, 101.0] {
        let mut r = request();
        spec(&mut r).observations[0].returns[0] = bad;
        assert_eq!(
            analysis::calculate(&r).unwrap_err().code,
            ErrorCode::InvalidSample
        );
    }
    let mut r = request();
    spec(&mut r).equity = 0.0;
    assert_eq!(
        analysis::calculate(&r).unwrap_err().code,
        ErrorCode::InvalidParameter
    );
    let mut r = request();
    let a = spec(&mut r).assets[0].clone();
    spec(&mut r).assets = vec![a; 17];
    assert_eq!(
        analysis::calculate(&r).unwrap_err().code,
        ErrorCode::LimitExceeded
    );
    let mut r = request();
    r.operations = vec![r.operations[0].clone(); 17];
    assert_eq!(
        analysis::calculate(&r).unwrap_err().code,
        ErrorCode::LimitExceeded
    );
    let mut r = request();
    spec(&mut r).scenarios[0].shocks.clear();
    assert_eq!(
        analysis::calculate(&r).unwrap_err().code,
        ErrorCode::InvalidSample
    );
}

#[test]
fn zero_variance_extreme_loss_hash_and_cancellation() {
    let mut r = request();
    for row in &mut spec(&mut r).observations {
        row.returns.fill(0.0);
    }
    assert!(matches!(
        run(&r).correlation[0][0],
        Scalar::Undefined { .. }
    ));
    spec(&mut r).assets[0].weight = 10.0;
    spec(&mut r).scenarios[0].shocks[0] = -1.0;
    assert!(run(&r).scenarios[0].equity_after < 0.0); // No insolvency clamp.
    let before = analysis::calculate(&r).unwrap();
    assert_eq!(
        before.output_hash,
        analysis::calculate(&r).unwrap().output_hash
    );
    spec(&mut r).assets[0].weight = 9.0;
    assert_ne!(
        before.output_hash,
        analysis::calculate(&r).unwrap().output_hash
    );
    let mut calls = 0;
    let e = analysis::calculate_controlled(&r, || {
        calls += 1;
        if calls >= 3 {
            Err(TaError::new(ErrorCode::Cancelled, "cancel fixture"))
        } else {
            Ok(())
        }
    })
    .unwrap_err();
    assert_eq!(e.code, ErrorCode::Cancelled);
}
