use super::*;

fn integer() -> Value {
    json!({"type":"integer","minimum":0,"maximum":254})
}
fn number() -> Value {
    json!({"type":"number","minimum":-1e100,"maximum":1e100})
}
fn source() -> Value {
    json!({"type":"string","enum":["close","open","high","low","hl2","tp","volume","volumed_price"]})
}
fn ma() -> Value {
    let choices=["sma","wma","hma","rma","ema","dma","dema","tma","tema","wsma","smm","swma","trima","lin_reg","vidya"].iter().map(|id|json!({"type":"object","properties":{*id:integer()},"required":[*id],"additionalProperties":false})).collect::<Vec<_>>();
    json!({"oneOf":choices})
}
fn object(properties: Value) -> Value {
    json!({"type":"object","properties":properties,"additionalProperties":false})
}
fn method_params(entry: &Value) -> Result<(Value, Value), TaError> {
    Ok(
        match entry["params_type"]
            .as_str()
            .ok_or_else(|| invalid("missing method parameter type"))?
        {
            "PeriodType" | "usize" => (json!({"period":14}), object(json!({"period":integer()}))),
            "()" => (json!({}), object(json!({}))),
            "Vec<ValueType>" => (
                json!({"weights":[1.0,2.0,3.0]}),
                object(
                    json!({"weights":{"type":"array","items":number(),"minItems":1,"maxItems":254}}),
                ),
            ),
            "(PeriodType, PeriodType)" => (
                json!({"first":2,"second":3}),
                object(json!({"first":integer(),"second":integer()})),
            ),
            "(ValueType, Source)" => (
                json!({"size":0.01,"source":"close"}),
                object(
                    json!({"size":{"type":"number","exclusiveMinimum":0,"exclusiveMaximum":1},"source":source()}),
                ),
            ),
            _ => return Err(invalid("unknown registry parameter type")),
        },
    )
}

pub(super) fn entries() -> Result<Vec<Value>, TaError> {
    let mut entries: Vec<Value> = serde_json::from_str(include_str!("inventory.json"))
        .map_err(|_| invalid("native inventory is invalid"))?;
    for entry in &mut entries {
        let (defaults, schema) = if entry["kind"] == "indicator" {
            let defaults = indicators::defaults(
                entry["module"]
                    .as_str()
                    .ok_or_else(|| invalid("missing module"))?,
            )?;
            let mut fields = serde_json::Map::new();
            for (name, ty) in entry["params_fields"]
                .as_object()
                .ok_or_else(|| invalid("missing parameters"))?
            {
                let schema = match ty.as_str() {
                    Some("M") => ma(),
                    Some("PeriodType" | "u8") => integer(),
                    Some("ValueType") => number(),
                    Some("Source") => source(),
                    Some("bool") => json!({"type":"boolean"}),
                    _ => return Err(invalid("unknown indicator parameter type")),
                };
                fields.insert(name.clone(), schema);
            }
            (defaults, object(Value::Object(fields)))
        } else {
            method_params(entry)?
        };
        // The root override object is partial; resolved parameters always contain all defaults.
        entry["defaults"] = defaults;
        entry["parameters_schema"] = schema;
        entry["tool"] = json!("native_batch_calculate");
        entry["online_update"] = json!(false);
        entry["initialization"] = json!(
            "new(first), then next on every sample including first; exactly native over semantics"
        );
        entry["verification_status"] = json!("native_parity; not an audited Profile");
        entry["output_contract"]=json!("native-v1; array order, native units and parameter relations are documented in documentation/source");
    }
    Ok(entries)
}

/// Complete executable native inventory. Aliases do not count as separate algorithms.
pub fn catalog() -> Result<Value, TaError> {
    let entries = entries()?;
    Ok(
        json!({"schema_version":1,"implementation_version":VERSION,"entries":entries,
        "counts":{"indicators":entries.iter().filter(|v|v["kind"]=="indicator").count(),"methods":entries.iter().filter(|v|v["kind"]=="method").count()},
        "limits":{"samples":MAX_SAMPLES,"operations":MAX_OPERATIONS,"output_items":MAX_OUTPUT_ITEMS,"renko_bricks_per_sample":MAX_RENKO_BRICKS,"period_max":254,"json_value_bytes":MAX_JSON_VALUE_BYTES,"retained_row_bytes":MAX_RETAINED_ROW_BYTES},
        "existing_profile_tool":"indicator_batch_calculate_v2","existing_analysis_tool":"analysis_batch_calculate",
        "documentation_provenance":{"license":"Apache-2.0","source":"Yata v0.7.0 derived native modules","notice":"THIRD-PARTY-NOTICES.md"},
        "contract":"native-v1; seeded outputs, emission timestamps, no future backfill; see docs/usage/native-mcp.md"}),
    )
}

// Only the small schema vocabulary produced above is accepted; no executable expressions.
fn matches(v: &Value, s: &Value) -> bool {
    if let Some(choices) = s["oneOf"].as_array() {
        return choices.iter().filter(|s| matches(v, s)).count() == 1;
    }
    if let Some(options) = s["enum"].as_array() {
        if !options.contains(v) {
            return false;
        }
    }
    match s["type"].as_str() {
        Some("object") => {
            let Some(map) = v.as_object() else {
                return false;
            };
            let Some(properties) = s["properties"].as_object() else {
                return false;
            };
            if let Some(required) = s["required"].as_array() {
                if required
                    .iter()
                    .any(|k| k.as_str().is_none_or(|k| !map.contains_key(k)))
                {
                    return false;
                }
            }
            map.iter()
                .all(|(key, v)| properties.get(key).is_some_and(|s| matches(v, s)))
        }
        Some("integer" | "number") => {
            if s["type"] == "integer" && v.as_u64().is_none() {
                return false;
            }
            let Some(n) = v.as_f64() else {
                return false;
            };
            n.is_finite()
                && s["minimum"].as_f64().is_none_or(|m| n >= m)
                && s["maximum"].as_f64().is_none_or(|m| n <= m)
                && s["exclusiveMinimum"].as_f64().is_none_or(|m| n > m)
                && s["exclusiveMaximum"].as_f64().is_none_or(|m| n < m)
        }
        Some("string") => v.is_string(),
        Some("boolean") => v.is_boolean(),
        Some("array") => v.as_array().is_some_and(|a| {
            s["minItems"].as_u64().is_none_or(|m| a.len() >= m as usize)
                && s["maxItems"].as_u64().is_none_or(|m| a.len() <= m as usize)
                && a.iter().all(|v| matches(v, &s["items"]))
        }),
        _ => false,
    }
}

pub(super) fn resolve(entry: &Value, overrides: &Value) -> Result<Value, TaError> {
    if !matches(overrides, &entry["parameters_schema"]) {
        return Err(invalid("parameters do not match the native catalog schema"));
    }
    let mut params = entry["defaults"]
        .as_object()
        .cloned()
        .ok_or_else(|| invalid("missing defaults"))?;
    for (key, value) in overrides
        .as_object()
        .ok_or_else(|| invalid("parameters must be an object"))?
    {
        params.insert(key.clone(), value.clone());
    }
    let result = Value::Object(params);
    if !matches(&result, &entry["parameters_schema"]) {
        return Err(invalid("resolved parameters violate schema"));
    }
    if entry["module"] == "reversal" {
        let a = result["first"].as_u64().unwrap_or(0);
        let b = result["second"].as_u64().unwrap_or(0);
        if a == 0 || b == 0 || a + b + 1 > 254 {
            return Err(invalid(
                "reversal requires positive first/second and first+second+1 <=254",
            ));
        }
    }
    Ok(result)
}
