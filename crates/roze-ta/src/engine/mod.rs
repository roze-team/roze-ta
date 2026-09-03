//! Versioned deterministic streaming/batch engine; no I/O or trading state.
//! See docs/contracts/engine-v2.md for compatibility and hash contracts.
mod extensions;
mod kernel;
use crate::{
    catalog::{self, Candle, Profile},
    error::{ErrorCode, TaError},
    fingerprint,
};
use kernel::Kernel;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub const SCHEMA_VERSION: u32 = 2;
pub const SNAPSHOT_VERSION: u32 = 1;
pub const IMPLEMENTATION: &str =
    "roze-ta-engine-v2.0/yata-0.7.0@5030e2349cedde60b0e367a9de9400d466ff644f";
pub const MAX_RESULT_ROWS: usize = 65_536;
pub const MAX_SNAPSHOT_BYTES: usize = 1_048_576;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct SeriesIdentity {
    pub series_id: String,
    pub instrument: String,
    pub timeframe: String,
    pub source: String,
    pub data_version: String,
}
impl SeriesIdentity {
    pub fn validate(&self) -> Result<(), TaError> {
        if [
            &self.series_id,
            &self.instrument,
            &self.timeframe,
            &self.source,
            &self.data_version,
        ]
        .iter()
        .any(|s| s.trim().is_empty() || s.len() > 128)
        {
            return Err(TaError::new(
                ErrorCode::InvalidIdentity,
                "identity fields must contain 1..128 UTF-8 bytes",
            ));
        }
        Ok(())
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ClosedBar {
    pub candle: Candle,
    /// Earliest instant the caller says this completed candle was knowable.
    pub available_at_ms: i64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Ready,
    WarmingUp,
    UndefinedResult,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct Measurement {
    pub schema_version: u32,
    pub profile_version: u32,
    pub profile_id: String,
    pub parameter_hash: String,
    pub status: Status,
    pub error_code: Option<ErrorCode>,
    pub values: BTreeMap<String, f64>,
    pub units: String,
    pub samples_seen: u64,
    pub minimum_samples: usize,
    pub last_bar_closed_at_ms: i64,
    pub available_at_ms: i64,
    pub quality_flags: Vec<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
struct State {
    identity: SeriesIdentity,
    profile_id: String,
    samples: u64,
    last_closed: Option<i64>,
    last_available: Option<i64>,
    latest: Option<Measurement>,
    kernel: Kernel,
}
#[derive(Clone, Debug)]
pub struct Stream {
    profile: Profile,
    parameter_hash: String,
    state: State,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct Snapshot {
    pub schema_version: u32,
    pub implementation_version: String,
    pub parameter_hash: String,
    /// bincode 2.0.1 serde, standard little-endian/variable integers, lowercase hex.
    pub payload_hex: String,
    pub checksum: String,
}
impl Stream {
    pub fn new(identity: SeriesIdentity, profile_id: &str) -> Result<Self, TaError> {
        identity.validate()?;
        let profile = catalog::catalog()
            .into_iter()
            .find(|p| p.id == profile_id)
            .ok_or_else(|| TaError::new(ErrorCode::UnsupportedProfile, "unregistered profile"))?;
        let parameter_hash =
            fingerprint::digest(&(IMPLEMENTATION, &profile.id, &profile.parameters))?;
        let kernel = Kernel::new(profile_id)?;
        Ok(Self {
            profile,
            parameter_hash,
            state: State {
                identity,
                profile_id: profile_id.into(),
                samples: 0,
                last_closed: None,
                last_available: None,
                latest: None,
                kernel,
            },
        })
    }
    pub fn latest(&self) -> Option<&Measurement> {
        self.state.latest.as_ref()
    }
    pub fn samples_seen(&self) -> u64 {
        self.state.samples
    }
    pub fn identity(&self) -> &SeriesIdentity {
        &self.state.identity
    }
    pub fn profile(&self) -> &Profile {
        &self.profile
    }

    /// Invalid input leaves the state unchanged. Undefined arithmetic consumes the bar.
    /// Snapshot/rollback of kernel state is O(window size), bounded by the fixed profile.
    pub fn update(&mut self, bar: &ClosedBar, as_of_ms: i64) -> Result<Measurement, TaError> {
        validate_bar(
            bar,
            as_of_ms,
            self.state.last_closed,
            self.state.last_available,
        )?;
        let samples = self
            .state
            .samples
            .checked_add(1)
            .ok_or_else(|| TaError::new(ErrorCode::LimitExceeded, "sample count exhausted"))?;
        let mut candidate = self.state.kernel.clone();
        let values =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| candidate.step(&bar.candle)))
                .map_err(|_| {
                    TaError::new(ErrorCode::UpstreamFailure, "upstream calculation panicked")
                })??;
        let status = if samples < self.profile.minimum_bars as u64 {
            Status::WarmingUp
        } else if values.len() != self.profile.outputs.len()
            || values.iter().any(|v| !v.is_finite())
        {
            Status::UndefinedResult
        } else {
            Status::Ready
        };
        let values = if status == Status::Ready {
            self.profile
                .outputs
                .iter()
                .cloned()
                .zip(values.into_iter().map(|v| if v == 0.0 { 0.0 } else { v }))
                .collect()
        } else {
            BTreeMap::new()
        };
        let mut quality_flags = Vec::new();
        if self.profile.minimum_bars == 256 {
            quality_flags.push("conservative_warmup_not_audited".into());
        }
        if bar.candle.volume == 0.0 {
            quality_flags.push("zero_volume".into());
        }
        let row = Measurement {
            schema_version: SCHEMA_VERSION,
            profile_version: 1,
            profile_id: self.profile.id.clone(),
            parameter_hash: self.parameter_hash.clone(),
            status,
            error_code: match status {
                Status::Ready => None,
                Status::WarmingUp => Some(ErrorCode::NotReady),
                Status::UndefinedResult => Some(ErrorCode::UndefinedResult),
            },
            values,
            units: self.profile.units.clone(),
            samples_seen: samples,
            minimum_samples: self.profile.minimum_bars,
            last_bar_closed_at_ms: bar.candle.closed_at_ms,
            available_at_ms: bar.available_at_ms,
            quality_flags,
        };
        self.state.kernel = candidate;
        self.state.samples = samples;
        self.state.last_closed = Some(bar.candle.closed_at_ms);
        self.state.last_available = Some(bar.available_at_ms);
        self.state.latest = Some(row.clone());
        Ok(row)
    }
    pub fn reset(&mut self) -> Result<(), TaError> {
        *self = Self::new(self.state.identity.clone(), &self.profile.id)?;
        Ok(())
    }
    pub fn snapshot(&self) -> Result<Snapshot, TaError> {
        let bytes = bincode::serde::encode_to_vec(&self.state, bincode::config::standard())
            .map_err(|_| TaError::new(ErrorCode::EncodingFailed, "state encoding failed"))?;
        if bytes.len() > MAX_SNAPSHOT_BYTES {
            return Err(TaError::new(ErrorCode::LimitExceeded, "snapshot too large"));
        }
        let payload_hex = fingerprint::hex(&bytes);
        let checksum = fingerprint::digest(&(
            SNAPSHOT_VERSION,
            IMPLEMENTATION,
            &self.parameter_hash,
            &payload_hex,
        ))?;
        Ok(Snapshot {
            schema_version: SNAPSHOT_VERSION,
            implementation_version: IMPLEMENTATION.into(),
            parameter_hash: self.parameter_hash.clone(),
            payload_hex,
            checksum,
        })
    }
    /// Restore only into the expected profile and exact sequence identity.
    /// The checksum detects corruption; it is not authentication.
    pub fn restore(
        snapshot: &Snapshot,
        identity: &SeriesIdentity,
        profile_id: &str,
    ) -> Result<Self, TaError> {
        let mut stream = Self::new(identity.clone(), profile_id)?;
        if snapshot.schema_version != SNAPSHOT_VERSION
            || snapshot.implementation_version != IMPLEMENTATION
            || snapshot.parameter_hash != stream.parameter_hash
        {
            return Err(TaError::new(
                ErrorCode::IncompatibleSnapshot,
                "snapshot version/profile mismatch; replay history",
            ));
        }
        if snapshot.payload_hex.len() > MAX_SNAPSHOT_BYTES * 2 {
            return Err(TaError::new(ErrorCode::LimitExceeded, "snapshot too large"));
        }
        let checksum = fingerprint::digest(&(
            snapshot.schema_version,
            &snapshot.implementation_version,
            &snapshot.parameter_hash,
            &snapshot.payload_hex,
        ))?;
        if checksum != snapshot.checksum {
            return Err(corrupt());
        }
        let bytes = fingerprint::unhex(&snapshot.payload_hex).ok_or_else(corrupt)?;
        let decoded = std::panic::catch_unwind(|| {
            bincode::serde::decode_from_slice::<State, _>(
                &bytes,
                bincode::config::standard().with_limit::<MAX_SNAPSHOT_BYTES>(),
            )
        });
        let (state, used) = decoded.map_err(|_| corrupt())?.map_err(|_| corrupt())?;
        if used != bytes.len()
            || &state.identity != identity
            || state.profile_id != profile_id
            || state.kernel.kind() != stream.state.kernel.kind()
            || !state.kernel.valid_for(profile_id, state.samples)
        {
            return Err(corrupt());
        }
        match (
            &state.latest,
            state.last_closed,
            state.last_available,
            state.samples,
        ) {
            (None, None, None, 0) => {}
            (Some(row), Some(closed), Some(available), samples)
                if samples > 0
                    && closed > 0
                    && available >= closed
                    && row.samples_seen == samples
                    && row.last_bar_closed_at_ms == closed
                    && row.available_at_ms == available
                    && row.schema_version == SCHEMA_VERSION
                    && row.profile_version == 1
                    && row.profile_id == profile_id
                    && row.parameter_hash == stream.parameter_hash
                    && row.minimum_samples == stream.profile.minimum_bars
                    && row.units == stream.profile.units
                    && row.values.values().all(|v| v.is_finite()) => {}
            _ => return Err(corrupt()),
        }
        if let Some(row) = &state.latest {
            let warming = state.samples < stream.profile.minimum_bars as u64;
            let valid = match row.status {
                Status::WarmingUp => {
                    warming && row.values.is_empty() && row.error_code == Some(ErrorCode::NotReady)
                }
                Status::UndefinedResult => {
                    !warming
                        && row.values.is_empty()
                        && row.error_code == Some(ErrorCode::UndefinedResult)
                }
                Status::Ready => {
                    !warming
                        && row.error_code.is_none()
                        && row.values.len() == stream.profile.outputs.len()
                        && stream
                            .profile
                            .outputs
                            .iter()
                            .all(|name| row.values.contains_key(name))
                }
            };
            if !valid {
                return Err(corrupt());
            }
        }
        stream.state = state;
        Ok(stream)
    }
}
fn corrupt() -> TaError {
    TaError::new(
        ErrorCode::CorruptSnapshot,
        "snapshot content is corrupt or belongs to another sequence; replay history",
    )
}

pub fn validate_bar(
    bar: &ClosedBar,
    as_of: i64,
    last_closed: Option<i64>,
    last_available: Option<i64>,
) -> Result<(), TaError> {
    let c = &bar.candle;
    if as_of <= 0
        || c.closed_at_ms <= 0
        || c.closed_at_ms > as_of
        || bar.available_at_ms < c.closed_at_ms
        || bar.available_at_ms > as_of
    {
        return Err(TaError::new(
            ErrorCode::InvalidTime,
            "bar must be closed and available at as_of_ms",
        ));
    }
    if last_closed.is_some_and(|last| c.closed_at_ms <= last)
        || last_available.is_some_and(|last| bar.available_at_ms < last)
    {
        return Err(TaError::new(
            ErrorCode::DuplicateOrUnorderedBar,
            "duplicate, reordered or retroactively available bar",
        ));
    }
    if [c.open, c.high, c.low, c.close, c.volume]
        .iter()
        .any(|v| !v.is_finite() || v.abs() > 1e100)
        || c.low <= 0.0
        || c.high < c.low
        || c.open < c.low
        || c.open > c.high
        || c.close < c.low
        || c.close > c.high
        || c.volume < 0.0
    {
        return Err(TaError::new(
            ErrorCode::InvalidBar,
            "invalid positive-price OHLCV; finite values and nonnegative volume required",
        ));
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum OutputMode {
    #[default]
    Latest,
    Series,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct BatchRequest {
    pub schema_version: u32,
    pub snapshot_id: String,
    pub identity: SeriesIdentity,
    pub as_of_ms: i64,
    pub bars: Vec<ClosedBar>,
    pub profiles: Vec<String>,
    #[serde(default)]
    pub output: OutputMode,
    #[serde(default)]
    pub require_all_ready: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct ProfileSeries {
    pub profile_id: String,
    pub rows: Vec<Measurement>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct BatchResult {
    pub schema_version: u32,
    pub implementation_version: String,
    pub hash_protocol: String,
    pub snapshot_id: String,
    pub identity: SeriesIdentity,
    pub as_of_ms: i64,
    pub input_hash: String,
    pub output_hash: String,
    pub series: Vec<ProfileSeries>,
}
/// All shared input is validated before calculation; output order follows request order.
pub fn calculate(request: &BatchRequest) -> Result<BatchResult, TaError> {
    calculate_controlled(request, || Ok(()))
}
/// Caller-owned cooperative cancellation/deadline hook, checked before each bar.
/// No clock or runtime dependency is introduced into the deterministic engine.
pub fn calculate_controlled(
    request: &BatchRequest,
    mut checkpoint: impl FnMut() -> Result<(), TaError>,
) -> Result<BatchResult, TaError> {
    checkpoint()?;
    request.identity.validate()?;
    if request.schema_version != SCHEMA_VERSION
        || request.snapshot_id.trim().is_empty()
        || request.snapshot_id.len() > 128
    {
        return Err(TaError::new(
            ErrorCode::InvalidIdentity,
            "unsupported schema or invalid snapshot identity",
        ));
    }
    let rows = match request.output {
        OutputMode::Latest => 1,
        OutputMode::Series => request.bars.len(),
    };
    if request.bars.is_empty()
        || request.bars.len() > catalog::MAX_BARS
        || request.profiles.is_empty()
        || request.profiles.len() > catalog::MAX_PROFILES
        || rows.saturating_mul(request.profiles.len()) > MAX_RESULT_ROWS
    {
        return Err(TaError::new(
            ErrorCode::LimitExceeded,
            "empty input or batch/output limit exceeded",
        ));
    }
    let (mut last_closed, mut last_available) = (None, None);
    for bar in &request.bars {
        checkpoint()?;
        validate_bar(bar, request.as_of_ms, last_closed, last_available)?;
        last_closed = Some(bar.candle.closed_at_ms);
        last_available = Some(bar.available_at_ms);
    }
    let mut seen = BTreeSet::new();
    let mut engines = Vec::new();
    for id in &request.profiles {
        if !seen.insert(id) {
            return Err(TaError::new(
                ErrorCode::DuplicateProfile,
                "duplicate profile",
            ));
        }
        engines.push(Stream::new(request.identity.clone(), id)?);
    }
    let mut series = Vec::new();
    for mut engine in engines {
        let mut rows = Vec::new();
        for bar in &request.bars {
            checkpoint()?;
            let row = engine.update(bar, request.as_of_ms)?;
            if matches!(request.output, OutputMode::Series) {
                rows.push(row);
            }
        }
        if matches!(request.output, OutputMode::Latest) {
            rows.push(
                engine
                    .latest()
                    .ok_or_else(|| TaError::new(ErrorCode::UpstreamFailure, "missing result"))?
                    .clone(),
            );
        }
        if request.require_all_ready && rows.iter().any(|r| r.status != Status::Ready) {
            return Err(TaError::new(
                ErrorCode::NotReady,
                "strict output contains warming-up or undefined results",
            ));
        }
        series.push(ProfileSeries {
            profile_id: engine.profile.id,
            rows,
        });
    }
    let input_hash = fingerprint::digest(request)?;
    let output_hash = fingerprint::digest(&(SCHEMA_VERSION, IMPLEMENTATION, &input_hash, &series))?;
    Ok(BatchResult {
        schema_version: SCHEMA_VERSION,
        implementation_version: IMPLEMENTATION.into(),
        hash_protocol: fingerprint::HASH_PROTOCOL.into(),
        snapshot_id: request.snapshot_id.clone(),
        identity: request.identity.clone(),
        as_of_ms: request.as_of_ms,
        input_hash,
        output_hash,
        series,
    })
}
