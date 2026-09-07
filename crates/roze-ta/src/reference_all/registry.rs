use super::*;
mod generated {
    include!("registry_generated.rs");
}
pub(super) fn build(operation: &Operation) -> Result<Box<dyn Runner>, TaError> {
    if operation.id.starts_with("extra.") {
        extras::build(operation)
    } else {
        generated::build(operation)
    }
}

fn entries() -> Result<&'static [Value], TaError> {
    static ENTRIES: std::sync::OnceLock<Result<Vec<Value>, TaError>> = std::sync::OnceLock::new();
    ENTRIES
        .get_or_init(|| {
            let mut entries: Vec<Value> = serde_json::from_str(include_str!("inventory.json"))
                .map_err(|_| {
                    TaError::new(ErrorCode::EncodingFailed, "invalid reference inventory")
                })?;
            entries.extend(extras::entries());
            Ok(entries)
        })
        .as_ref()
        .map(Vec::as_slice)
        .map_err(Clone::clone)
}
pub(super) fn validate_params(operation: &Operation) -> Result<(), TaError> {
    let all = entries()?;
    let entry = all
        .iter()
        .find(|e| e["id"] == operation.id)
        .ok_or_else(|| {
            TaError::new(ErrorCode::UnsupportedProfile, "unknown reference indicator")
        })?;
    let params = operation
        .params
        .as_object()
        .ok_or_else(|| invalid("parameters must be an object"))?;
    let properties = entry["parameters"]["properties"]
        .as_object()
        .ok_or_else(|| invalid("invalid parameter inventory"))?;
    if params.len() != properties.len() || params.keys().any(|k| !properties.contains_key(k)) {
        return Err(invalid(
            "supply every constructor parameter exactly once, using reference_catalog",
        ));
    }
    for (key, schema) in properties {
        let value = &params[key];
        if schema["type"] == "string" {
            if !schema["enum"].as_array().is_some_and(|a| a.contains(value)) {
                return Err(invalid("invalid moving-average type"));
            }
        } else {
            let n = value
                .as_f64()
                .ok_or_else(|| invalid("constructor parameter must be a number"))?;
            if !n.is_finite()
                || n < schema["minimum"].as_f64().unwrap_or(0.0)
                || n > schema["maximum"].as_f64().unwrap_or(0.0)
                || (schema["type"] == "integer" && value.as_i64().is_none())
            {
                return Err(invalid(&format!(
                    "constructor parameter {key} exceeds its declared range"
                )));
            }
        }
    }
    Ok(())
}
pub(super) fn catalog() -> Result<Value, TaError> {
    Ok(
        json!({"schema_version":1,"implementation_version":VERSION,"entries":entries()?,
        "limits":{"samples":MAX_SAMPLES,"history_bytes":MAX_BYTES,"nested_items":MAX_ITEMS,"period":512},
        "snapshots":"bounded_history_replayed_through_validated_inputs",
        "time_unit":"UTC_milliseconds","readiness":"source_warmup_is_a_lower_bound_for_event_driven_outputs",
        "formula_policy":"explicit_Wickra_variants_not_TA_Lib_compatibility"}),
    )
}

pub(super) fn validate_domain(operation: &Operation, sample: &Sample) -> Result<(), TaError> {
    if matches!(
        operation.id.as_str(),
        "wickra.BipowerVariation"
            | "wickra.EwmaVolatility"
            | "wickra.Garch11"
            | "wickra.GeometricMa"
            | "wickra.HistoricalVolatility"
            | "wickra.JumpIndicator"
            | "wickra.LogReturn"
            | "wickra.RealizedVolatility"
            | "wickra.RegimeLabel"
            | "wickra.VolatilityOfVolatility"
            | "wickra.VolatilityCone"
    ) && sample.value.as_f64().is_some_and(|v| v <= 0.0)
    {
        return Err(invalid(
            "this logarithmic-price operation requires strictly positive prices",
        ));
    }
    Ok(())
}
/// Prevent explosive alternative-bar allocations before calling upstream.
pub(super) fn preflight_emission(
    operation: &Operation,
    sample: &Sample,
    previous: Option<&Sample>,
) -> Result<(), TaError> {
    if !matches!(
        operation.id.as_str(),
        "wickra.RenkoBars" | "wickra.PointAndFigureBars" | "wickra.KagiBars"
    ) {
        return Ok(());
    }
    let candle: w::Candle = input::decode(&sample.value, sample.at_ms)?;
    let prior = previous
        .and_then(|s| s.value["close"].as_f64())
        .unwrap_or(candle.open);
    let scale = operation.params.as_object().and_then(|o| {
        o.iter()
            .find(|(k, _)| k.contains("size") || k.contains("box") || k.contains("reversal"))
            .and_then(|(_, v)| v.as_f64())
    });
    if let Some(scale) = scale {
        if scale <= 0.0 || (candle.high - candle.low + (candle.close - prior).abs()) / scale > 64.0
        {
            return Err(limit(
                "alternative-bar price move exceeds the bounded emission budget",
            ));
        }
    } else {
        return Err(invalid("missing alternative-bar size parameter"));
    }
    Ok(())
}
