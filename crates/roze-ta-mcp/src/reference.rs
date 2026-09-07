use crate::{tool_result, IndicatorGateway, COMPUTE_TIMEOUT};
use rmcp::{
    handler::server::wrapper::Parameters, model::CallToolResult, service::RequestContext,
    RoleServer,
};
use roze_ta::{
    engine::SeriesIdentity,
    error::{ErrorCode, TaError},
    reference_all::{self, Operation, Sample, Snapshot, Stream},
};
use serde::{Deserialize, Serialize};
use std::time::Instant;

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("type" = "object"))]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
pub enum Request {
    Create {
        identity: SeriesIdentity,
        operation: Operation,
    },
    Advance {
        identity: SeriesIdentity,
        operation: Operation,
        snapshot: Snapshot,
        as_of_ms: i64,
        samples: Vec<Sample>,
    },
    Inspect {
        identity: SeriesIdentity,
        operation: Operation,
        snapshot: Snapshot,
    },
    Reset {
        identity: SeriesIdentity,
        operation: Operation,
        snapshot: Snapshot,
    },
}

fn stream(
    request: Request,
    check: &mut dyn FnMut() -> Result<(), TaError>,
) -> Result<serde_json::Value, TaError> {
    let state = match request {
        Request::Create {
            identity,
            operation,
        } => Stream::new(identity, operation)?,
        Request::Advance {
            identity,
            operation,
            snapshot,
            as_of_ms,
            samples,
        } => {
            if snapshot
                .history
                .last()
                .is_some_and(|s| s.available_at_ms > as_of_ms)
            {
                return Err(TaError::new(
                    ErrorCode::InvalidTime,
                    "snapshot contains samples not yet available at as_of_ms",
                ));
            }
            if samples.len() > reference_all::MAX_SAMPLES {
                return Err(TaError::new(
                    ErrorCode::LimitExceeded,
                    "advance exceeds 4096 samples",
                ));
            }
            let mut state = Stream::restore_controlled(&snapshot, &identity, &operation, check)?;
            for sample in samples {
                check()?;
                state.update_with_checkpoint(&sample, as_of_ms, check)?;
            }
            state
        }
        Request::Inspect {
            identity,
            operation,
            snapshot,
        } => Stream::restore_controlled(&snapshot, &identity, &operation, check)?,
        Request::Reset {
            identity,
            operation,
            snapshot,
        } => {
            let mut state = Stream::restore_controlled(&snapshot, &identity, &operation, check)?;
            state.reset()?;
            state
        }
    };
    check()?;
    Ok(serde_json::json!({"schema_version":1,"snapshot":state.snapshot()?,"latest":state.latest()}))
}

pub async fn calculate(
    gateway: &IndicatorGateway,
    request: Parameters<reference_all::Request>,
    context: RequestContext<RoleServer>,
) -> CallToolResult {
    run(gateway, context, move |check| {
        reference_all::calculate_controlled(&request.0, check)
    })
    .await
}
pub async fn streaming(
    gateway: &IndicatorGateway,
    request: Parameters<Request>,
    context: RequestContext<RoleServer>,
) -> CallToolResult {
    run(gateway, context, move |check| stream(request.0, check)).await
}
async fn run<F>(
    gateway: &IndicatorGateway,
    context: RequestContext<RoleServer>,
    operation: F,
) -> CallToolResult
where
    F: FnOnce(&mut dyn FnMut() -> Result<(), TaError>) -> Result<serde_json::Value, TaError>
        + Send
        + 'static,
{
    let Ok(permit) = gateway.permits.clone().try_acquire_owned() else {
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
        tool_result(operation(&mut || {
            if token.is_cancelled() {
                Err(TaError::new(
                    ErrorCode::Cancelled,
                    "reference calculation cancelled",
                ))
            } else if Instant::now() >= deadline {
                Err(TaError::new(
                    ErrorCode::TimedOut,
                    "reference deadline exceeded",
                ))
            } else {
                Ok(())
            }
        }))
    })
    .await
    .unwrap_or_else(|_| {
        tool_result(Err(TaError::new(
            ErrorCode::UpstreamFailure,
            "reference worker failed",
        )))
    })
}
