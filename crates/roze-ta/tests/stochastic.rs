use roze_ta::analysis::{self, Request};
use serde_json::{json, Value};
fn calc(task: Value) -> Value {
    let r:Request=serde_json::from_value(json!({"schema_version":1,"identity":{"series_id":"stochastic","instrument":"TEST","timeframe":"snapshot","source":"manual","data_version":"1"},"input_kind":"stochastic_parameters","units":"explicit","as_of_ms":10,"fit_cutoff_ms":10,"points":[],"events":[],"operations":[{"method":"stochastic","spec":{"available_at_ms":1,"task":task}}]})).unwrap();
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
#[test]
fn heston_independent_riccati_reference_and_deterministic_limit() {
    let mut task = json!({"kind":"heston","parameters":{"spot":100.0,"strike":100.0,"years":1.0,"rate":0.05,"dividend_yield":0.0,"initial_variance":0.04,"reversion":2.0,"long_run_variance":0.04,"vol_of_variance":0.3,"correlation":-0.7},"integration_limit":150.0,"intervals":1600,"tolerance":0.00001});
    let o = calc(task.clone());
    assert!((val(&o, "call") - 10.394218552313326).abs() < 2e-6, "{o}");
    assert_eq!(o["converged"], true);
    task["parameters"]["vol_of_variance"] = json!(0.0);
    let o = calc(task);
    assert!((val(&o, "call") - 10.450583572185565).abs() < 1e-10);
}
#[test]
fn seeded_paths_are_reproducible_and_zero_vol_is_exact() {
    let task = json!({"kind":"paths","process":{"kind":"brownian","initial":0.0,"drift":1.0,"diffusion":0.0},"horizon":2.0,"steps":4,"paths":2,"seed":42});
    let o = calc(task.clone());
    assert_eq!(o, calc(task));
    assert_eq!(val(&o, "terminal_mean"), 2.0);
    assert_eq!(val(&o, "terminal_mean_standard_error"), 0.0);
    let task = json!({"kind":"paths","process":{"kind":"geometric_brownian","initial":1.0,"drift":0.05,"volatility":0.2},"horizon":1.0,"steps":4,"paths":3,"seed":42});
    assert_eq!(calc(task.clone()), calc(task));
}
#[test]
fn ito_log_gbm_drift_correction() {
    let o = calc(
        json!({"kind":"ito","time_derivative":0.0,"space_derivative":0.5,"second_space_derivative":-0.25,"drift":0.2,"diffusion":0.4}),
    );
    assert!((val(&o, "transformed_drift") - 0.08).abs() < 1e-12);
}
