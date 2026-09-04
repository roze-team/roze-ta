//! Bounded, versioned access to all implemented native indicators and Methods.
//! Native seeded values preserve the original API; they are not audited Profile readiness.
use crate::{
    core::{Action, Candle, IndicatorConfig, IndicatorInstance, Method, OHLCV},
    engine::{self, ClosedBar, OutputMode, SeriesIdentity},
    error::{ErrorCode, TaError},
    fingerprint,
};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::{json, Value};
use std::panic::{catch_unwind, AssertUnwindSafe};

pub const VERSION: &str = "roze-ta-native-api-v1";
pub const MAX_SAMPLES: usize = 4096;
pub const MAX_OPERATIONS: usize = 16;
pub const MAX_OUTPUT_ITEMS: usize = 65536;
pub const MAX_RENKO_BRICKS: usize = 4096;
pub const MAX_JSON_VALUE_BYTES: usize = 1024;
pub const MAX_RETAINED_ROW_BYTES: usize = 3 * 1024 * 1024;
type Check<'a> = dyn FnMut() -> Result<(), TaError> + 'a;
type Runner<'a> = Box<dyn FnMut(usize, &mut Check<'_>) -> Result<NativeValue, TaError> + 'a>;

fn invalid(message: &str) -> TaError {
    TaError::new(ErrorCode::InvalidParameter, message)
}
fn encode<T: Serialize>(value: T) -> Result<Value, TaError> {
    serde_json::to_value(value)
        .map_err(|_| TaError::new(ErrorCode::EncodingFailed, "native value encoding failed"))
}
fn decode<T: DeserializeOwned>(value: &Value) -> Result<T, TaError> {
    serde_json::from_value(value.clone())
        .map_err(|e| invalid(&format!("invalid native parameters: {e}")))
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct Timed<T> {
    pub at_ms: i64,
    pub available_at_ms: i64,
    pub value: T,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(
    tag = "kind",
    content = "samples",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum Input {
    Bars(Vec<ClosedBar>),
    Scalars(Vec<Timed<f64>>),
    Pairs(Vec<Timed<(f64, f64)>>),
    Values(Vec<Timed<Value>>),
}
impl Input {
    fn kind(&self) -> &'static str {
        match self {
            Self::Bars(_) => "bars",
            Self::Scalars(_) => "scalars",
            Self::Pairs(_) => "pairs",
            Self::Values(_) => "values",
        }
    }
    fn len(&self) -> usize {
        match self {
            Self::Bars(v) => v.len(),
            Self::Scalars(v) => v.len(),
            Self::Pairs(v) => v.len(),
            Self::Values(v) => v.len(),
        }
    }
    fn time(&self, i: usize) -> (i64, i64) {
        match self {
            Self::Bars(v) => (v[i].candle.closed_at_ms, v[i].available_at_ms),
            Self::Scalars(v) => (v[i].at_ms, v[i].available_at_ms),
            Self::Pairs(v) => (v[i].at_ms, v[i].available_at_ms),
            Self::Values(v) => (v[i].at_ms, v[i].available_at_ms),
        }
    }
    fn bars(&self) -> Result<&[ClosedBar], TaError> {
        if let Self::Bars(v) = self {
            Ok(v)
        } else {
            Err(invalid("operation requires bars"))
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct Operation {
    /// Canonical ID or alias from native_catalog.
    pub id: String,
    /// Overrides merged with the defaults returned by native_catalog.
    #[serde(default = "empty_params")]
    pub params: Value,
}
fn empty_params() -> Value {
    json!({})
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub schema_version: u32,
    pub identity: SeriesIdentity,
    pub as_of_ms: i64,
    pub data: Input,
    pub operations: Vec<Operation>,
    #[serde(default)]
    pub output: OutputMode,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct Signal {
    pub analog: i8,
    pub ratio: Option<f64>,
}
impl From<Action> for Signal {
    fn from(value: Action) -> Self {
        Self {
            analog: value.analog(),
            ratio: value.ratio(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct CandleValue {
    pub open: Option<f64>,
    pub high: Option<f64>,
    pub low: Option<f64>,
    pub close: Option<f64>,
    pub volume: Option<f64>,
}
fn finite(v: f64) -> Option<f64> {
    v.is_finite().then_some(if v == 0.0 { 0.0 } else { v })
}
impl CandleValue {
    fn from_bar(v: &impl OHLCV) -> Self {
        Self {
            open: finite(v.open()),
            high: finite(v.high()),
            low: finite(v.low()),
            close: finite(v.close()),
            volume: finite(v.volume()),
        }
    }
    fn valid(&self) -> bool {
        self.open.is_some()
            && self.high.is_some()
            && self.low.is_some()
            && self.close.is_some()
            && self.volume.is_some()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum NativeValue {
    Scalar {
        value: Option<f64>,
    },
    Index {
        value: u8,
    },
    Signal {
        value: Signal,
    },
    Indicator {
        values: Vec<Option<f64>>,
        signals: Vec<Signal>,
    },
    Candle {
        value: CandleValue,
    },
    Candles {
        values: Vec<CandleValue>,
    },
    Data {
        value: Value,
    },
    Pending,
}
impl NativeValue {
    fn status(&self) -> &'static str {
        match self {
            Self::Scalar { value: None } => "undefined_result",
            Self::Indicator { values, .. } if values.iter().any(Option::is_none) => {
                "undefined_result"
            }
            Self::Candle { value } if !value.valid() => "undefined_result",
            Self::Candles { values } if values.iter().any(|v| !v.valid()) => "undefined_result",
            Self::Candles { values } if values.is_empty() => "no_output",
            Self::Pending => "no_output",
            _ => "native_output",
        }
    }
    fn cost(&self) -> usize {
        match self {
            Self::Candles { values } => values.len().max(1),
            _ => 1,
        }
    }
}
trait ToNative {
    fn native(self, check: &mut Check<'_>) -> Result<NativeValue, TaError>;
}
impl ToNative for f64 {
    fn native(self, _: &mut Check<'_>) -> Result<NativeValue, TaError> {
        Ok(NativeValue::Scalar {
            value: finite(self),
        })
    }
}
impl ToNative for u8 {
    fn native(self, _: &mut Check<'_>) -> Result<NativeValue, TaError> {
        Ok(NativeValue::Index { value: self })
    }
}
impl ToNative for Action {
    fn native(self, _: &mut Check<'_>) -> Result<NativeValue, TaError> {
        Ok(NativeValue::Signal { value: self.into() })
    }
}
impl ToNative for Candle {
    fn native(self, _: &mut Check<'_>) -> Result<NativeValue, TaError> {
        Ok(NativeValue::Candle {
            value: CandleValue::from_bar(&self),
        })
    }
}
impl ToNative for Option<Candle> {
    fn native(self, check: &mut Check<'_>) -> Result<NativeValue, TaError> {
        match self {
            Some(v) => v.native(check),
            None => Ok(NativeValue::Pending),
        }
    }
}
impl ToNative for Value {
    fn native(self, _: &mut Check<'_>) -> Result<NativeValue, TaError> {
        Ok(NativeValue::Data { value: self })
    }
}
impl ToNative for (f64, f64) {
    fn native(self, _: &mut Check<'_>) -> Result<NativeValue, TaError> {
        Ok(NativeValue::Data {
            value: json!([self.0, self.1]),
        })
    }
}
impl ToNative for crate::methods::renko::RenkoOutput {
    fn native(self, check: &mut Check<'_>) -> Result<NativeValue, TaError> {
        let mut values = Vec::new();
        for brick in self.take(MAX_RENKO_BRICKS + 1) {
            check()?;
            if values.len() == MAX_RENKO_BRICKS {
                return Err(TaError::new(
                    ErrorCode::LimitExceeded,
                    "Renko exceeds 4096 bricks per sample",
                ));
            }
            values.push(CandleValue::from_bar(&brick));
        }
        Ok(NativeValue::Candles { values })
    }
}

#[derive(Debug, Serialize)]
pub struct Row {
    pub at_ms: i64,
    pub available_at_ms: i64,
    pub samples_seen: usize,
    pub status: &'static str,
    pub output: NativeValue,
}
#[derive(Debug, Serialize)]
pub struct Series {
    pub id: String,
    pub params: Value,
    pub parameter_hash: String,
    pub rows: Vec<Row>,
}
#[derive(Debug, Serialize)]
pub struct Response {
    pub schema_version: u32,
    pub implementation_version: &'static str,
    pub hash_protocol: &'static str,
    pub identity: SeriesIdentity,
    pub as_of_ms: i64,
    pub input_hash: String,
    pub output_hash: String,
    pub quality_flags: Vec<&'static str>,
    pub results: Vec<Series>,
}

fn candle(bar: &ClosedBar) -> Candle {
    let c = &bar.candle;
    Candle {
        open: c.open,
        high: c.high,
        low: c.low,
        close: c.close,
        volume: c.volume,
    }
}
fn new_method<M: Method>(params: M::Params, initial: &M::Input) -> Result<M, TaError> {
    M::new(params, initial).map_err(|_| invalid("native method rejected parameters"))
}
fn scalar<'a, M>(params: M::Params, data: &'a Input) -> Result<Runner<'a>, TaError>
where
    M: Method<Input = f64> + 'a,
    M::Output: ToNative,
{
    let Input::Scalars(v) = data else {
        return Err(invalid("operation requires scalars"));
    };
    let mut state = new_method::<M>(
        params,
        &v.first().ok_or_else(|| invalid("empty samples"))?.value,
    )?;
    Ok(Box::new(move |i, check| {
        state.next(&v[i].value).native(check)
    }))
}
fn pair<'a, M>(params: M::Params, data: &'a Input) -> Result<Runner<'a>, TaError>
where
    M: Method<Input = (f64, f64)> + 'a,
    M::Output: ToNative,
{
    let Input::Pairs(v) = data else {
        return Err(invalid("operation requires pairs"));
    };
    let mut state = new_method::<M>(
        params,
        &v.first().ok_or_else(|| invalid("empty samples"))?.value,
    )?;
    Ok(Box::new(move |i, check| {
        state.next(&v[i].value).native(check)
    }))
}
fn bars<'a, M>(params: M::Params, data: &'a Input) -> Result<Runner<'a>, TaError>
where
    M: Method<Input = dyn OHLCV> + 'a,
    M::Output: ToNative,
{
    let v = data.bars()?;
    let mut state = new_method::<M>(
        params,
        &candle(v.first().ok_or_else(|| invalid("empty samples"))?),
    )?;
    Ok(Box::new(move |i, check| {
        state.next(&candle(&v[i])).native(check)
    }))
}
fn owned_bars<'a, M>(params: M::Params, data: &'a Input) -> Result<Runner<'a>, TaError>
where
    M: Method<Input = Candle> + 'a,
    M::Output: ToNative,
{
    let v = data.bars()?;
    let mut state = new_method::<M>(
        params,
        &candle(v.first().ok_or_else(|| invalid("empty samples"))?),
    )?;
    Ok(Box::new(move |i, check| {
        state.next(&candle(&v[i])).native(check)
    }))
}
fn past(params: u8, data: &Input) -> Result<Runner<'_>, TaError> {
    match data {
        Input::Scalars(_) => scalar::<crate::methods::Past<f64>>(params, data),
        Input::Pairs(_) => pair::<crate::methods::Past<(f64, f64)>>(params, data),
        Input::Bars(_) => owned_bars::<crate::methods::Past<Candle>>(params, data),
        Input::Values(v) => {
            let mut state = new_method::<crate::methods::Past<Value>>(
                params,
                &v.first().ok_or_else(|| invalid("empty samples"))?.value,
            )?;
            Ok(Box::new(move |i, check| {
                state.next(&v[i].value).native(check)
            }))
        }
    }
}

fn indicator<'a, C>(params: &Value, data: &'a Input) -> Result<Runner<'a>, TaError>
where
    C: IndicatorConfig + DeserializeOwned + 'a,
    C::Instance: 'a,
{
    let cfg: C = decode(params)?;
    if !cfg.validate() {
        return Err(invalid("native indicator rejected parameters"));
    }
    let v = data.bars()?;
    let mut state = cfg
        .init(&candle(v.first().ok_or_else(|| invalid("empty samples"))?))
        .map_err(|_| invalid("native indicator initialization failed"))?;
    Ok(Box::new(move |i, _| {
        let out = state.next(&candle(&v[i]));
        Ok(NativeValue::Indicator {
            values: out.values().iter().copied().map(finite).collect(),
            signals: out.signals().iter().copied().map(Into::into).collect(),
        })
    }))
}
macro_rules! indicator_registry {($(($id:literal,$ty:ident)),* $(,)?)=>{
    pub(super) fn defaults(id:&str)->Result<Value,TaError>{match id { $($id=>encode(crate::indicators::$ty::default()),)* _=>Err(invalid("unknown native indicator"))}}
    pub(super) fn runner<'a>(id:&str,params:&Value,data:&'a Input)->Result<Runner<'a>,TaError>{match id { $($id=>indicator::<crate::indicators::$ty>(params,data),)* _=>Err(invalid("unknown native indicator"))}}
};}
mod catalog;
mod indicators;
mod methods;
pub use catalog::catalog;

fn validate_value(v: &Value) -> Result<(), TaError> {
    match v {
        Value::Number(n) => {
            if n.as_f64().is_none_or(|f| !f.is_finite() || f.abs() > 1e100) {
                return Err(invalid("numbers must be finite and within 1e100"));
            }
        }
        Value::Array(v) => {
            for x in v {
                validate_value(x)?;
            }
        }
        Value::Object(v) => {
            for x in v.values() {
                validate_value(x)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn validate(request: &Request, check: &mut Check<'_>) -> Result<(), TaError> {
    if request.schema_version != 1 {
        return Err(invalid("unsupported native schema_version"));
    }
    request.identity.validate()?;
    if request.as_of_ms <= 0 {
        return Err(TaError::new(
            ErrorCode::InvalidTime,
            "as_of_ms must be positive",
        ));
    }
    if request.data.len() == 0
        || request.data.len() > MAX_SAMPLES
        || request.operations.is_empty()
        || request.operations.len() > MAX_OPERATIONS
    {
        return Err(TaError::new(
            ErrorCode::LimitExceeded,
            "require 1..4096 samples and 1..16 operations",
        ));
    }
    let mut previous = None;
    for i in 0..request.data.len() {
        check()?;
        let (t, a) = request.data.time(i);
        if t <= 0 || a < t || a > request.as_of_ms {
            return Err(TaError::new(
                ErrorCode::InvalidTime,
                "sample must be available at as_of_ms",
            ));
        }
        if previous.is_some_and(|(pt, pa)| t <= pt || a < pa) {
            return Err(TaError::new(
                ErrorCode::DuplicateOrUnorderedBar,
                "duplicate, unordered or retroactive samples",
            ));
        }
        match &request.data {
            Input::Bars(v) => engine::validate_bar(&v[i], request.as_of_ms, None, None)?,
            Input::Scalars(v) => {
                if !v[i].value.is_finite() || v[i].value.abs() > 1e100 {
                    return Err(invalid("invalid scalar"));
                }
            }
            Input::Pairs(v) => {
                for x in [v[i].value.0, v[i].value.1] {
                    if !x.is_finite() || x.abs() > 1e100 {
                        return Err(invalid("invalid pair"));
                    }
                }
            }
            Input::Values(v) => {
                // Past seeds a window by cloning the first value; bound that amplification.
                if serde_json::to_vec(&v[i].value)
                    .map_err(|_| invalid("invalid JSON value"))?
                    .len()
                    > MAX_JSON_VALUE_BYTES
                {
                    return Err(TaError::new(
                        ErrorCode::LimitExceeded,
                        "Past JSON values are limited to 1024 encoded bytes each",
                    ));
                }
                validate_value(&v[i].value)?;
            }
        }
        previous = Some((t, a));
    }
    Ok(())
}

pub fn calculate(request: &Request) -> Result<Response, TaError> {
    calculate_controlled(request, || Ok(()))
}
pub fn calculate_controlled(
    request: &Request,
    mut checkpoint: impl FnMut() -> Result<(), TaError>,
) -> Result<Response, TaError> {
    // A native implementation panic is isolated to this pure request. Never return partial results.
    catch_unwind(AssertUnwindSafe(|| {
        calculate_inner(request, &mut checkpoint)
    }))
    .unwrap_or_else(|_| {
        Err(TaError::new(
            ErrorCode::UpstreamFailure,
            "native algorithm failed; no partial result returned",
        ))
    })
}
fn calculate_inner(request: &Request, check: &mut Check<'_>) -> Result<Response, TaError> {
    validate(request, check)?;
    let entries = catalog::entries()?;
    let mut jobs = Vec::new();
    // Resolve and initialize every operation before consuming the first sample.
    for operation in &request.operations {
        check()?;
        let entry = entries
            .iter()
            .find(|e| {
                e["id"] == operation.id
                    || e["aliases"]
                        .as_array()
                        .is_some_and(|a| a.iter().any(|v| v == &operation.id))
            })
            .ok_or_else(|| {
                TaError::new(
                    ErrorCode::UnsupportedProfile,
                    "unknown native operation; use native_catalog",
                )
            })?;
        if !entry["inputs"]
            .as_array()
            .is_some_and(|v| v.iter().any(|k| k == request.data.kind()))
        {
            return Err(invalid("input kind does not match operation"));
        }
        let params = catalog::resolve(entry, &operation.params)?;
        let id = entry["id"]
            .as_str()
            .ok_or_else(|| invalid("invalid registry ID"))?
            .to_owned();
        let runner = if entry["kind"] == "indicator" {
            indicators::runner(
                entry["module"]
                    .as_str()
                    .ok_or_else(|| invalid("invalid registry module"))?,
                &params,
                &request.data,
            )?
        } else {
            methods::runner(
                entry["symbol"]
                    .as_str()
                    .ok_or_else(|| invalid("invalid registry symbol"))?,
                &params,
                &request.data,
            )?
        };
        let hash = fingerprint::digest(&(VERSION, &id, &params))?;
        jobs.push((
            Series {
                id,
                params,
                parameter_hash: hash,
                rows: Vec::new(),
            },
            runner,
        ));
    }
    let mut cost = 0;
    let mut retained_bytes = 0;
    for i in 0..request.data.len() {
        for (series, runner) in &mut jobs {
            check()?;
            let output = runner(i, check)?;
            cost += output.cost();
            if cost > MAX_OUTPUT_ITEMS {
                return Err(TaError::new(
                    ErrorCode::LimitExceeded,
                    "native generated output exceeds 65536 items",
                ));
            }
            if matches!(request.output, OutputMode::Series) || i + 1 == request.data.len() {
                let (at_ms, available_at_ms) = request.data.time(i);
                let row = Row {
                    at_ms,
                    available_at_ms,
                    samples_seen: i + 1,
                    status: output.status(),
                    output,
                };
                retained_bytes += serde_json::to_vec(&row)
                    .map_err(|_| {
                        TaError::new(ErrorCode::EncodingFailed, "native row encoding failed")
                    })?
                    .len();
                if retained_bytes > MAX_RETAINED_ROW_BYTES {
                    return Err(TaError::new(
                        ErrorCode::LimitExceeded,
                        "native retained rows exceed 3 MiB; use latest or a smaller batch",
                    ));
                }
                series.rows.push(row);
            }
        }
    }
    check()?;
    let results: Vec<_> = jobs.into_iter().map(|(series, _)| series).collect();
    let input_hash = fingerprint::digest(&(VERSION, request))?;
    let output_hash = fingerprint::digest(&(VERSION, &input_hash, &results))?;
    Ok(Response {
        schema_version: 1,
        implementation_version: VERSION,
        hash_protocol: fingerprint::HASH_PROTOCOL,
        identity: request.identity.clone(),
        as_of_ms: request.as_of_ms,
        input_hash,
        output_hash,
        quality_flags: vec![
            "native_seeded_output",
            "warmup_not_certified",
            "signals_are_not_orders",
            "timestamps_are_emission_times",
        ],
        results,
    })
}
