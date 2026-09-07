//! Versioned, bounded wire access to the maintained reference algorithms.
//! Explicit typed inputs are checked before consuming any algorithm state.
mod compat;
mod extras;
mod input;
mod registry;
mod talib;

use crate::{
    engine::SeriesIdentity,
    error::{ErrorCode, TaError},
    fingerprint, wickra_all as w,
};
use input::WireInput;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::panic::{catch_unwind, AssertUnwindSafe};

pub const VERSION: &str = "roze-ta-reference-all-v1";
pub const MAX_SAMPLES: usize = 4096;
pub const MAX_ITEMS: usize = 128;
pub const MAX_BYTES: usize = 4 * 1024 * 1024;

fn invalid(message: &str) -> TaError {
    TaError::new(ErrorCode::InvalidParameter, message)
}
fn limit(message: &str) -> TaError {
    TaError::new(ErrorCode::LimitExceeded, message)
}
fn upstream(error: w::Error) -> TaError {
    invalid(&error.to_string())
}
fn encode<T: Serialize>(value: T) -> Result<Value, TaError> {
    serde_json::to_value(value).map_err(|_| {
        TaError::new(
            ErrorCode::EncodingFailed,
            "reference output encoding failed",
        )
    })
}
fn bytes<T: Serialize>(value: &T) -> Result<usize, TaError> {
    serde_json::to_vec(value)
        .map(|v| v.len())
        .map_err(|_| TaError::new(ErrorCode::EncodingFailed, "reference encoding failed"))
}
fn has_null(value: &Value) -> bool {
    match value {
        Value::Null => true,
        Value::Array(a) => a.iter().any(has_null),
        Value::Object(o) => o.values().any(has_null),
        _ => false,
    }
}

trait Runner: Send {
    fn validate(&self, sample: &Sample) -> Result<(), TaError>;
    fn step(&mut self, sample: &Sample) -> Result<Option<Value>, TaError>;
    fn warmup(&self) -> usize;
    fn clone_box(&self) -> Box<dyn Runner>;
}
struct IndicatorRunner<I>(I);
impl<I> Runner for IndicatorRunner<I>
where
    I: w::Indicator + Clone + Send + 'static,
    I::Input: WireInput,
    I::Output: Serialize,
{
    fn validate(&self, sample: &Sample) -> Result<(), TaError> {
        input::decode::<I::Input>(&sample.value, sample.at_ms).map(|_| ())
    }
    fn step(&mut self, sample: &Sample) -> Result<Option<Value>, TaError> {
        self.0
            .update(input::decode(&sample.value, sample.at_ms)?)
            .map(encode)
            .transpose()
    }
    fn warmup(&self) -> usize {
        self.0.warmup_period()
    }
    fn clone_box(&self) -> Box<dyn Runner> {
        Box::new(Self(self.0.clone()))
    }
}
fn indicator<I>(value: I) -> Box<dyn Runner>
where
    I: w::Indicator + Clone + Send + 'static,
    I::Input: WireInput,
    I::Output: Serialize,
{
    Box::new(IndicatorRunner(value))
}
struct BuilderRunner<I>(I);
impl<I> Runner for BuilderRunner<I>
where
    I: w::BarBuilder + Clone + Send + 'static,
    I::Bar: Serialize,
{
    fn validate(&self, sample: &Sample) -> Result<(), TaError> {
        input::decode::<w::Candle>(&sample.value, sample.at_ms).map(|_| ())
    }
    fn step(&mut self, sample: &Sample) -> Result<Option<Value>, TaError> {
        let out = self.0.update(input::decode(&sample.value, sample.at_ms)?);
        if out.len() > MAX_ITEMS {
            return Err(limit("builder emission exceeds 128 bars"));
        }
        if out.is_empty() {
            Ok(None)
        } else {
            encode(out).map(Some)
        }
    }
    fn warmup(&self) -> usize {
        1
    }
    fn clone_box(&self) -> Box<dyn Runner> {
        Box::new(Self(self.0.clone()))
    }
}
fn builder<I>(value: I) -> Box<dyn Runner>
where
    I: w::BarBuilder + Clone + Send + 'static,
    I::Bar: Serialize,
{
    Box::new(BuilderRunner(value))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct Operation {
    pub id: String,
    /// Explicit constructor parameters, using names and types in reference_catalog.
    pub params: Value,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct Sample {
    pub at_ms: i64,
    pub available_at_ms: i64,
    pub value: Value,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub schema_version: u32,
    pub identity: SeriesIdentity,
    pub operation: Operation,
    pub as_of_ms: i64,
    pub samples: Vec<Sample>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Ready,
    WarmingUp,
    Pending,
    UndefinedResult,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Row {
    pub at_ms: i64,
    pub available_at_ms: i64,
    pub samples_seen: usize,
    pub status: Status,
    pub value: Option<Value>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct Snapshot {
    pub schema_version: u32,
    pub implementation_version: String,
    pub identity: SeriesIdentity,
    pub operation: Operation,
    pub history: Vec<Sample>,
    pub checksum: String,
}

/// Live updates use the native state machine. Snapshots store a bounded replay
/// history so restoration never deserializes unchecked private algorithm state.
pub struct Stream {
    identity: SeriesIdentity,
    operation: Operation,
    runner: Box<dyn Runner>,
    history: Vec<Sample>,
    retained_bytes: usize,
    latest: Option<Row>,
}
impl Stream {
    pub fn new(identity: SeriesIdentity, operation: Operation) -> Result<Self, TaError> {
        identity.validate()?;
        registry::validate_params(&operation)?;
        let runner = registry::build(&operation)?;
        Ok(Self {
            identity,
            operation,
            runner,
            history: Vec::new(),
            retained_bytes: 0,
            latest: None,
        })
    }
    pub fn latest(&self) -> Option<&Row> {
        self.latest.as_ref()
    }
    pub fn reset(&mut self) -> Result<(), TaError> {
        *self = Self::new(self.identity.clone(), self.operation.clone())?;
        Ok(())
    }
    pub fn update(&mut self, sample: &Sample, as_of_ms: i64) -> Result<Row, TaError> {
        self.update_controlled(sample, as_of_ms, &mut || Ok(()))
    }
    pub fn update_with_checkpoint(
        &mut self,
        sample: &Sample,
        as_of_ms: i64,
        check: &mut dyn FnMut() -> Result<(), TaError>,
    ) -> Result<Row, TaError> {
        self.update_controlled(sample, as_of_ms, check)
    }
    fn update_controlled(
        &mut self,
        sample: &Sample,
        as_of_ms: i64,
        check: &mut dyn FnMut() -> Result<(), TaError>,
    ) -> Result<Row, TaError> {
        check()?;
        validate_sample(sample, as_of_ms, self.history.last())?;
        if self.history.len() >= MAX_SAMPLES {
            return Err(limit("stream replay history exceeds 4096 samples"));
        }
        let added_bytes = bytes(sample)?;
        if self.retained_bytes + added_bytes > MAX_BYTES {
            return Err(limit("stream replay history exceeds 4 MiB"));
        }
        self.runner.validate(sample)?;
        registry::validate_domain(&self.operation, sample)?;
        registry::preflight_emission(&self.operation, sample, self.history.last())?;
        // Work on a private candidate: rejected output, cancellation and panics
        // cannot leave the caller's stream partially advanced.
        let mut candidate = self.runner.clone_box();
        let value = catch_unwind(AssertUnwindSafe(|| candidate.step(sample))).map_err(|_| {
            TaError::new(
                ErrorCode::UpstreamFailure,
                "reference algorithm failed; state unchanged",
            )
        })??;
        if bytes(&value)? > 64 * 1024 {
            return Err(limit("reference row exceeds 64 KiB"));
        }
        check()?;
        let seen = self.history.len() + 1;
        let status = match &value {
            Some(v) if has_null(v) => Status::UndefinedResult,
            Some(_) => Status::Ready,
            None if seen < candidate.warmup() => Status::WarmingUp,
            None => Status::Pending,
        };
        let row = Row {
            at_ms: sample.at_ms,
            available_at_ms: sample.available_at_ms,
            samples_seen: seen,
            status,
            value,
        };
        self.runner = candidate;
        self.history.push(sample.clone());
        self.retained_bytes += added_bytes;
        self.latest = Some(row.clone());
        Ok(row)
    }
    pub fn snapshot(&self) -> Result<Snapshot, TaError> {
        let mut value = Snapshot {
            schema_version: 1,
            implementation_version: VERSION.into(),
            identity: self.identity.clone(),
            operation: self.operation.clone(),
            history: self.history.clone(),
            checksum: String::new(),
        };
        value.checksum = snapshot_hash(&value)?;
        Ok(value)
    }
    pub fn restore(
        snapshot: &Snapshot,
        identity: &SeriesIdentity,
        operation: &Operation,
    ) -> Result<Self, TaError> {
        Self::restore_controlled(snapshot, identity, operation, &mut || Ok(()))
    }
    pub fn restore_controlled(
        snapshot: &Snapshot,
        identity: &SeriesIdentity,
        operation: &Operation,
        check: &mut dyn FnMut() -> Result<(), TaError>,
    ) -> Result<Self, TaError> {
        if snapshot.schema_version != 1
            || snapshot.implementation_version != VERSION
            || fingerprint::digest(&snapshot.identity)? != fingerprint::digest(identity)?
            || fingerprint::digest(&snapshot.operation)? != fingerprint::digest(operation)?
        {
            return Err(TaError::new(
                ErrorCode::IncompatibleSnapshot,
                "reference snapshot version, identity or operation mismatch",
            ));
        }
        if snapshot.history.len() > MAX_SAMPLES || bytes(&snapshot.history)? > MAX_BYTES {
            return Err(limit("snapshot history exceeds replay limits"));
        }
        if snapshot.checksum != snapshot_hash(snapshot)? {
            return Err(TaError::new(
                ErrorCode::CorruptSnapshot,
                "reference snapshot checksum mismatch",
            ));
        }
        let mut result = Self::new(identity.clone(), operation.clone())?;
        let as_of = snapshot.history.last().map_or(1, |s| s.available_at_ms);
        for sample in &snapshot.history {
            result.update_controlled(sample, as_of, check)?;
        }
        Ok(result)
    }
}
fn snapshot_hash(s: &Snapshot) -> Result<String, TaError> {
    fingerprint::digest(&(
        s.schema_version,
        &s.implementation_version,
        &s.identity,
        &s.operation,
        &s.history,
    ))
}
fn validate_sample(
    sample: &Sample,
    as_of_ms: i64,
    previous: Option<&Sample>,
) -> Result<(), TaError> {
    if sample.at_ms <= 0
        || sample.available_at_ms < sample.at_ms
        || sample.available_at_ms > as_of_ms
    {
        return Err(TaError::new(
            ErrorCode::InvalidTime,
            "sample must be closed and available at as_of_ms",
        ));
    }
    if previous
        .is_some_and(|p| sample.at_ms <= p.at_ms || sample.available_at_ms < p.available_at_ms)
    {
        return Err(TaError::new(
            ErrorCode::DuplicateOrUnorderedBar,
            "duplicate, unordered or retroactive sample",
        ));
    }
    input::validate_json(&sample.value, 0)
}
pub fn catalog() -> Result<Value, TaError> {
    registry::catalog()
}
pub fn calculate(request: &Request) -> Result<Value, TaError> {
    calculate_controlled(request, || Ok(()))
}
pub fn calculate_controlled(
    request: &Request,
    mut check: impl FnMut() -> Result<(), TaError>,
) -> Result<Value, TaError> {
    if request.schema_version != 1 {
        return Err(invalid("unsupported reference schema_version"));
    }
    if request.samples.is_empty()
        || request.samples.len() > MAX_SAMPLES
        || bytes(&request.samples)? > MAX_BYTES
    {
        return Err(limit("require 1..4096 samples within 4 MiB"));
    }
    let mut stream = Stream::new(request.identity.clone(), request.operation.clone())?;
    let mut rows = Vec::with_capacity(request.samples.len());
    let mut row_bytes = 0;
    for sample in &request.samples {
        let row = stream.update_controlled(sample, request.as_of_ms, &mut check)?;
        row_bytes += bytes(&row)?;
        if row_bytes > MAX_BYTES {
            return Err(limit("reference results exceed 4 MiB"));
        }
        rows.push(row);
    }
    let input_hash = fingerprint::digest(&(VERSION, request))?;
    let output_hash = fingerprint::digest(&(VERSION, &input_hash, &rows))?;
    Ok(
        json!({"schema_version":1,"implementation_version":VERSION,"identity":request.identity,
        "operation":request.operation,"as_of_ms":request.as_of_ms,"input_hash":input_hash,"output_hash":output_hash,
        "quality_flags":[if request.operation.id.starts_with("compat.") {"source_specific_formula_variant"} else if request.operation.id.starts_with("talib.") {"talib_formula_variant"} else {"wickra_formula_variant"},"source_neutral_value_conventions","timestamps_are_emission_times"],"rows":rows}),
    )
}
