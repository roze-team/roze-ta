use super::*;
use rmcp::model::CallToolRequestParams;
use serde_json::{json, Value};

fn args(id: &str, kind: &str) -> Value {
    let samples:Vec<_>=(1..=40).map(|i|{
        let x=100.0+((i as f64)/3.0).sin()*5.0;
        match kind{
            "bars"=>json!({"candle":{"closed_at_ms":i,"open":x,"high":x+2.0,"low":x-2.0,"close":x+0.5,"volume":i%7+1},"available_at_ms":i+1}),
            "scalars"=>json!({"at_ms":i,"available_at_ms":i+1,"value":x}),
            "pairs"=>json!({"at_ms":i,"available_at_ms":i+1,"value":[x,100.0]}),
            "values"=>json!({"at_ms":i,"available_at_ms":i+1,"value":{"index":i,"x":x}}),
            _=>panic!("input kind"),
        }
    }).collect();
    json!({"schema_version":1,"identity":{"series_id":"all-native","instrument":"TEST","timeframe":"1ms","source":"fixture","data_version":"1"},"as_of_ms":100,"data":{"kind":kind,"samples":samples},"operations":[{"id":id,"params":{}}],"output":"series"})
}
fn call(name: &'static str, args: Value) -> CallToolRequestParams {
    CallToolRequestParams::new(name).with_arguments(args.as_object().unwrap().clone())
}
#[tokio::test]
async fn every_native_entry_input_kind_and_alias_is_callable_over_mcp() -> anyhow::Result<()> {
    let (s, c) = tokio::io::duplex(65536);
    let gateway = IndicatorGateway::default();
    let probe = gateway.clone();
    let server = tokio::spawn(async move {
        gateway.serve(s).await?.waiting().await?;
        anyhow::Ok(())
    });
    let client = ().serve(c).await?;
    let result = client
        .call_tool(CallToolRequestParams::new("native_catalog"))
        .await?;
    assert_eq!(result.is_error, Some(false));
    let catalog = result.structured_content.unwrap();
    assert_eq!(catalog, native::catalog()?);
    assert_eq!(catalog["counts"], json!({"indicators":36,"methods":44}));
    let mut count = 0;
    for entry in catalog["entries"].as_array().unwrap() {
        let ids = std::iter::once(&entry["id"]).chain(entry["aliases"].as_array().unwrap());
        for id in ids {
            for kind in entry["inputs"].as_array().unwrap() {
                let arguments = args(id.as_str().unwrap(), kind.as_str().unwrap());
                let expected: native::Request = serde_json::from_value(arguments.clone())?;
                let result = client
                    .call_tool(call("native_batch_calculate", arguments))
                    .await?;
                assert_eq!(result.is_error, Some(false), "{}: {:?}", id, result);
                assert_eq!(
                    result.structured_content.unwrap(),
                    serde_json::to_value(native::calculate(&expected)?)?
                );
                count += 1;
            }
        }
    }
    assert!(count >= 83); // 80 concrete algorithms plus Past's other input representations, then aliases.
    let arguments = args("method.sma", "scalars");
    let permits = probe.permits.clone().acquire_many_owned(2).await?;
    let result = client
        .call_tool(call("native_batch_calculate", arguments.clone()))
        .await?;
    assert_eq!(
        result.structured_content.unwrap()["error"]["code"],
        "limit_exceeded"
    );
    drop(permits);
    let mut bad = arguments;
    bad["operations"][0]["params"] = json!({"period":256});
    let result = client
        .call_tool(call("native_batch_calculate", bad))
        .await?;
    assert_eq!(
        result.structured_content.unwrap()["error"]["code"],
        "invalid_parameter"
    );
    let mut bad = args("method.renko", "bars");
    bad["operations"][0]["params"] = json!({"size":1e-9});
    let result = client
        .call_tool(call("native_batch_calculate", bad))
        .await?;
    assert_eq!(
        result.structured_content.unwrap()["error"]["code"],
        "limit_exceeded"
    );
    client.cancel().await?;
    server.await??;
    Ok(())
}
