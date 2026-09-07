use rmcp::{
    handler::server::wrapper::Parameters, model::CallToolResult, service::RequestContext, tool,
    tool_router, transport::stdio, RoleServer, ServiceExt,
};
use roze_ta::catalog::{self, BatchRequest};
use roze_ta::{
    analysis, engine,
    error::{ErrorCode, TaError},
    native,
};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::Semaphore;
mod bounded;
#[cfg(test)]
mod coverage_tests;
#[cfg(test)]
mod native_tests;
mod reference;
#[cfg(test)]
mod reference_tests;
mod stream;

const MAX_RESPONSE_BYTES: usize = 8 * 1024 * 1024;
const COMPUTE_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Clone)]
struct IndicatorGateway {
    permits: Arc<Semaphore>,
}
impl Default for IndicatorGateway {
    fn default() -> Self {
        Self {
            permits: Arc::new(Semaphore::new(2)),
        }
    }
}

fn tool_result(value: Result<serde_json::Value, TaError>) -> CallToolResult {
    match value {
        Err(error) => CallToolResult::structured_error(serde_json::json!({"error":error})),
        Ok(value) => {
            let result = CallToolResult::structured(value);
            match serde_json::to_vec(&result) {
                Ok(bytes) if bytes.len() <= MAX_RESPONSE_BYTES => result,
                _ => CallToolResult::structured_error(
                    serde_json::json!({"error":{"code":"limit_exceeded","message":"response exceeds 8 MiB; request latest values or a smaller batch"}}),
                ),
            }
        }
    }
}

#[tool_router(server_handler)]
impl IndicatorGateway {
    #[tool(
        description = "Discover maintained reference algorithm variants, explicit constructor parameter schemas and rich input types. Includes OHLCV, scalar, paired, order-book, trade, derivatives and cross-section inputs. Source neutral values are documented variants, not TA-Lib compatibility.",
        annotations(
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    fn reference_catalog(&self) -> CallToolResult {
        tool_result(roze_ta::reference_all::catalog())
    }

    #[tool(
        description = "Calculate a reference algorithm selected from reference_catalog on timed, typed caller data. Parameters are explicit; no data acquisition or trading. Supports bounded full-series output and source formula variants.",
        annotations(
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn reference_batch_calculate(
        &self,
        request: Parameters<roze_ta::reference_all::Request>,
        context: RequestContext<RoleServer>,
    ) -> CallToolResult {
        reference::calculate(self, request, context).await
    }

    #[tool(
        description = "Create, advance, inspect or reset reference algorithm streams using caller-owned bounded replay snapshots. Restore validates identity, configuration, checksum, inputs and timestamps. No server persistence or trading.",
        annotations(
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn reference_stream(
        &self,
        request: Parameters<reference::Request>,
        context: RequestContext<RoleServer>,
    ) -> CallToolResult {
        reference::streaming(self, request, context).await
    }

    #[tool(
        description = "List registered read-only indicator profiles, resolved parameters, units and minimum completed-bar requirements.",
        annotations(
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    fn indicator_catalog(&self) -> CallToolResult {
        let profiles: Vec<_> = catalog::catalog()
            .into_iter()
            .map(|p| {
                let mut value = serde_json::json!(p);
                if let Some(object) = value.as_object_mut() {
                    object.insert("capability_kind".into(), serde_json::json!("indicator"));
                    object.insert("data_type".into(), serde_json::json!("completed_ohlcv"));
                    object.insert("fits".into(), serde_json::json!(false));
                    object.insert("random".into(), serde_json::json!(false));
                    object.insert("online_update".into(), serde_json::json!(true));
                }
                value
            })
            .collect();
        CallToolResult::structured(
            serde_json::json!({"version":catalog::VERSION,"profiles":profiles,"analysis":analysis::catalog(),
                "engine_schema_version":engine::SCHEMA_VERSION,"engine_version":engine::IMPLEMENTATION,
                "capabilities":{"streaming":true,"snapshot_restore":true,"full_series":true,"v2_tool":"indicator_batch_calculate_v2",
                    "stream_tool":"indicator_stream","stream_schema_version":stream::SCHEMA_VERSION,
                    "stream_actions":["create","advance","inspect","reset"],"stream_state":"caller_owned_snapshot",
                    "analysis_tool":"analysis_batch_calculate","parameter_schema":"tools/list.inputSchema",
                    "native_catalog_tool":"native_catalog","native_calculate_tool":"native_batch_calculate"},
                "limits":{"bars":catalog::MAX_BARS,"profiles":catalog::MAX_PROFILES,"result_rows":engine::MAX_RESULT_ROWS,
                    "request_bytes":bounded::MAX_REQUEST_BYTES,"response_bytes":MAX_RESPONSE_BYTES,
                    "stream_snapshot_json_bytes":stream::MAX_STATE_JSON_BYTES,"concurrent_calculations":2,"compute_timeout_seconds":10}}),
        )
    }

    #[tool(
        description = "Discover every implemented native indicator and Method, including aliases, parameter schemas/defaults, input kinds, source documentation and output semantics. Native seeded output is distinct from audited Profile readiness.",
        annotations(
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    fn native_catalog(&self) -> CallToolResult {
        tool_result(native::catalog())
    }

    #[tool(
        description = "Calculate any native indicator or Method selected from native_catalog, including unregistered indicators, smoothers, extrema, crossover/reversal signals, Heikin-Ashi, bounded Renko and count-based bar aggregation. Supports bars, timed scalars, pairs or JSON values (Past). Preserves native initialization/units; returns explicit nonfinite/pending statuses and emission timestamps. No trading, I/O or server state.",
        annotations(
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn native_batch_calculate(
        &self,
        Parameters(request): Parameters<native::Request>,
        context: RequestContext<RoleServer>,
    ) -> CallToolResult {
        let Ok(permit) = self.permits.clone().try_acquire_owned() else {
            return tool_result(Err(TaError::new(
                ErrorCode::LimitExceeded,
                "calculation concurrency limit exceeded",
            )));
        };
        let token = context.ct.child_token();
        let _cancel_on_drop = token.clone().drop_guard();
        let deadline = Instant::now() + COMPUTE_TIMEOUT;
        tokio::task::spawn_blocking(move || {
            let _permit = permit;
            let result = native::calculate_controlled(&request, || {
                if token.is_cancelled() {
                    Err(TaError::new(
                        ErrorCode::Cancelled,
                        "native calculation cancelled",
                    ))
                } else if Instant::now() >= deadline {
                    Err(TaError::new(
                        ErrorCode::TimedOut,
                        "native calculation deadline exceeded",
                    ))
                } else {
                    Ok(())
                }
            });
            tool_result(result.and_then(|v| {
                serde_json::to_value(v).map_err(|_| {
                    TaError::new(ErrorCode::EncodingFailed, "native result encoding failed")
                })
            }))
        })
        .await
        .unwrap_or_else(|_| {
            tool_result(Err(TaError::new(
                ErrorCode::UpstreamFailure,
                "native worker failed",
            )))
        })
    }

    #[tool(
        description = "Calculate a bounded batch of indicators on caller-supplied completed OHLCV bars. Pure calculation; no market fetching, account access or order execution. Select profile IDs from indicator_catalog.",
        annotations(
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn indicator_batch_calculate(
        &self,
        Parameters(request): Parameters<BatchRequest>,
        context: RequestContext<RoleServer>,
    ) -> CallToolResult {
        let Ok(permit) = self.permits.clone().try_acquire_owned() else {
            return CallToolResult::structured_error(
                serde_json::json!({"error":"calculation concurrency limit exceeded"}),
            );
        };
        let token = context.ct.child_token();
        let _cancel_on_drop = token.clone().drop_guard();
        let deadline = Instant::now() + COMPUTE_TIMEOUT;
        let job = tokio::task::spawn_blocking(move || {
            let _permit = permit;
            catalog::calculate_controlled(&request, || {
                if token.is_cancelled() {
                    Err("cancelled".into())
                } else if Instant::now() >= deadline {
                    Err("timed_out".into())
                } else {
                    Ok(())
                }
            })
        })
        .await;
        match job {
            Ok(Ok(result)) => tool_result(
                serde_json::to_value(result)
                    .map_err(|_| TaError::new(ErrorCode::EncodingFailed, "result encoding failed")),
            ),
            Ok(Err(error)) => CallToolResult::structured_error(serde_json::json!({"error":error})),
            Err(_) => CallToolResult::structured_error(
                serde_json::json!({"error":"calculation worker failed"}),
            ),
        }
    }

    #[tool(
        description = "Version 2 bounded calculation: explicit data identity and availability, latest or full series, typed statuses and canonical hashes. Uses the same streaming engine as native Rust. No I/O or trading.",
        annotations(
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn indicator_batch_calculate_v2(
        &self,
        Parameters(request): Parameters<engine::BatchRequest>,
        context: RequestContext<RoleServer>,
    ) -> CallToolResult {
        let Ok(permit) = self.permits.clone().try_acquire_owned() else {
            return tool_result(Err(TaError::new(
                ErrorCode::LimitExceeded,
                "calculation concurrency limit exceeded",
            )));
        };
        let token = context.ct.child_token();
        let _cancel_on_drop = token.clone().drop_guard();
        let deadline = Instant::now() + COMPUTE_TIMEOUT;
        let job = tokio::task::spawn_blocking(move || {
            let _permit = permit;
            let result = engine::calculate_controlled(&request, || {
                if token.is_cancelled() {
                    Err(TaError::new(ErrorCode::Cancelled, "calculation cancelled"))
                } else if Instant::now() >= deadline {
                    Err(TaError::new(
                        ErrorCode::TimedOut,
                        "calculation deadline exceeded",
                    ))
                } else {
                    Ok(())
                }
            });
            tool_result(result.and_then(|result| {
                serde_json::to_value(result)
                    .map_err(|_| TaError::new(ErrorCode::EncodingFailed, "result encoding failed"))
            }))
        })
        .await;
        job.unwrap_or_else(|_| {
            tool_result(Err(TaError::new(
                ErrorCode::UpstreamFailure,
                "calculation worker failed",
            )))
        })
    }

    #[tool(
        description = "Stateless streaming for any registered indicator profile. Create, advance, inspect or reset caller-owned snapshots using the native engine. Returns latest, optional new series rows and a resumable snapshot. No server sessions, persistent writes, market fetching or trading.",
        annotations(
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn indicator_stream(
        &self,
        Parameters(request): Parameters<stream::Request>,
        context: RequestContext<RoleServer>,
    ) -> CallToolResult {
        let Ok(permit) = self.permits.clone().try_acquire_owned() else {
            return tool_result(Err(TaError::new(
                ErrorCode::LimitExceeded,
                "calculation concurrency limit exceeded",
            )));
        };
        let token = context.ct.child_token();
        let _cancel_on_drop = token.clone().drop_guard();
        let deadline = Instant::now() + COMPUTE_TIMEOUT;
        tokio::task::spawn_blocking(move || {
            let _permit = permit;
            let result = stream::calculate_controlled(&request, || {
                if token.is_cancelled() {
                    Err(TaError::new(
                        ErrorCode::Cancelled,
                        "stream calculation cancelled",
                    ))
                } else if Instant::now() >= deadline {
                    Err(TaError::new(
                        ErrorCode::TimedOut,
                        "stream calculation deadline exceeded",
                    ))
                } else {
                    Ok(())
                }
            });
            tool_result(result.and_then(|value| {
                serde_json::to_value(value).map_err(|_| {
                    TaError::new(ErrorCode::EncodingFailed, "stream result encoding failed")
                })
            }))
        })
        .await
        .unwrap_or_else(|_| {
            tool_result(Err(TaError::new(
                ErrorCode::UpstreamFailure,
                "stream worker failed",
            )))
        })
    }

    #[tool(
        description = "Bounded versioned analysis: statistics, probability, net-return risk, fixed-exposure portfolio covariance/concentration/risk contributions and explicit stress scenarios, trade/factor evaluation, frozen-forecast calibration scores, seeded IID/block bootstrap of the mean, and chronological label-purged partitions. Select methods from indicator_catalog analysis capabilities. No data fetching or persistent writes.",
        annotations(
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn analysis_batch_calculate(
        &self,
        Parameters(request): Parameters<analysis::Request>,
        context: RequestContext<RoleServer>,
    ) -> CallToolResult {
        let Ok(permit) = self.permits.clone().try_acquire_owned() else {
            return tool_result(Err(TaError::new(
                ErrorCode::LimitExceeded,
                "calculation concurrency limit exceeded",
            )));
        };
        let token = context.ct.child_token();
        let _cancel_on_drop = token.clone().drop_guard();
        let deadline = Instant::now() + COMPUTE_TIMEOUT;
        tokio::task::spawn_blocking(move || {
            let _permit = permit;
            let result = analysis::calculate_controlled(&request, || {
                if token.is_cancelled() {
                    Err(TaError::new(ErrorCode::Cancelled, "analysis cancelled"))
                } else if Instant::now() >= deadline {
                    Err(TaError::new(
                        ErrorCode::TimedOut,
                        "analysis deadline exceeded",
                    ))
                } else {
                    Ok(())
                }
            });
            tool_result(result.and_then(|value| {
                serde_json::to_value(value).map_err(|_| {
                    TaError::new(ErrorCode::EncodingFailed, "analysis encoding failed")
                })
            }))
        })
        .await
        .unwrap_or_else(|_| {
            tool_result(Err(TaError::new(
                ErrorCode::UpstreamFailure,
                "analysis worker failed",
            )))
        })
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let (input, output) = stdio();
    let service = IndicatorGateway::default()
        .serve((bounded::BoundedLines::new(input), output))
        .await?;
    service.waiting().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rmcp::model::CallToolRequestParams;
    #[tokio::test]
    async fn mcp_negotiates_lists_and_calls_read_only_tools() -> anyhow::Result<()> {
        let (server_transport, client_transport) = tokio::io::duplex(65536);
        let gateway = IndicatorGateway::default();
        let probe = gateway.clone();
        let server = tokio::spawn(async move {
            gateway.serve(server_transport).await?.waiting().await?;
            anyhow::Ok(())
        });
        let client = ().serve(client_transport).await?;
        let tools = client.list_all_tools().await?;
        assert_eq!(tools.len(), 10);
        assert!(tools
            .iter()
            .all(|t| t.annotations.as_ref().and_then(|a| a.read_only_hint) == Some(true)));
        let result = client
            .call_tool(CallToolRequestParams::new("indicator_catalog"))
            .await?;
        assert_eq!(
            result.structured_content.unwrap()["profiles"]
                .as_array()
                .unwrap()
                .len(),
            50
        );
        let args = serde_json::json!({"snapshot_id":"s1","symbol":"TEST","timeframe":"1m","as_of_ms":10000,"profiles":["sma.5"],"bars":(1..=5).map(|i|serde_json::json!({"closed_at_ms":i*1000,"open":100+i,"high":102+i,"low":99+i,"close":101+i,"volume":100})).collect::<Vec<_>>()});
        let result = client
            .call_tool(
                CallToolRequestParams::new("indicator_batch_calculate")
                    .with_arguments(args.as_object().unwrap().clone()),
            )
            .await?;
        assert_eq!(result.is_error, Some(false));
        let actual = result.structured_content.unwrap();
        let request: BatchRequest = serde_json::from_value(args.clone())?;
        assert_eq!(
            actual["output_hash"],
            catalog::calculate(&request).unwrap().output_hash
        );
        let mut bad = args;
        bad["profiles"] = serde_json::json!(["place_order"]);
        let denied = client
            .call_tool(
                CallToolRequestParams::new("indicator_batch_calculate")
                    .with_arguments(bad.as_object().unwrap().clone()),
            )
            .await?;
        assert_eq!(denied.is_error, Some(true));
        let v2 = serde_json::json!({
            "schema_version":2,"snapshot_id":"s1",
            "identity":{"series_id":"seq","instrument":"TEST","timeframe":"1m","source":"fixture","data_version":"1"},
            "as_of_ms":10000,"profiles":["sma.5"],"output":"series",
            "bars":bad["bars"].as_array().unwrap().iter().map(|bar|serde_json::json!({
                "candle":bar,"available_at_ms":bar["closed_at_ms"]
            })).collect::<Vec<_>>()
        });
        let result = client
            .call_tool(
                CallToolRequestParams::new("indicator_batch_calculate_v2")
                    .with_arguments(v2.as_object().unwrap().clone()),
            )
            .await?;
        assert_eq!(result.is_error, Some(false));
        let native: engine::BatchRequest = serde_json::from_value(v2.clone())?;
        assert_eq!(
            result.structured_content.unwrap(),
            serde_json::to_value(engine::calculate(&native)?)?
        );
        let mut a1 = v2.clone();
        a1["as_of_ms"] = serde_json::json!(100_000);
        a1["profiles"] = serde_json::json!([
            "wma.14",
            "rma.14",
            "dema.14",
            "tema.14",
            "vwma.14",
            "roc.14",
            "momentum.14",
            "ad.cumulative",
            "obv.zero_seed",
            "williams_r.14"
        ]);
        a1["bars"] = serde_json::json!((1..=50).map(|i|serde_json::json!({"candle":{"closed_at_ms":i*1000,"open":100+i,"high":103+i,"low":98+i,"close":101+i,"volume":i%7},"available_at_ms":i*1000+1})).collect::<Vec<_>>());
        let result = client
            .call_tool(
                CallToolRequestParams::new("indicator_batch_calculate_v2")
                    .with_arguments(a1.as_object().unwrap().clone()),
            )
            .await?;
        assert_eq!(result.is_error, Some(false));
        let native: engine::BatchRequest = serde_json::from_value(a1)?;
        assert_eq!(
            result.structured_content.unwrap(),
            serde_json::to_value(engine::calculate(&native)?)?
        );
        let analysis_args = serde_json::json!({
            "schema_version":1,"identity":{"series_id":"fixture","instrument":"TEST","timeframe":"1ms","source":"manual","data_version":"1"},
            "input_kind":"price","units":"unit","as_of_ms":10,"fit_cutoff_ms":10,
            "points":[{"at_ms":1,"available_at_ms":1,"x":1.0,"y":null},{"at_ms":2,"available_at_ms":2,"x":3.0,"y":null}],"events":[],
            "operations":[{"method":"describe","ddof":1,"quantiles":[0.5],"trim_fraction":0.0,"interval_level":0.95},
                {"method":"distribution","task":{"distribution":{"family":"normal","mean":0.0,"standard_deviation":1.0},"evaluate_at":[0.0],"quantiles":[0.5],"sampling":{"seed":42,"samples":4}}}]
        });
        let analysis_result = client
            .call_tool(
                CallToolRequestParams::new("analysis_batch_calculate")
                    .with_arguments(analysis_args.as_object().unwrap().clone()),
            )
            .await?;
        assert_eq!(analysis_result.is_error, Some(false));
        let native: analysis::Request = serde_json::from_value(analysis_args.clone())?;
        assert_eq!(
            analysis_result.structured_content.unwrap(),
            serde_json::to_value(analysis::calculate(&native)?)?
        );
        let mut bad_analysis = analysis_args.clone();
        bad_analysis["operations"][0]["ddof"] = serde_json::json!(2);
        let denied = client
            .call_tool(
                CallToolRequestParams::new("analysis_batch_calculate")
                    .with_arguments(bad_analysis.as_object().unwrap().clone()),
            )
            .await?;
        assert_eq!(
            denied.structured_content.unwrap()["error"]["code"],
            "invalid_parameter"
        );
        let permits = probe.permits.clone().acquire_many_owned(2).await?;
        let blocked = client
            .call_tool(
                CallToolRequestParams::new("analysis_batch_calculate")
                    .with_arguments(analysis_args.as_object().unwrap().clone()),
            )
            .await?;
        assert_eq!(
            blocked.structured_content.unwrap()["error"]["code"],
            "limit_exceeded"
        );
        let blocked = client
            .call_tool(
                CallToolRequestParams::new("indicator_batch_calculate_v2")
                    .with_arguments(v2.as_object().unwrap().clone()),
            )
            .await?;
        assert_eq!(
            blocked.structured_content.unwrap()["error"]["code"],
            "limit_exceeded"
        );
        drop(permits);
        let mut invalid = v2;
        invalid["profiles"] = serde_json::json!(["place_order"]);
        let denied = client
            .call_tool(
                CallToolRequestParams::new("indicator_batch_calculate_v2")
                    .with_arguments(invalid.as_object().unwrap().clone()),
            )
            .await?;
        assert_eq!(
            denied.structured_content.unwrap()["error"]["code"],
            "unsupported_profile"
        );
        client.cancel().await?;
        server.await??;
        Ok(())
    }

    #[test]
    fn response_size_is_bounded() {
        let result = tool_result(Ok(
            serde_json::json!({"payload":"x".repeat(MAX_RESPONSE_BYTES)}),
        ));
        assert_eq!(result.is_error, Some(true));
        assert_eq!(
            result.structured_content.unwrap()["error"]["code"],
            "limit_exceeded"
        );
    }

    #[tokio::test]
    async fn evaluation_matches_native_and_rejects_invalid_parameters() -> anyhow::Result<()> {
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
        let mut args: serde_json::Value = serde_json::from_str(include_str!(
            "../../../docs/usage/evaluation-request-v1.json"
        ))?;
        let request: analysis::Request = serde_json::from_value(args.clone())?;
        let result = client
            .call_tool(
                CallToolRequestParams::new("analysis_batch_calculate")
                    .with_arguments(args.as_object().unwrap().clone()),
            )
            .await?;
        assert_eq!(result.is_error, Some(false));
        assert_eq!(
            result.structured_content.unwrap(),
            serde_json::to_value(analysis::calculate(&request)?)?
        );
        args["operations"][0]["spec"]["periods_per_year"] = serde_json::json!(0);
        let denied = client
            .call_tool(
                CallToolRequestParams::new("analysis_batch_calculate")
                    .with_arguments(args.as_object().unwrap().clone()),
            )
            .await?;
        assert_eq!(denied.is_error, Some(true));
        assert_eq!(
            denied.structured_content.unwrap()["error"]["code"],
            "invalid_parameter"
        );
        client.cancel().await?;
        server.await??;
        Ok(())
    }

    #[tokio::test]
    async fn validation_operations_match_native_and_enforce_training_boundary() -> anyhow::Result<()>
    {
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
        let mut args: serde_json::Value = serde_json::from_str(include_str!(
            "../../../docs/usage/validation-request-v1.json"
        ))?;
        let native: analysis::Request = serde_json::from_value(args.clone())?;
        let result = client
            .call_tool(
                CallToolRequestParams::new("analysis_batch_calculate")
                    .with_arguments(args.as_object().unwrap().clone()),
            )
            .await?;
        assert_eq!(result.is_error, Some(false));
        assert_eq!(
            result.structured_content.unwrap(),
            serde_json::to_value(analysis::calculate(&native)?)?
        );
        args["operations"][0]["spec"]["baseline_fit_cutoff_ms"] = serde_json::json!(10);
        let denied = client
            .call_tool(
                CallToolRequestParams::new("analysis_batch_calculate")
                    .with_arguments(args.as_object().unwrap().clone()),
            )
            .await?;
        assert_eq!(denied.is_error, Some(true));
        assert_eq!(
            denied.structured_content.unwrap()["error"]["code"],
            "invalid_time"
        );
        client.cancel().await?;
        server.await??;
        Ok(())
    }
}
