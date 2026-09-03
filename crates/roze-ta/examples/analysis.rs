use roze_ta::analysis::{calculate, infer_beta, Output, Request};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let request: Request = serde_json::from_value(serde_json::json!({
        "schema_version":1,
        "identity":{"series_id":"demo","instrument":"TEST","timeframe":"1ms","source":"manual","data_version":"1"},
        "input_kind":"price","units":"unit","as_of_ms":10,"fit_cutoff_ms":10,
        "points":[{"at_ms":1,"available_at_ms":1,"x":1.0,"y":2.0},{"at_ms":2,"available_at_ms":2,"x":2.0,"y":4.0},{"at_ms":3,"available_at_ms":3,"x":3.0,"y":6.0}],
        "events":[{"id":"e1","start_ms":1,"end_ms":3,"available_at_ms":3,"condition_known_at_ms":1,"condition_matches":true,"hit":true},
                  {"id":"e2","start_ms":3,"end_ms":5,"available_at_ms":5,"condition_known_at_ms":3,"condition_matches":true,"hit":false}],
        "operations":[
            {"method":"describe","ddof":1,"quantiles":[0.25,0.5,0.75],"trim_fraction":0.0,"interval_level":0.95},
            {"method":"pair","ddof":1,"window":null},
            {"method":"probability","task":{"event_definition":"terminal simple return > 0","conditioning":"all events","horizon_ms":2,"label_definition_version":"close/simple_return/no_cost/v1","interval_level":0.95,"prior":{"alpha":1.0,"beta":1.0},"assumption":"iid_after_nonoverlap_selection"}}
        ]
    }))?;
    let result = calculate(&request)?;
    println!("{}", serde_json::to_string_pretty(&result)?);
    if let Some(Output::Probability(p)) = result.results.last() {
        let inference = infer_beta(&p.artifact, 20)?;
        println!(
            "Later inference without refitting: {}",
            serde_json::to_string(&inference)?
        );
    }
    Ok(())
}
