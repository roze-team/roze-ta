use roze_ta::analysis::{self, Request};
use roze_ta::error::ErrorCode;
use serde_json::{json, Value};

fn request(task: Value) -> Request {
    serde_json::from_value(json!({"schema_version":1,"identity":{"series_id":"workflow","instrument":"TEST","timeframe":"snapshot","source":"manual","data_version":"1"},"input_kind":"inference_samples","units":"explicit","as_of_ms":100,"fit_cutoff_ms":100,"points":[],"events":[],"operations":[{"method":"inference","spec":{"available_at_ms":1,"task":task}}]})).unwrap()
}
fn calc(task: Value) -> Value {
    serde_json::to_value(analysis::calculate(&request(task)).unwrap()).unwrap()["results"][0]
        ["result"]
        .clone()
}
fn value(o: &Value, name: &str) -> f64 {
    o["values"]
        .as_array()
        .unwrap()
        .iter()
        .find(|v| v["name"] == name)
        .unwrap()["value"]["value"]
        .as_f64()
        .unwrap()
}
fn rows() -> Vec<Vec<f64>> {
    (0..40)
        .map(|i| {
            let t = i as f64;
            vec![t * 0.1 + (t * 1.3).sin(), t * 0.13 + (t * 0.7).cos()]
        })
        .collect()
}
#[test]
fn gaussian_calibration_is_reproducible_lower_tail_and_scale_invariant() {
    let samples = vec![
        0., 1., -1., 2., 0., -2., 1., 0., 2., -1., 0., 1., -2., 1., 0., 2.,
    ];
    let task = json!({"kind":"adf_gaussian_calibration","samples":samples,"lags":0,"trend":"constant","replicates":99,"seed":123});
    let result = calc(task.clone());
    assert_eq!(result, calc(task.clone()));
    let p = value(&result, "gaussian_null_p_value");
    assert!((0.01..=1.0).contains(&p));
    assert!((p * 100. - (p * 100.).round()).abs() < 1e-10);
    assert!(value(&result, "critical_01") <= value(&result, "critical_05"));
    assert!(value(&result, "critical_05") <= value(&result, "critical_10"));
    let mut scaled = task;
    scaled["samples"] = json!(samples.iter().map(|x| x * 2. + 10.).collect::<Vec<_>>());
    let other = calc(scaled);
    assert_eq!(value(&other, "gaussian_null_p_value"), p);
    assert!((value(&other, "adf_statistic") - value(&result, "adf_statistic")).abs() < 1e-10);
    // EG must calibrate estimated residuals from two independent random walks.
    let eg = calc(
        json!({"kind":"engle_granger_gaussian_calibration","dependent":samples,"independent":(0..16).collect::<Vec<_>>(),"lags":0,"replicates":99,"seed":123}),
    );
    assert!(value(&eg, "critical_05") < value(&result, "critical_05"));
    assert_ne!(value(&eg, "critical_05"), value(&result, "critical_05"));
}
#[test]
fn calibration_rejects_wrong_initial_condition_and_excessive_work() {
    let task = json!({"kind":"adf_gaussian_calibration","samples":[1.,2.,1.,3.,2.,1.,2.],"lags":0,"trend":"none","replicates":99,"seed":0});
    assert_eq!(
        analysis::calculate(&request(task)).unwrap_err().code,
        ErrorCode::InvalidParameter
    );
    let task = json!({"kind":"adf_gaussian_calibration","samples":(0..4096).map(|i|(i as f64).sin()).collect::<Vec<_>>(),"lags":0,"trend":"constant","replicates":4095,"seed":0});
    assert_eq!(
        analysis::calculate(&request(task)).unwrap_err().code,
        ErrorCode::LimitExceeded
    );
}
#[test]
fn fitting_rejects_invalid_bounds_and_propagates_mid_iteration_cancellation() {
    let task = json!({"kind":"dynamics_fit","model":{"kind":"arima","observations":[1.,2.,3.,1.,2.,3.,1.,2.,3.],"ar":[],"ma":[],"intercept":0.,"differences":0},"lower":[0.],"upper":[4.],"max_iterations":40,"tolerance":0.0001});
    let mut invalid = task.clone();
    invalid["upper"] = json!([0.]);
    assert_eq!(
        analysis::calculate(&request(invalid)).unwrap_err().code,
        ErrorCode::InvalidParameter
    );
    let mut calls = 0;
    let error = analysis::calculate_controlled(&request(task), || {
        calls += 1;
        if calls == 30 {
            Err(roze_ta::error::TaError::new(
                ErrorCode::LimitExceeded,
                "cancel test",
            ))
        } else {
            Ok(())
        }
    })
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::LimitExceeded);
    assert_eq!(calls, 30);
}
#[test]
fn rank_zero_vecm_is_random_walk_and_covariance_accumulates_linearly() {
    let data = rows();
    let out = calc(
        json!({"kind":"vecm","observations":data,"lagged_differences":0,"include_constant":false,"rank":0,"steps":4}),
    );
    let m = &out["vecm"];
    for row in m["forecasts"].as_array().unwrap() {
        assert_eq!(row, &json!(data.last().unwrap()));
    }
    for h in 0..4 {
        for i in 0..2 {
            for j in 0..2 {
                let mut cross = 0.;
                for pair in data.windows(2) {
                    cross += (pair[1][i] - pair[0][i]) * (pair[1][j] - pair[0][j]);
                }
                let expected = cross / 39.;
                assert!(
                    (m["innovation_covariance"][i][j].as_f64().unwrap() - expected).abs() < 1e-12
                );
                assert!(
                    (m["forecast_covariances"][h][i][j].as_f64().unwrap()
                        - (h + 1) as f64 * expected)
                        .abs()
                        < 1e-12
                );
            }
        }
    }
}
#[test]
fn fitted_arima_intercept_matches_analytic_sample_mean() {
    let task = json!({"kind":"dynamics_fit","model":{"kind":"arima","observations":[1.,2.,3.,1.,2.,3.,1.,2.,3.],"ar":[],"ma":[],"intercept":0.,"differences":0},"lower":[0.],"upper":[4.],"max_iterations":40,"tolerance":0.0001});
    let out = calc(task.clone());
    let fit = &out["fit"];
    assert_eq!(fit["model"]["intercept"], 2.);
    assert_eq!(fit["objective"], 6.);
    assert_eq!(fit["initial_objective"], 42.);
    assert_eq!(fit["converged"], true);
    let mut short = task;
    short["max_iterations"] = json!(1);
    assert_eq!(calc(short)["fit"]["converged"], false);
}
#[test]
fn full_rank_vecm_matches_independent_var_two_fit_and_forecast() {
    let reference: Value =
        serde_json::from_str(include_str!("fixtures/vecm-var-reference.json")).unwrap();
    let out = calc(
        json!({"kind":"vecm","observations":reference["observations"],"lagged_differences":1,"include_constant":true,"rank":2,"steps":3}),
    );
    fn close(a: &Value, b: &Value) {
        if let Some(x) = a.as_f64() {
            assert!((x - b.as_f64().unwrap()).abs() < 1e-9, "{a} vs {b}");
        } else {
            let aa = a.as_array().unwrap();
            let bb = b.as_array().unwrap();
            assert_eq!(aa.len(), bb.len());
            for (x, y) in aa.iter().zip(bb) {
                close(x, y);
            }
        }
    }
    for key in [
        "constant",
        "short_run",
        "innovation_covariance",
        "forecasts",
        "forecast_covariances",
    ] {
        close(&out["vecm"][key], &reference[key]);
    }
}
#[test]
fn fitted_garch_likelihood_matches_independent_recursion() {
    let residuals = vec![1., -2., 1., 2., -1., 3., -2., 1., -1., 2.];
    let out = calc(
        json!({"kind":"dynamics_fit","model":{"kind":"garch","residuals":residuals,"omega":0.5,"alpha":[0.1],"beta":[0.5],"initial_squared_residuals":[1.],"initial_variances":[2.]},"lower":[0.01,0.,0.],"upper":[5.,0.8,0.8],"max_iterations":40,"tolerance":0.001}),
    );
    let f = &out["fit"];
    let m = &f["model"];
    let w = m["omega"].as_f64().unwrap();
    let a = m["alpha"][0].as_f64().unwrap();
    let b = m["beta"][0].as_f64().unwrap();
    assert!(a + b < 1.);
    let (mut h, mut shock, mut nll) = (2., 1., 0.);
    for e in residuals {
        h = w + a * shock + b * h;
        shock = e * e;
        nll += 0.5 * (std::f64::consts::TAU.ln() + h.ln() + shock / h);
    }
    assert!((f["objective"].as_f64().unwrap() - nll).abs() < 1e-10);
    assert!(nll < f["initial_objective"].as_f64().unwrap());
}
#[test]
fn dcc_scores_pre_observation_correlation_and_hawkes_uses_event_likelihood() {
    // a=b=0 initially gives R=I; initial quasi-likelihood is sum(z'z)/2.
    let z = vec![
        vec![1., 0.2],
        vec![-1., 0.5],
        vec![0.3, -1.],
        vec![0.7, 0.2],
        vec![-0.4, -0.3],
        vec![0.8, 1.],
    ];
    let expected = z.iter().flatten().map(|x| x * x).sum::<f64>() / 2.;
    let out = calc(
        json!({"kind":"dynamics_fit","model":{"kind":"dcc","standardized_residuals":z,"a":0.,"b":0.,"long_run_q":[[1.,0.],[0.,1.]],"initial_q":[[1.,0.],[0.,1.]]},"lower":[0.,0.],"upper":[0.8,0.8],"max_iterations":10,"tolerance":0.001}),
    );
    assert!((out["fit"]["initial_objective"].as_f64().unwrap() - expected).abs() < 1e-12);
    let out = calc(
        json!({"kind":"dynamics_fit","model":{"kind":"hawkes","baseline":[1.],"alpha":[[0.]],"beta":[[1.]],"arrivals":[{"time":0.2,"channel":0},{"time":0.8,"channel":0},{"time":1.5,"channel":0},{"time":2.4,"channel":0},{"time":3.5,"channel":0},{"time":4.8,"channel":0}],"horizon":6.},"lower":[0.1,0.,0.1],"upper":[2.,1.,2.],"max_iterations":20,"tolerance":0.001}),
    );
    assert_eq!(out["fit"]["initial_objective"], 6.);
    let f = &out["fit"];
    let m = &f["model"];
    let mu = m["baseline"][0].as_f64().unwrap();
    let a = m["alpha"][0][0].as_f64().unwrap();
    let b = m["beta"][0][0].as_f64().unwrap();
    let times: [f64; 6] = [0.2, 0.8, 1.5, 2.4, 3.5, 4.8];
    let mut ll = -mu * 6.;
    for (i, t) in times.iter().enumerate() {
        ll += (mu
            + times[..i]
                .iter()
                .map(|s| a * (-b * (t - s)).exp())
                .sum::<f64>())
        .ln();
        ll -= a / b * (1. - (-b * (6. - t)).exp());
    }
    assert!((f["objective"].as_f64().unwrap() + ll).abs() < 1e-10);
}
