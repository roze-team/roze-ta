//! Shared, read-only indicator profiles for Native Tools and MCP. No IO or orders.
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
#[cfg(test)]
use yata::prelude::{IndicatorInstance, Method};
use yata::{indicators::*, prelude::IndicatorConfig};

pub const VERSION: &str = "roze-ta-catalog-v1/yata-0.7.0@5030e2349cedde60b0e367a9de9400d466ff644f";
pub const MAX_BARS: usize = 4096;
pub const MAX_PROFILES: usize = 64;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct Candle {
    pub closed_at_ms: i64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct BatchRequest {
    /// Caller-provided snapshot reference. The gateway does not certify its origin.
    pub snapshot_id: String,
    pub symbol: String,
    pub timeframe: String,
    pub as_of_ms: i64,
    pub bars: Vec<Candle>,
    /// Select IDs from indicator_catalog. No arbitrary code or parameters accepted.
    pub profiles: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct Profile {
    pub id: String,
    pub family_id: String,
    pub capability_kind: String,
    pub formula_variant: String,
    pub warmup_rule: String,
    pub verification_status: String,
    pub name: String,
    pub implementation_version: String,
    pub parameters: serde_json::Value,
    pub minimum_bars: usize,
    pub outputs: Vec<String>,
    pub units: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Calculation {
    pub profile_id: String,
    pub parameters: serde_json::Value,
    pub status: String,
    pub values: BTreeMap<String, f64>,
    pub error: Option<String>,
    pub minimum_bars: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BatchResult {
    pub snapshot_id: String,
    pub implementation_version: String,
    pub input_origin: String,
    pub input_hash: String,
    pub output_hash: String,
    pub results: Vec<Calculation>,
}

fn hash(value: &impl Serialize) -> Result<String, String> {
    Ok(format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(value).map_err(|_| "invalid indicator data")?)
    ))
}

fn profile(
    id: &str,
    name: &str,
    params: serde_json::Value,
    minimum: usize,
    outputs: &[&str],
    units: &str,
) -> Profile {
    Profile {
        id: id.into(),
        family_id: id.split('.').next().unwrap_or(id).into(),
        capability_kind: "indicator".into(),
        formula_variant: if id == "trix.default" {
            "yata_absolute_smoothed_change"
        } else {
            "existing_profile_v1"
        }
        .into(),
        warmup_rule: if minimum == 256 {
            "legacy_256_pending_individual_audit"
        } else {
            "documented_base_profile_minimum"
        }
        .into(),
        verification_status: "existing_profile_partial_audit".into(),
        name: name.into(),
        implementation_version: VERSION.into(),
        parameters: params,
        minimum_bars: minimum,
        outputs: outputs.iter().map(|s| (*s).into()).collect(),
        units: units.into(),
    }
}

pub fn catalog() -> Vec<Profile> {
    let mut result = Vec::new();
    for period in [5, 8, 13, 21, 34, 55, 89, 144] {
        result.push(profile(
            &format!("ema.{period}"),
            "EMA",
            serde_json::json!({"period":period,"source":"close"}),
            period,
            &["ema"],
            "price",
        ));
    }
    for period in [5, 10, 20, 50, 100, 200] {
        result.push(profile(
            &format!("sma.{period}"),
            "SMA",
            serde_json::json!({"period":period,"source":"close"}),
            period,
            &["sma"],
            "price",
        ));
    }
    result.push(profile(
        "rsi.14",
        "RSI",
        serde_json::json!({"period":14,"source":"close"}),
        15,
        &["rsi"],
        "0..100",
    ));
    result.push(profile(
        "atr.14",
        "ATR",
        serde_json::json!({"period":14,"smoothing":"rma"}),
        15,
        &["atr"],
        "price",
    ));
    macro_rules! entry {
        ($id:expr,$type:ty,$names:expr,$units:expr) => {
            result.push(profile(
                $id,
                <$type>::NAME,
                serde_json::to_value(<$type>::default()).expect("static profile"),
                256,
                $names,
                $units,
            ));
        };
    }
    entry!("macd.default", MACD, &["macd", "signal"], "price");
    entry!(
        "bollinger.default",
        BollingerBands,
        &["upper", "middle", "lower"],
        "price"
    );
    entry!(
        "adx.default",
        AverageDirectionalIndex,
        &["adx", "plus_di", "minus_di"],
        "0..1 (yata convention)"
    );
    entry!(
        "cci.default",
        CommodityChannelIndex,
        &["cci"],
        "yata normalized CCI"
    );
    entry!(
        "stochastic.default",
        StochasticOscillator,
        &["k", "d"],
        "0..1 (yata convention)"
    );
    entry!(
        "donchian.default",
        DonchianChannel,
        &["lower", "middle", "upper"],
        "price"
    );
    entry!(
        "aroon.default",
        Aroon,
        &["up", "down"],
        "0..1 (yata convention)"
    );
    entry!("cmf.default", ChaikinMoneyFlow, &["cmf"], "ratio");
    entry!(
        "sar.default",
        ParabolicSAR,
        &["sar", "trend"],
        "price / direction"
    );
    entry!("hma.default", HullMovingAverage, &["hma"], "price");
    entry!("kama.default", Kaufman, &["kama"], "price");
    entry!(
        "ichimoku.default",
        IchimokuCloud,
        &["tenkan_sen", "kijun_sen", "senkou_span_a", "senkou_span_b"],
        "price; Senkou outputs use upstream delayed windows"
    );
    // The implementation returns source first, despite the upstream doc listing upper first.
    entry!(
        "keltner.default",
        KeltnerChannel,
        &["source", "upper", "lower"],
        "price"
    );
    entry!(
        "mfi.default",
        MoneyFlowIndex,
        &["upper_threshold", "mfi", "lower_threshold"],
        "0..1 (yata convention)"
    );
    entry!(
        "trix.default",
        Trix,
        &["trix", "signal"],
        "absolute smoothed price change (Yata extended TRIX, not percentage)"
    );
    entry!(
        "cmo.default",
        ChandeMomentumOscillator,
        &["cmo"],
        "-1..1 (yata convention)"
    );
    entry!(
        "chaikin.default",
        ChaikinOscillator,
        &["chaikin"],
        "volume-weighted accumulation/distribution difference"
    );
    entry!("efi.default", EldersForceIndex, &["efi"], "price * volume");
    entry!("ao.default", AwesomeOscillator, &["ao"], "price difference");
    for (id, name, variant, minimum, output, units, warmup) in [
        (
            "wma.14",
            "WMA",
            "linear_weights_newest_14",
            14,
            "wma",
            "price",
            "14 observed closes replace padded history",
        ),
        (
            "rma.14",
            "RMA",
            "alpha_1_over_n_first_close_seed",
            14,
            "rma",
            "price",
            "14 observations; first-close seed influence persists geometrically",
        ),
        (
            "dema.14",
            "DEMA",
            "two_ema_first_close_seed",
            27,
            "dema",
            "price",
            "2*(14-1)+1 observations; both EMA stages seeded by first close",
        ),
        (
            "tema.14",
            "TEMA",
            "three_ema_first_close_seed",
            40,
            "tema",
            "price",
            "3*(14-1)+1 observations; all EMA stages seeded by first close",
        ),
        (
            "vwma.14",
            "VWMA",
            "rolling_close_bar_volume_recomputed",
            14,
            "vwma",
            "price",
            "14 observed close/volume pairs; no synthetic volume padding",
        ),
        (
            "roc.14",
            "ROC",
            "relative_change_fraction",
            15,
            "roc",
            "ratio",
            "current close and actual close 14 observations earlier",
        ),
        (
            "momentum.14",
            "Momentum",
            "absolute_close_difference",
            15,
            "momentum",
            "price",
            "current close and actual close 14 observations earlier",
        ),
        (
            "ad.cumulative",
            "Accumulation/Distribution",
            "clv_volume_zero_seed_flat_clv_zero",
            1,
            "ad",
            "caller_volume",
            "one bar; cumulative from explicit stream start",
        ),
        (
            "obv.zero_seed",
            "OBV",
            "zero_seed_close_direction_signed_volume",
            2,
            "obv",
            "caller_volume",
            "first bar establishes baseline; one actual close change required",
        ),
        (
            "williams_r.14",
            "Williams %R",
            "negative_percent_range",
            14,
            "williams_r",
            "-100..0",
            "14 observed high/low pairs including current bar",
        ),
    ] {
        let mut parameters = serde_json::json!({"formula_variant":variant,"algorithm_version":"a1-v1","source":"close","period":14});
        if id == "ad.cumulative" || id == "obv.zero_seed" {
            parameters = serde_json::json!({"formula_variant":variant,"algorithm_version":"a1-v1","initial_value":0.0});
        }
        if matches!(id, "vwma.14" | "ad.cumulative" | "obv.zero_seed") {
            parameters["volume_semantics"] =
                serde_json::json!("caller_supplied_consistent_units; no_trade_classification");
        }
        let mut item = profile(id, name, parameters, minimum, &[output], units);
        item.formula_variant = variant.into();
        item.warmup_rule = warmup.into();
        item.verification_status =
            "a1_windows_reference_stream_restore_verified; cross_platform_pending".into();
        item.implementation_version = format!("roze-ta-a1-v1/{VERSION}");
        result.push(item);
    }
    result
}

#[cfg(test)]
fn run_config<C: IndicatorConfig + Default>(bars: &[[f64; 5]]) -> Result<Vec<f64>, String> {
    let mut instance = C::default()
        .init(&bars[0])
        .map_err(|_| "indicator initialization failed")?;
    let mut values = Vec::new();
    for bar in bars {
        values = instance.next(bar).values().to_vec();
    }
    Ok(values)
}

#[cfg(test)]
fn compute_legacy(id: &str, bars: &[Candle]) -> Result<Vec<f64>, String> {
    let closes = bars.iter().map(|b| b.close).collect::<Vec<_>>();
    if let Some(period) = id
        .strip_prefix("ema.")
        .and_then(|s| s.parse::<usize>().ok())
    {
        return crate::ema(&closes, period)
            .map(|v| vec![v])
            .ok_or_else(|| "EMA failed".into());
    }
    if let Some(period) = id.strip_prefix("sma.").and_then(|s| s.parse::<u8>().ok()) {
        let mut state = yata::methods::SMA::new(period, &closes[0]).map_err(|_| "SMA failed")?;
        return Ok(vec![closes.iter().fold(closes[0], |_, v| state.next(v))]);
    }
    if id == "rsi.14" {
        return crate::rsi(&closes, 14)
            .map(|v| vec![v])
            .ok_or_else(|| "RSI failed".into());
    }
    if id == "atr.14" {
        return crate::atr(
            &bars
                .iter()
                .map(|b| crate::Bar {
                    open: b.open,
                    high: b.high,
                    low: b.low,
                    close: b.close,
                    volume: b.volume,
                })
                .collect::<Vec<_>>(),
            14,
        )
        .map(|v| vec![v])
        .ok_or_else(|| "ATR failed".into());
    }
    let candles = bars
        .iter()
        .map(|b| [b.open, b.high, b.low, b.close, b.volume])
        .collect::<Vec<_>>();
    match id {
        "macd.default" => run_config::<MACD>(&candles),
        "bollinger.default" => run_config::<BollingerBands>(&candles),
        "adx.default" => run_config::<AverageDirectionalIndex>(&candles),
        "cci.default" => run_config::<CommodityChannelIndex>(&candles),
        "stochastic.default" => run_config::<StochasticOscillator>(&candles),
        "donchian.default" => run_config::<DonchianChannel>(&candles),
        "aroon.default" => run_config::<Aroon>(&candles),
        "cmf.default" => run_config::<ChaikinMoneyFlow>(&candles),
        "sar.default" => run_config::<ParabolicSAR>(&candles),
        "hma.default" => run_config::<HullMovingAverage>(&candles),
        "kama.default" => run_config::<Kaufman>(&candles),
        "ichimoku.default" => run_config::<IchimokuCloud>(&candles),
        "keltner.default" => run_config::<KeltnerChannel>(&candles),
        "mfi.default" => run_config::<MoneyFlowIndex>(&candles),
        "trix.default" => run_config::<Trix>(&candles),
        "cmo.default" => run_config::<ChandeMomentumOscillator>(&candles),
        "chaikin.default" => run_config::<ChaikinOscillator>(&candles),
        "efi.default" => run_config::<EldersForceIndex>(&candles),
        "ao.default" => run_config::<AwesomeOscillator>(&candles),
        _ => Err("unregistered indicator profile".into()),
    }
}

#[cfg(test)]
fn compute(id: &str, bars: &[Candle]) -> Result<Vec<f64>, String> {
    compute_controlled(id, bars, &mut || Ok(()))
}

fn compute_controlled(
    id: &str,
    bars: &[Candle],
    checkpoint: &mut impl FnMut() -> Result<(), String>,
) -> Result<Vec<f64>, String> {
    use crate::engine::{ClosedBar, SeriesIdentity, Stream};
    let identity = SeriesIdentity {
        series_id: "legacy-replay".into(),
        instrument: "legacy".into(),
        timeframe: "caller-provided".into(),
        source: "caller-provided".into(),
        data_version: "v1".into(),
    };
    let mut stream = Stream::new(identity, id).map_err(|e| e.to_string())?;
    for bar in bars {
        checkpoint()?;
        stream
            .update(
                &ClosedBar {
                    candle: bar.clone(),
                    available_at_ms: bar.closed_at_ms,
                },
                bar.closed_at_ms,
            )
            .map_err(|e| e.to_string())?;
    }
    let row = stream.latest().ok_or("empty series")?;
    stream
        .profile()
        .outputs
        .iter()
        .map(|key| {
            row.values
                .get(key)
                .copied()
                .ok_or_else(|| "undefined indicator result".into())
        })
        .collect()
}

pub fn calculate(request: &BatchRequest) -> Result<BatchResult, String> {
    calculate_controlled(request, || Ok(()))
}

pub fn calculate_controlled(
    request: &BatchRequest,
    mut checkpoint: impl FnMut() -> Result<(), String>,
) -> Result<BatchResult, String> {
    checkpoint()?;
    if [&request.snapshot_id, &request.symbol, &request.timeframe]
        .iter()
        .any(|s| s.trim().is_empty() || s.len() > 128)
        || request.as_of_ms <= 0
    {
        return Err("invalid snapshot identity".into());
    }
    if request.bars.is_empty()
        || request.bars.len() > MAX_BARS
        || request.profiles.is_empty()
        || request.profiles.len() > MAX_PROFILES
    {
        return Err("batch exceeds bar/profile limits or is empty".into());
    }
    let mut previous = 0;
    for bar in &request.bars {
        checkpoint()?;
        if bar.closed_at_ms <= previous
            || bar.closed_at_ms > request.as_of_ms
            || [bar.open, bar.high, bar.low, bar.close, bar.volume]
                .iter()
                .any(|v| !v.is_finite() || v.abs() > 1e100)
            || bar.low <= 0.0
            || bar.high < bar.low
            || bar.open < bar.low
            || bar.open > bar.high
            || bar.close < bar.low
            || bar.close > bar.high
            || bar.volume < 0.0
        {
            return Err("bars must be finite, valid, ordered and closed".into());
        }
        previous = bar.closed_at_ms;
    }
    let profiles = catalog()
        .into_iter()
        .map(|p| (p.id.clone(), p))
        .collect::<BTreeMap<_, _>>();
    let mut seen = BTreeSet::new();
    let mut results = Vec::new();
    for id in &request.profiles {
        if !seen.insert(id) {
            return Err("duplicate profile request".into());
        }
        let p = profiles.get(id).ok_or("unregistered indicator profile")?;
        let mut result = Calculation {
            profile_id: id.clone(),
            parameters: p.parameters.clone(),
            status: "ready".into(),
            values: BTreeMap::new(),
            error: None,
            minimum_bars: p.minimum_bars,
        };
        if request.bars.len() < p.minimum_bars {
            result.status = "insufficient_data".into();
            result.error = Some("warm_up_required".into());
        } else {
            match compute_controlled(id, &request.bars, &mut checkpoint) {
                Ok(values)
                    if values.len() == p.outputs.len() && values.iter().all(|v| v.is_finite()) =>
                {
                    result.values = p.outputs.iter().cloned().zip(values).collect()
                }
                Err(error) => return Err(error),
                Ok(_) => {
                    result.status = "failed".into();
                    result.error = Some("undefined_indicator_result".into());
                }
            }
        }
        checkpoint()?;
        results.push(result);
    }
    let input_hash = hash(request)?;
    let output_hash = hash(&(VERSION, &input_hash, &results))?;
    Ok(BatchResult {
        snapshot_id: request.snapshot_id.clone(),
        implementation_version: VERSION.into(),
        input_origin: "caller_supplied".into(),
        input_hash,
        output_hash,
        results,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn request() -> BatchRequest {
        BatchRequest {
            snapshot_id: "s1".into(),
            symbol: "TEST".into(),
            timeframe: "1m".into(),
            as_of_ms: 400_000,
            bars: (1..=300)
                .map(|i| Candle {
                    closed_at_ms: i * 1000,
                    open: 100.0 + i as f64,
                    high: 103.0 + i as f64,
                    low: 98.0 + i as f64,
                    close: 101.0 + i as f64,
                    volume: 100.0 + (i % 5) as f64,
                })
                .collect(),
            profiles: catalog().into_iter().map(|p| p.id).collect(),
        }
    }
    #[test]
    fn all_profiles_are_ready_and_hashed() {
        let r = request();
        let a = calculate(&r).unwrap();
        assert_eq!(a.results.len(), 45);
        assert!(
            a.results.iter().all(|r| r.status == "ready"),
            "{:?}",
            a.results
        );
        assert_eq!(a.output_hash, calculate(&r).unwrap().output_hash);
        for profile in catalog().into_iter().take(35) {
            let old = compute_legacy(&profile.id, &r.bars).unwrap();
            let new = compute(&profile.id, &r.bars).unwrap();
            assert_eq!(old, new, "legacy parity: {}", profile.id);
        }
        assert!(
            (a.results
                .iter()
                .find(|r| r.profile_id == "sma.5")
                .unwrap()
                .values["sma"]
                - 399.0)
                .abs()
                < 1e-8
        );
    }
    #[test]
    fn input_validation_and_warm_up_fail_closed() {
        let mut r = request();
        r.bars.truncate(10);
        assert!(calculate(&r)
            .unwrap()
            .results
            .iter()
            .any(|r| r.status == "insufficient_data"));
        r.bars[1].closed_at_ms = r.bars[0].closed_at_ms;
        assert!(calculate(&r).is_err());
        let mut r = request();
        r.profiles = vec!["execute_order".into()];
        assert!(calculate(&r).is_err());
    }
}
