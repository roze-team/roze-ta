use roze_ta::analysis::{self, Request};
use serde_json::{json, Value};
fn run(family: Value) -> Value {
    let req:Request=serde_json::from_value(json!({"schema_version":1,"identity":{"series_id":"dist","instrument":"TEST","timeframe":"1d","source":"manual","data_version":"1"},"input_kind":"parameters","units":"scalar","as_of_ms":10,"fit_cutoff_ms":10,"points":[],"events":[],"operations":[{"method":"distribution","task":{"distribution":family,"evaluate_at":[-1.0,0.0,1.0],"quantiles":[0.0,0.5,1.0],"sampling":{"seed":1,"samples":10}}}]})).unwrap();
    serde_json::to_value(analysis::calculate(&req).unwrap()).unwrap()["results"][0]["result"]
        .clone()
}
#[test]
fn all_new_distributions_reference_values_and_support() {
    for f in [
        json!({"family":"bernoulli","probability":0.25}),
        json!({"family":"poisson","rate":1.0}),
        json!({"family":"exponential","rate":1.0}),
        json!({"family":"log_normal","log_mean":0.0,"log_standard_deviation":1.0}),
    ] {
        let o = run(f.clone());
        assert_eq!(o, run(f));
        assert_eq!(o["evaluations"][0]["cdf"], 0.0);
        assert_eq!(o["completed_samples"], 10);
    }
    let o = run(json!({"family":"poisson","rate":1.0}));
    let pmf = o["evaluations"][1]["pdf_or_pmf"]["value"].as_f64().unwrap();
    assert!((pmf - (-1.0f64).exp()).abs() < 1e-12);
    assert_eq!(o["quantiles"][2]["status"], "undefined");
    assert_eq!(o["quantiles"][1]["value"], 1.0);
    let o = run(json!({"family":"log_normal","log_mean":0.0,"log_standard_deviation":1.0}));
    assert_eq!(o["quantiles"][1]["value"], 1.0);
    let o = run(json!({"family":"exponential","rate":1.0}));
    assert!((o["quantiles"][1]["value"].as_f64().unwrap() - 2f64.ln()).abs() < 1e-12);
}
