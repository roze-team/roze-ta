use roze_ta::analysis::{self, Request};
use serde_json::{json, Value};
fn calc(task: Value) -> Value {
    let r:Request=serde_json::from_value(json!({"schema_version":1,"identity":{"series_id":"dynamics","instrument":"TEST","timeframe":"snapshot","source":"manual","data_version":"1"},"input_kind":"dynamics_parameters","units":"explicit","as_of_ms":10,"fit_cutoff_ms":10,"points":[],"events":[],"operations":[{"method":"dynamics","spec":{"available_at_ms":1,"task":task}}]})).unwrap();
    serde_json::to_value(analysis::calculate(&r).unwrap()).unwrap()["results"][0]["result"].clone()
}
fn val(o: &Value, n: &str) -> f64 {
    o["values"]
        .as_array()
        .unwrap()
        .iter()
        .find(|v| v["name"] == n)
        .unwrap()["value"]["value"]
        .as_f64()
        .unwrap()
}
fn close(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-9, "{a} != {b}");
}
#[test]
fn conditional_garch_ewma_indexing() {
    let o = calc(
        json!({"kind":"garch","residuals":[2.0],"omega":1.0,"alpha":[0.2],"beta":[0.3],"initial_squared_residuals":[1.0],"initial_variances":[2.0]}),
    );
    close(o["series"][0][0].as_f64().unwrap(), 1.8);
    close(val(&o, "next_variance"), 2.34);
    close(val(&o, "unconditional_variance"), 2.0);
    let o = calc(json!({"kind":"ewma","returns":[2.0],"initial_variance":1.0,"decay":0.5}));
    close(val(&o, "next_variance"), 2.5);
}
#[test]
fn dcc_and_har_reference() {
    let o = calc(
        json!({"kind":"dcc","standardized_residuals":[[1.0,2.0]],"a":0.1,"b":0.8,"long_run_q":[[1.0,0.0],[0.0,1.0]],"initial_q":[[1.0,0.0],[0.0,1.0]]}),
    );
    close(o["series"][0][1].as_f64().unwrap(), 0.2 / 1.3f64.sqrt());
    let o = calc(
        json!({"kind":"har_rv","realized_variances":vec![2.0;22],"intercept":1.0,"daily":0.1,"weekly":0.2,"monthly":0.3}),
    );
    close(val(&o, "next_realized_variance"), 2.2);
}
#[test]
fn kalman_joseph_one_dimensional_hand_example() {
    let o = calc(
        json!({"kind":"kalman","observations":[[2.0]],"transition":[[1.0]],"observation":[[1.0]],"process_covariance":[[0.0]],"observation_covariance":[[1.0]],"initial_state":[0.0],"initial_covariance":[[1.0]]}),
    );
    close(val(&o, "final_state_0"), 1.0);
    close(o["final_matrix"][0][0].as_f64().unwrap(), 0.5);
}
#[test]
fn hawkes_exact_likelihood_and_branching_ratio() {
    let o = calc(
        json!({"kind":"hawkes","baseline":[1.0],"alpha":[[0.5]],"beta":[[2.0]],"arrivals":[{"time":1.0,"channel":0}],"horizon":2.0}),
    );
    close(
        val(&o, "log_likelihood"),
        -2.0 - 0.25 * (1.0 - (-2.0f64).exp()),
    );
    close(val(&o, "terminal_intensity_0"), 1.0 + 0.5 * (-2.0f64).exp());
    close(val(&o, "branching_radius_upper_bound"), 0.25);
}
#[test]
fn arima_differencing_and_ou_transition() {
    let o = calc(
        json!({"kind":"arima","observations":[1.0,3.0,5.0,7.0],"ar":[1.0],"ma":[],"intercept":0.0,"differences":1}),
    );
    close(val(&o, "next_level_forecast"), 9.0);
    let o = calc(
        json!({"kind":"ornstein_uhlenbeck","current":0.0,"long_run_mean":2.0,"reversion":1.0,"diffusion":1.0,"elapsed":0.0}),
    );
    close(val(&o, "conditional_mean"), 0.0);
    close(val(&o, "conditional_variance"), 0.0);
    let o = calc(json!({"kind":"realized_variance","log_returns":[0.1,-0.2]}));
    close(val(&o, "realized_variance"), 0.05);
    let o = calc(json!({"kind":"parkinson","high":[2.0],"low":[1.0],"periods_per_year":4.0}));
    close(val(&o, "annualized_volatility"), 2f64.ln().sqrt());
}
