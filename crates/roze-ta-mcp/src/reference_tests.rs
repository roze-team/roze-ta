use super::*;
use rmcp::model::CallToolRequestParams;
use serde_json::{json, Value};

fn payload(kind: &str) -> Value {
    match kind {
        "f64" => json!(100.0),
        "(f64, f64)" => json!([100.0, 3.0]),
        "Candle" => {
            json!({"open":100.0,"high":100.5,"low":99.5,"close":100.0,"volume":10.0,"timestamp":1})
        }
        "OrderBook" => {
            json!({"bids":[{"price":99.9,"size":5.0}],"asks":[{"price":100.1,"size":4.0}]})
        }
        "Trade" => json!({"price":100.0,"size":1.0,"side":"Buy","timestamp":1}),
        "TradeQuote" => {
            json!({"trade":{"price":100.1,"size":1.0,"side":"Buy","timestamp":1},"mid":100.0})
        }
        "CrossSection" => {
            json!({"members":[{"change":1.0,"volume":10.0,"new_high":true,"new_low":false,"above_ma":true,"on_buy_signal":true}],"timestamp":1})
        }
        "DerivativesTick" => {
            json!({"funding_rate":0.0001,"mark_price":100.0,"index_price":99.9,"futures_price":100.2,"open_interest":1000.0,"long_size":600.0,"short_size":400.0,"taker_buy_volume":50.0,"taker_sell_volume":40.0,"long_liquidation":5.0,"short_liquidation":3.0,"timestamp":1})
        }
        _ => panic!("missing fixture for {kind}"),
    }
}
fn call(name: &'static str, args: Value) -> CallToolRequestParams {
    CallToolRequestParams::new(name).with_arguments(args.as_object().unwrap().clone())
}

#[tokio::test]
async fn every_reference_entry_is_discoverable_and_matches_core_over_mcp() -> anyhow::Result<()> {
    tokio::time::timeout(std::time::Duration::from_secs(45), exercise()).await??;
    Ok(())
}
async fn exercise() -> anyhow::Result<()> {
    let (s, c) = tokio::io::duplex(65536);
    let gateway = IndicatorGateway::default();
    let probe = gateway.clone();
    let server = tokio::spawn(async move {
        gateway.serve(s).await?.waiting().await?;
        anyhow::Ok(())
    });
    let client = ().serve(c).await?;
    let result = client
        .call_tool(CallToolRequestParams::new("reference_catalog"))
        .await?;
    assert_eq!(result.is_error, Some(false));
    let catalog = result.structured_content.unwrap();
    assert_eq!(catalog, roze_ta::reference_all::catalog()?);
    let identity = json!({"series_id":"reference-mcp","instrument":"TEST","timeframe":"1ms","source":"fixture","data_version":"1"});
    for entry in catalog["entries"].as_array().unwrap() {
        let args = json!({"schema_version":1,"identity":identity,"operation":{"id":entry["id"],"params":entry["example_params"]},"as_of_ms":10,
            "samples":[{"at_ms":1,"available_at_ms":2,"value":payload(entry["input_type"].as_str().unwrap())}]});
        let request = serde_json::from_value(args.clone())?;
        let expected = roze_ta::reference_all::calculate(&request)?;
        let result = client
            .call_tool(call("reference_batch_calculate", args))
            .await?;
        assert_eq!(result.is_error, Some(false), "{}: {result:?}", entry["id"]);
        assert_eq!(
            result.structured_content.unwrap(),
            expected,
            "{}",
            entry["id"]
        );
    }
    let operation = json!({"id":"wickra.Sma","params":{"period":2}});
    let create = client
        .call_tool(call(
            "reference_stream",
            json!({"action":"create","identity":identity,"operation":operation}),
        ))
        .await?
        .structured_content
        .unwrap();
    let advanced=client.call_tool(call("reference_stream",json!({"action":"advance","identity":identity,"operation":operation,"snapshot":create["snapshot"],"as_of_ms":10,
        "samples":[{"at_ms":1,"available_at_ms":2,"value":2.0},{"at_ms":2,"available_at_ms":3,"value":6.0}]}))).await?.structured_content.unwrap();
    assert_eq!(advanced["latest"]["value"], 4.0);
    let inspected=client.call_tool(call("reference_stream",json!({"action":"inspect","identity":identity,"operation":operation,"snapshot":advanced["snapshot"]}))).await?.structured_content.unwrap();
    assert_eq!(inspected, advanced);
    let reset=client.call_tool(call("reference_stream",json!({"action":"reset","identity":identity,"operation":operation,"snapshot":advanced["snapshot"]}))).await?.structured_content.unwrap();
    assert!(reset["latest"].is_null());
    assert_eq!(reset["snapshot"]["history"], json!([]));
    let permits = probe.permits.clone().acquire_many_owned(2).await?;
    let blocked = client
        .call_tool(call(
            "reference_stream",
            json!({"action":"create","identity":identity,"operation":operation}),
        ))
        .await?;
    assert_eq!(
        blocked.structured_content.unwrap()["error"]["code"],
        "limit_exceeded"
    );
    drop(permits);
    client.cancel().await?;
    server.await??;
    Ok(())
}
