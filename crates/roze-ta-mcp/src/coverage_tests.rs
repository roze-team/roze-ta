use super::*;
use rmcp::model::CallToolRequestParams;
use serde_json::{json, Value};
use std::collections::BTreeSet;

fn identity() -> engine::SeriesIdentity {
    engine::SeriesIdentity {
        series_id: "fixture".into(),
        instrument: "TEST".into(),
        timeframe: "1ms".into(),
        source: "manual".into(),
        data_version: "1".into(),
    }
}

fn bars() -> Vec<engine::ClosedBar> {
    (1..=300)
        .map(|i| {
            let close = 100.0 + (i as f64 / 7.0).sin() * 5.0;
            engine::ClosedBar {
                candle: catalog::Candle {
                    closed_at_ms: i,
                    open: close,
                    high: close + 2.0,
                    low: close - 2.0,
                    close,
                    volume: (i % 11) as f64,
                },
                available_at_ms: i + 1,
            }
        })
        .collect()
}

fn call(name: &'static str, args: Value) -> CallToolRequestParams {
    CallToolRequestParams::new(name).with_arguments(args.as_object().unwrap().clone())
}

fn success(result: CallToolResult) -> Value {
    assert_eq!(result.is_error, Some(false), "{result:?}");
    result.structured_content.unwrap()
}

#[tokio::test]
async fn every_profile_batch_stream_restore_inspect_reset_matches_native() -> anyhow::Result<()> {
    let (server_transport, client_transport) = tokio::io::duplex(65536);
    let server = tokio::spawn(async move {
        IndicatorGateway::default()
            .serve(server_transport)
            .await?
            .waiting()
            .await?;
        anyhow::Ok(())
    });
    let client = ().serve(client_transport).await?;
    let catalog_result = success(
        client
            .call_tool(CallToolRequestParams::new("indicator_catalog"))
            .await?,
    );
    assert_eq!(
        catalog_result["capabilities"]["stream_tool"],
        "indicator_stream"
    );
    let profiles: Vec<_> = catalog::catalog().into_iter().map(|p| p.id).collect();
    assert_eq!(profiles.len(), 50);
    let bars = bars();
    let batch_args = json!({"schema_version":2,"snapshot_id":"all-profiles","identity":identity(),
        "as_of_ms":1000,"bars":bars,"profiles":profiles,"output":"latest"});
    let native_batch: engine::BatchRequest = serde_json::from_value(batch_args.clone())?;
    assert_eq!(
        success(
            client
                .call_tool(call("indicator_batch_calculate_v2", batch_args))
                .await?
        ),
        serde_json::to_value(engine::calculate(&native_batch)?)?
    );
    let legacy_args = json!({"snapshot_id":"all-profiles","symbol":"TEST","timeframe":"1ms",
        "as_of_ms":1000,"bars":bars.iter().map(|b| &b.candle).collect::<Vec<_>>(),"profiles":profiles});
    let native_legacy: catalog::BatchRequest = serde_json::from_value(legacy_args.clone())?;
    assert_eq!(
        success(
            client
                .call_tool(call("indicator_batch_calculate", legacy_args))
                .await?
        ),
        serde_json::to_value(catalog::calculate(&native_legacy).map_err(anyhow::Error::msg)?)?
    );

    for id in profiles {
        let mut native = engine::Stream::new(identity(), &id)?;
        let args = json!({"schema_version":1,"identity":identity(),"profile_id":id,"as_of_ms":1000,
            "action":{"kind":"create","bars":&bars[..143],"output":"series"}});
        let first = success(
            client
                .call_tool(call("indicator_stream", args.clone()))
                .await?,
        );
        let rows: Vec<_> = bars[..143]
            .iter()
            .map(|b| native.update(b, 1000))
            .collect::<Result<_, _>>()?;
        assert_eq!(first["rows"], serde_json::to_value(rows)?);
        assert_eq!(first["snapshot"], serde_json::to_value(native.snapshot()?)?);
        // Same request is replayable and has no server-side mutation.
        assert_eq!(
            first,
            success(client.call_tool(call("indicator_stream", args)).await?)
        );
        let mut advance = json!({"schema_version":1,"identity":identity(),"profile_id":id,"as_of_ms":1000,
            "action":{"kind":"advance","snapshot":first["snapshot"],"bars":&bars[143..],"output":"series"}});
        let second = success(
            client
                .call_tool(call("indicator_stream", advance.clone()))
                .await?,
        );
        let rows: Vec<_> = bars[143..]
            .iter()
            .map(|b| native.update(b, 1000))
            .collect::<Result<_, _>>()?;
        assert_eq!(second["rows"], serde_json::to_value(rows)?);
        assert_eq!(second["latest"], serde_json::to_value(native.latest())?);
        assert_eq!(
            second["snapshot"],
            serde_json::to_value(native.snapshot()?)?
        );
        assert_eq!(
            second["previous_snapshot_checksum"],
            first["snapshot"]["checksum"]
        );
        advance["action"] = json!({"kind":"inspect","snapshot":second["snapshot"]});
        let inspected = success(
            client
                .call_tool(call("indicator_stream", advance.clone()))
                .await?,
        );
        assert_eq!(inspected["snapshot"], second["snapshot"]);
        assert_eq!(inspected["latest"], second["latest"]);
        advance["action"]["kind"] = json!("reset");
        let reset = success(client.call_tool(call("indicator_stream", advance)).await?);
        native.reset()?;
        assert_eq!(reset["snapshot"], serde_json::to_value(native.snapshot()?)?);
        assert_eq!(reset["samples_seen"], 0);
        assert_eq!(reset["latest"], Value::Null);
        assert_eq!(reset["rows"], json!([]));
    }
    client.cancel().await?;
    server.await??;
    Ok(())
}

#[tokio::test]
async fn all_analysis_methods_are_discoverable_and_callable() -> anyhow::Result<()> {
    let (server_transport, client_transport) = tokio::io::duplex(65536);
    let server = tokio::spawn(async move {
        IndicatorGateway::default()
            .serve(server_transport)
            .await?
            .waiting()
            .await?;
        anyhow::Ok(())
    });
    let client = ().serve(client_transport).await?;
    let mut seen = BTreeSet::new();
    let mut s1 = json!({"schema_version":1,"identity":identity(),"input_kind":"price","units":"unit",
        "as_of_ms":10000,"fit_cutoff_ms":10000,
        "points":(1..=5).map(|i|json!({"at_ms":i,"available_at_ms":i,"x":i as f64,"y":2.0*i as f64})).collect::<Vec<_>>(),
        "events":[{"id":"e1","start_ms":1,"end_ms":3,"available_at_ms":3,"condition_known_at_ms":1,"condition_matches":true,"hit":true},
            {"id":"e2","start_ms":3,"end_ms":5,"available_at_ms":5,"condition_known_at_ms":3,"condition_matches":true,"hit":false}],
        "operations":[{"method":"describe","ddof":1,"quantiles":[0.5],"trim_fraction":0.0,"interval_level":0.95},
            {"method":"transform","kind":"simple_return","lag":1},
            {"method":"rolling_zscore","window":3,"ddof":0,"robust":true},
            {"method":"pair","ddof":1,"window":null},
            {"method":"distribution","task":{"distribution":{"family":"normal","mean":0.0,"standard_deviation":1.0},"evaluate_at":[0.0],"quantiles":[0.5],"sampling":{"seed":42,"samples":4}}},
            {"method":"probability","task":{"event_definition":"return > 0","conditioning":"all","horizon_ms":2,
                "label_definition_version":"v1","interval_level":0.95,"prior":{"alpha":1.0,"beta":1.0},"assumption":"iid_after_nonoverlap_selection"}}]});
    let native: analysis::Request = serde_json::from_value(s1.clone())?;
    let computed = analysis::calculate(&native)?;
    let analysis::Output::Probability(probability) = &computed.results[5] else {
        panic!("probability output")
    };
    s1["operations"]
        .as_array_mut()
        .unwrap()
        .push(json!({"method":"infer_beta","artifact":probability.artifact}));
    let evaluation: Value = serde_json::from_str(include_str!(
        "../../../docs/usage/evaluation-request-v1.json"
    ))?;
    let validation: Value = serde_json::from_str(include_str!(
        "../../../docs/usage/validation-request-v1.json"
    ))?;
    let portfolio: Value = serde_json::from_str(include_str!(
        "../../../docs/usage/portfolio-request-v1.json"
    ))?;
    let mut requests = vec![s1, evaluation, validation, portfolio];
    let paper_requests: Vec<Value> =
        serde_json::from_str(include_str!("../../../docs/usage/paper-requests-v1.json"))?;
    requests.extend(paper_requests);
    for args in requests {
        for operation in args["operations"].as_array().unwrap() {
            seen.insert(operation["method"].as_str().unwrap().to_owned());
        }
        let native: analysis::Request = serde_json::from_value(args.clone())?;
        assert_eq!(
            success(
                client
                    .call_tool(call("analysis_batch_calculate", args))
                    .await?
            ),
            serde_json::to_value(analysis::calculate(&native)?)?
        );
    }
    let discovered = success(
        client
            .call_tool(CallToolRequestParams::new("indicator_catalog"))
            .await?,
    );
    let advertised: BTreeSet<_> = discovered["analysis"]["capabilities"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|c| c["methods"].as_array().unwrap())
        .map(|m| m.as_str().unwrap().to_owned())
        .collect();
    assert_eq!(advertised, seen);
    assert_eq!(seen.len(), 21);
    // Guard against a future enum variant becoming callable but undocumented/untested.
    let schema = serde_json::to_value(schemars::schema_for!(analysis::Operation))?;
    let variants: BTreeSet<_> = schema["oneOf"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| {
            v["properties"]["method"]["const"]
                .as_str()
                .unwrap()
                .to_owned()
        })
        .collect();
    assert_eq!(variants, seen);
    client.cancel().await?;
    server.await??;
    Ok(())
}

#[tokio::test]
async fn stream_rejects_corrupt_future_duplicate_incompatible_and_busy_requests(
) -> anyhow::Result<()> {
    let (server_transport, client_transport) = tokio::io::duplex(65536);
    let gateway = IndicatorGateway::default();
    let probe = gateway.clone();
    let server = tokio::spawn(async move {
        gateway.serve(server_transport).await?.waiting().await?;
        anyhow::Ok(())
    });
    let client = ().serve(client_transport).await?;
    let bars = bars();
    let mut native = engine::Stream::new(identity(), "sma.5")?;
    native.update(&bars[0], 1000)?;
    let args = json!({"schema_version":1,"identity":identity(),"profile_id":"sma.5","as_of_ms":1000,
        "action":{"kind":"advance","snapshot":native.snapshot()?,"bars":&bars[1..3]}});
    let mut invalids = Vec::new();
    let mut bad = args.clone();
    bad["action"]["snapshot"]["checksum"] = json!("corrupt");
    invalids.push((bad, "corrupt_snapshot"));
    let mut bad = args.clone();
    bad["identity"]["data_version"] = json!("other");
    invalids.push((bad, "corrupt_snapshot"));
    let mut bad = args.clone();
    bad["profile_id"] = json!("ema.5");
    invalids.push((bad, "incompatible_snapshot"));
    let mut bad = args.clone();
    bad["action"]["bars"] = json!([bars[0]]);
    invalids.push((bad, "duplicate_or_unordered_bar"));
    let mut bad = args.clone();
    bad["action"] = json!({"kind":"inspect","snapshot":native.snapshot()?});
    bad["as_of_ms"] = json!(1);
    invalids.push((bad, "invalid_time"));
    let mut bad = args.clone();
    bad["schema_version"] = json!(9);
    invalids.push((bad, "invalid_parameter"));
    let mut bad = args.clone();
    bad["action"]["bars"] = json!([]);
    invalids.push((bad, "invalid_parameter"));
    let mut bad = args.clone();
    bad["action"]["snapshot"]["payload_hex"] = json!("0".repeat(stream::MAX_STATE_JSON_BYTES + 1));
    invalids.push((bad, "limit_exceeded"));
    let mut bad = args.clone();
    bad["action"]["bars"] = json!(vec![bars[1].clone(); catalog::MAX_BARS + 1]);
    invalids.push((bad, "limit_exceeded"));
    for (bad, code) in invalids {
        let result = client.call_tool(call("indicator_stream", bad)).await?;
        assert_eq!(result.is_error, Some(true));
        assert_eq!(result.structured_content.unwrap()["error"]["code"], code);
    }
    let permits = probe.permits.clone().acquire_many_owned(2).await?;
    let blocked = client
        .call_tool(call("indicator_stream", args.clone()))
        .await?;
    assert_eq!(
        blocked.structured_content.unwrap()["error"]["code"],
        "limit_exceeded"
    );
    drop(permits);
    success(client.call_tool(call("indicator_stream", args)).await?);
    client.cancel().await?;
    server.await??;
    Ok(())
}

#[test]
fn cancelled_or_timed_out_stream_does_not_consume_callers_snapshot() -> anyhow::Result<()> {
    let mut native = engine::Stream::new(identity(), "ema.5")?;
    let bars = bars();
    native.update(&bars[0], 1000)?;
    let request = stream::Request {
        schema_version: 1,
        identity: identity(),
        profile_id: "ema.5".into(),
        as_of_ms: 1000,
        action: stream::Action::Advance {
            snapshot: native.snapshot()?,
            bars: bars[1..].to_vec(),
            output: engine::OutputMode::Series,
        },
    };
    let original = serde_json::to_value(&request)?;
    for code in [ErrorCode::Cancelled, ErrorCode::TimedOut] {
        let mut calls = 0;
        let error = stream::calculate_controlled(&request, || {
            calls += 1;
            // Past the preflight: interrupt while advancing the private stream.
            if calls == 310 {
                Err(TaError::new(code, "test interruption"))
            } else {
                Ok(())
            }
        })
        .unwrap_err();
        assert_eq!(error.code, code);
        assert_eq!(serde_json::to_value(&request)?, original);
    }
    let result = stream::calculate_controlled(&request, || Ok(()))?;
    for bar in &bars[1..] {
        native.update(bar, 1000)?;
    }
    assert_eq!(
        serde_json::to_value(result.snapshot)?,
        serde_json::to_value(native.snapshot()?)?
    );
    let mut unknown = original;
    unknown["action"]["command"] = json!("arbitrary code");
    assert!(serde_json::from_value::<stream::Request>(unknown).is_err());
    Ok(())
}
