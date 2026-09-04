use roze_ta::analysis::{
    self,
    inference::standard_calibration::{calibrate_statistic, MacKinnonCase},
    Request,
};
use roze_ta::error::ErrorCode;
use serde_json::{json, Value};
fn request(method: &str, task: Value) -> Request {
    let input = if method == "inference" {
        "inference_samples"
    } else {
        "stochastic_parameters"
    };
    serde_json::from_value(json!({"schema_version":1,"identity":{"series_id":"calibration","instrument":"TEST","timeframe":"snapshot","source":"manual","data_version":"1"},"input_kind":input,"units":"explicit","as_of_ms":100,"fit_cutoff_ms":100,"points":[],"events":[],"operations":[{"method":method,"spec":{"available_at_ms":1,"task":task}}]})).unwrap()
}
fn calc(method: &str, task: Value) -> Value {
    serde_json::to_value(analysis::calculate(&request(method, task)).unwrap()).unwrap()["results"]
        [0]["result"]
        .clone()
}
#[test]
fn mackinnon_matches_independent_erfc_and_polynomial_reference() {
    let cases = [
        MacKinnonCase::AdfNone,
        MacKinnonCase::AdfConstant,
        MacKinnonCase::AdfLinear,
        MacKinnonCase::EngleGrangerConstant,
    ];
    let references: Vec<Value> =
        serde_json::from_str(include_str!("fixtures/mackinnon-reference.json")).unwrap();
    for row in references {
        let result = calibrate_statistic(
            row["statistic"].as_f64().unwrap(),
            cases[row["case"].as_u64().unwrap() as usize],
            100,
        )
        .unwrap();
        // statrs Normal CDF and the independent libm erfc differ at ~1e-12.
        assert!(
            (result.approximate_p_value - row["p_value"].as_f64().unwrap()).abs() < 1e-10,
            "actual {} reference {}",
            result.approximate_p_value,
            row
        );
        for (actual, expected) in result
            .critical_values
            .iter()
            .zip(row["critical"].as_array().unwrap())
        {
            assert!((actual - expected.as_f64().unwrap()).abs() < 1e-12);
        }
    }
    assert_eq!(
        calibrate_statistic(f64::NAN, cases[0], 100)
            .unwrap_err()
            .code,
        ErrorCode::InvalidParameter
    );
    assert_eq!(
        calibrate_statistic(-3., cases[0], 0).unwrap_err().code,
        ErrorCode::InvalidParameter
    );
}
#[test]
fn adf_and_eg_use_different_critical_sample_counts() {
    let x: Vec<f64> = (0..40)
        .map(|i| (i as f64 * 0.7).sin() + i as f64 * 0.03)
        .collect();
    let y: Vec<f64> = (0..40)
        .map(|i| (i as f64 * 1.3).cos() + i as f64 * 0.1)
        .collect();
    let adf = calc(
        "inference",
        json!({"kind":"adf_mac_kinnon","samples":x,"lags":2,"trend":"constant"}),
    );
    assert_eq!(adf["mackinnon"]["critical_value_observations"], 37);
    let eg = calc(
        "inference",
        json!({"kind":"engle_granger_mac_kinnon","dependent":y,"independent":x,"lags":2}),
    );
    assert_eq!(eg["mackinnon"]["critical_value_observations"], 39);
    assert_eq!(eg["mackinnon"]["case"], "engle_granger_constant");
}
#[test]
fn johansen_sequential_tables_select_rank_and_drive_forecasts() {
    let x: Vec<_> = (0..40)
        .map(|i| {
            let t = i as f64;
            vec![t * 0.1 + (t * 1.3).sin(), t * 0.13 + (t * 0.7).cos()]
        })
        .collect();
    let task = json!({"kind":"johansen_rank","observations":x,"lagged_differences":0,"include_constant":true,"significance":"five_percent","test":"trace","forecast_steps":2});
    let out = calc("inference", task.clone());
    assert_eq!(out["rank_selection"]["selected_rank"], 0);
    assert_eq!(out["rank_selection"]["critical_values"][0], 15.4943);
    assert_eq!(out["rank_selection"]["rejected"], json!([false]));
    assert_eq!(out["vecm"]["forecasts"].as_array().unwrap().len(), 2);
    let mut max = task.clone();
    max["test"] = json!("max_eigenvalue");
    max["forecast_steps"] = Value::Null;
    let out = calc("inference", max);
    assert_eq!(out["rank_selection"]["selected_rank"], 1);
    assert_eq!(
        out["rank_selection"]["critical_values"],
        json!([14.2639, 3.8415])
    );
    assert!(out.get("vecm").is_none());
    let mut ten = task;
    ten["significance"] = json!("ten_percent");
    assert_eq!(calc("inference", ten)["rank_selection"]["selected_rank"], 1);
}
fn heston_task() -> Value {
    // Exact deterministic-variance Heston limit: theta=v0=.04 gives BS sigma=.2.
    json!({"kind":"heston_calibrate","quotes":[{"spot":100.,"strike":100.,"years":1.,"rate":0.05,"dividend_yield":0.,"kind":"call","price":10.450583572185565,"weight":1.}],"initial":{"initial_variance":0.02,"reversion":2.,"long_run_variance":0.04,"vol_of_variance":0.,"correlation":-0.7},"parameters":["initial_variance"],"lower":[0.],"upper":[0.08],"max_iterations":20,"fit_tolerance":0.001,"integration_limit":50.,"intervals":64,"pricing_tolerance":0.001})
}
#[test]
fn heston_calibration_recovers_known_variance_and_reports_iteration_limit() {
    let task = heston_task();
    let result = calc("stochastic", task.clone());
    let fit = &result["calibration"];
    assert!((fit["model"]["initial_variance"].as_f64().unwrap() - 0.04).abs() < 1e-12);
    assert!(fit["weighted_mse"].as_f64().unwrap() < 1e-20);
    assert_eq!(fit["converged"], true);
    assert_eq!(result, calc("stochastic", task.clone()));
    let mut short = task;
    short["max_iterations"] = json!(1);
    assert_eq!(calc("stochastic", short)["calibration"]["converged"], false);
}
#[test]
fn heston_bounds_budget_and_mid_calibration_cancel_are_enforced() {
    let mut task = heston_task();
    task["lower"] = json!([-1.]);
    assert_eq!(
        analysis::calculate(&request("stochastic", task))
            .unwrap_err()
            .code,
        ErrorCode::InvalidParameter
    );
    let mut task = heston_task();
    task["intervals"] = json!(4096);
    assert_eq!(
        analysis::calculate(&request("stochastic", task))
            .unwrap_err()
            .code,
        ErrorCode::LimitExceeded
    );
    let mut calls = 0;
    let error = analysis::calculate_controlled(&request("stochastic", heston_task()), || {
        calls += 1;
        if calls == 15 {
            Err(roze_ta::error::TaError::new(
                ErrorCode::LimitExceeded,
                "cancel",
            ))
        } else {
            Ok(())
        }
    })
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::LimitExceeded);
    assert_eq!(calls, 15);
}
#[test]
fn heston_nonzero_volatility_calibrates_against_riccati_reference() {
    let mut task = heston_task();
    task["quotes"][0]["price"] = json!(10.394218552313326);
    task["initial"]["initial_variance"] = json!(0.04);
    task["initial"]["vol_of_variance"] = json!(0.2);
    task["parameters"] = json!(["vol_of_variance"]);
    task["lower"] = json!([0.1]);
    task["upper"] = json!([0.5]);
    task["intervals"] = json!(128);
    task["pricing_tolerance"] = json!(0.0001);
    let out = calc("stochastic", task);
    let fit = &out["calibration"];
    assert!(
        (fit["model"]["vol_of_variance"].as_f64().unwrap() - 0.3).abs() < 0.002,
        "{fit}"
    );
    assert!(fit["weighted_rmse"].as_f64().unwrap() < 0.0001);
    assert!(fit["weighted_mse"].as_f64().unwrap() < fit["initial_weighted_mse"].as_f64().unwrap());
}
#[test]
fn heston_all_five_parameters_accept_a_multi_quote_surface() {
    let mut task = heston_task();
    // Independent Black-Scholes European call references at sigma=.2, r=.05, T=1.
    let strikes = [80., 90., 100., 110., 120.];
    let prices = [
        24.58883544392775,
        16.699448408416004,
        10.450583572185565,
        6.040088129724239,
        3.2474774165608125,
    ];
    let quotes:Vec<_>=strikes.iter().zip(prices).enumerate().map(|(i,(strike,price))|json!({"spot":100.,"strike":strike,"years":1.,"rate":0.05,"dividend_yield":0.,"kind":"call","price":price,"weight":i+1})).collect();
    task["quotes"] = json!(quotes);
    task["initial"]["initial_variance"] = json!(0.03);
    task["initial"]["vol_of_variance"] = json!(0.1);
    task["parameters"] = json!([
        "initial_variance",
        "reversion",
        "long_run_variance",
        "vol_of_variance",
        "correlation"
    ]);
    task["lower"] = json!([0.01, 0.5, 0.01, 0., -0.9]);
    task["upper"] = json!([0.1, 3., 0.1, 0.5, 0.]);
    task["max_iterations"] = json!(1);
    task["pricing_tolerance"] = json!(0.01);
    let result = calc("stochastic", task);
    let fit = &result["calibration"];
    assert_eq!(fit["active_parameters"].as_array().unwrap().len(), 5);
    assert_eq!(fit["fitted_prices"].as_array().unwrap().len(), 5);
    assert_eq!(fit["converged"], false);
    assert!(fit["weighted_mse"].as_f64().unwrap() <= fit["initial_weighted_mse"].as_f64().unwrap());
    let independent = fit["fitted_prices"]
        .as_array()
        .unwrap()
        .iter()
        .zip(prices)
        .enumerate()
        .map(|(i, (p, expected))| (i + 1) as f64 * (p.as_f64().unwrap() - expected).powi(2))
        .sum::<f64>()
        / 15.;
    assert!((independent - fit["weighted_mse"].as_f64().unwrap()).abs() < 1e-10);
}
