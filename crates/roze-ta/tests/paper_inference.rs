use roze_ta::analysis::{self, Request};
use serde_json::{json, Value};
fn calc(task: Value) -> Value {
    let r:Request=serde_json::from_value(json!({"schema_version":1,"identity":{"series_id":"inference","instrument":"TEST","timeframe":"snapshot","source":"manual","data_version":"1"},"input_kind":"inference_samples","units":"explicit","as_of_ms":10,"fit_cutoff_ms":10,"points":[],"events":[],"operations":[{"method":"inference","spec":{"available_at_ms":1,"task":task}}]})).unwrap();
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
fn mean_tests_and_conjugate_posterior() {
    let o = calc(json!({"kind":"one_sample_t","samples":[1.0,2.0,3.0],"null_mean":2.0}));
    assert_eq!(val(&o, "t_statistic"), 0.0);
    assert_eq!(val(&o, "two_sided_p_value"), 1.0);
    let o = calc(json!({"kind":"welch","first":[1.0,2.0,3.0],"second":[2.0,3.0,4.0]}));
    assert!((val(&o, "t_statistic") + 1.5f64.sqrt()).abs() < 1e-12);
    assert_eq!(val(&o, "degrees_of_freedom"), 4.0);
    let o = calc(
        json!({"kind":"normal_mean_posterior","samples":[2.0,4.0],"known_variance":2.0,"prior_mean":0.0,"prior_variance":1.0}),
    );
    assert_eq!(val(&o, "posterior_mean"), 1.5);
    assert_eq!(val(&o, "posterior_variance"), 0.5);
}
#[test]
fn mle_divisor_and_adf_statistic_not_student_p_value() {
    let o = calc(json!({"kind":"maximum_likelihood","family":"normal","samples":[1.0,2.0,3.0]}));
    assert_eq!(val(&o, "variance_mle"), 2.0 / 3.0);
    let o =
        calc(json!({"kind":"adf","samples":[1.0,2.0,1.0,3.0,2.0,1.0,2.0],"lags":0,"trend":"none"}));
    // Sum xlag^2=20, sum xlag*delta=-3; SSE=9-9/20; df=5.
    assert!((val(&o, "adf_statistic") - (-0.15 / (8.55f64 / 100.0).sqrt())).abs() < 1e-10);
    assert!(!o.to_string().contains("two_sided_p_value"));
}
#[test]
fn johansen_canonical_vectors_normalize_and_statistics_are_ordered() {
    let rows: Vec<_> = (0..40)
        .map(|i| {
            let t = i as f64;
            vec![t * 0.1 + (t * 1.3).sin(), t * 0.13 + (t * 0.7).cos()]
        })
        .collect();
    let o = calc(
        json!({"kind":"johansen","observations":rows,"lagged_differences":0,"include_constant":true,"rank":1}),
    );
    let e = o["eigenvalues"].as_array().unwrap();
    assert!((e[0].as_f64().unwrap() - 0.3096880026203923).abs() < 1e-10);
    assert!((e[1].as_f64().unwrap() - 0.012741373431172598).abs() < 1e-10);
    assert!((val(&o, "trace_rank_0") - 14.95395931961356).abs() < 1e-9);
    assert!(e[0].as_f64().unwrap() >= e[1].as_f64().unwrap());
    assert!(val(&o, "trace_rank_0") >= val(&o, "max_eigen_rank_0"));
    assert_eq!(o["cointegration_vectors"].as_array().unwrap().len(), 2);
}
