//! Bounded wire adapter to the official TA-Lib Rust formula variants.
use super::*;
use crate::talib::{abstract_api as api, Core};

fn kind(info: &api::FuncInfo) -> &'static str {
    if info.inputs.iter().any(|i| i.kind == api::InputType::Price) {
        "Candle"
    } else if info.inputs.len() == 2 {
        "(f64, f64)"
    } else {
        "f64"
    }
}
pub(super) fn entries() -> Vec<Value> {
    api::funcs().map(|info| {
        let mut properties = serde_json::Map::new();
        let mut defaults = serde_json::Map::new();
        for opt in info.opt_inputs {
            let (schema, default) = match opt.kind {
                api::OptInputType::RealRange {min,max,default,..} => (json!({"type":"number","minimum":min.max(-1e6),"maximum":max.min(1e6)}),json!(default)),
                api::OptInputType::IntegerRange {min,max,default,..} => (json!({"type":"integer","minimum":min.max(-512),"maximum":max.min(512)}),json!(default)),
                api::OptInputType::IntegerList {values,default} => (json!({"type":"integer","enum":values.iter().map(|(v,_)|*v).collect::<Vec<_>>(),"minimum":values.iter().map(|(v,_)|*v).min(),"maximum":values.iter().map(|(v,_)|*v).max()}),json!(default)),
                api::OptInputType::RealList {values,default} => (json!({"type":"number","enum":values.iter().map(|(v,_)|*v).collect::<Vec<_>>(),"minimum":values.iter().map(|(v,_)|*v).fold(f64::INFINITY,f64::min),"maximum":values.iter().map(|(v,_)|*v).fold(f64::NEG_INFINITY,f64::max)}),json!(default)),
            };
            properties.insert(opt.param_name.into(),schema);
            defaults.insert(opt.param_name.into(),default);
        }
        let required = properties.keys().cloned().collect::<Vec<_>>();
        json!({"id":format!("talib.{}",info.name),"name":info.name,"category":info.group.as_str(),"input_type":kind(info),"output_type":if info.outputs.len()==1 {"number"} else {"object"},
            "output_fields":info.outputs.iter().map(|o|o.param_name).collect::<Vec<_>>(),
            "formula_variant":"ta-lib-rust-2f0426d-v1","source":format!("vendor/ta-lib-rust/ta_codegen/output/rust/library/src/ta_func/{}.rs",info.name.to_lowercase()),
            "documentation":info.hint,"parameters":{"type":"object","properties":properties,"required":required,"additionalProperties":false},"example_params":defaults,
            "warmup_rule":"native TA-Lib lookback + 1; original seeds and zero conventions","complexity":"bounded full-prefix replay per update; no claim of constant time"})
    }).collect()
}

#[derive(Clone)]
struct TaRunner {
    id: api::FuncId,
    operation: Operation,
    history: Vec<Value>,
    warmup: usize,
}
fn error(e: crate::talib::RetCode) -> TaError {
    invalid(&format!("TA-Lib: {e:?}"))
}
fn parameters(call: &mut api::ParamHolder<'_>, operation: &Operation) -> Result<(), TaError> {
    for (index, opt) in call.info().opt_inputs.iter().enumerate() {
        let value = &operation.params[opt.param_name];
        match opt.kind {
            api::OptInputType::IntegerRange { .. } | api::OptInputType::IntegerList { .. } => {
                call.set_opt(
                    index,
                    value
                        .as_i64()
                        .and_then(|v| i32::try_from(v).ok())
                        .ok_or_else(|| invalid("expected bounded integer"))?,
                )
                .map_err(error)?;
            }
            _ => {
                call.set_opt(
                    index,
                    value
                        .as_f64()
                        .ok_or_else(|| invalid("expected real parameter"))?,
                )
                .map_err(error)?;
            }
        }
    }
    Ok(())
}
pub(super) fn build(operation: &Operation) -> Result<Box<dyn Runner>, TaError> {
    let name = operation
        .id
        .strip_prefix("talib.")
        .ok_or_else(|| invalid("invalid TA-Lib id"))?;
    let id = api::get_func_handle(name).ok_or_else(|| invalid("unknown TA-Lib function"))?;
    let core = Core::new();
    let mut call = id.new_call(&core);
    parameters(&mut call, operation)?;
    let warmup = call
        .lookback()
        .map_err(error)?
        .checked_add(1)
        .ok_or_else(|| limit("lookback overflow"))?;
    if warmup > MAX_SAMPLES {
        return Err(limit("TA-Lib lookback exceeds history budget"));
    }
    Ok(Box::new(TaRunner {
        id,
        operation: operation.clone(),
        history: Vec::new(),
        warmup,
    }))
}
enum Output {
    Real(Vec<f64>),
    Integer(Vec<i32>),
}
impl Runner for TaRunner {
    fn validate(&self, sample: &Sample) -> Result<(), TaError> {
        match kind(self.id.info()) {
            "Candle" => input::decode::<w::Candle>(&sample.value, sample.at_ms).map(|_| ()),
            "(f64, f64)" => input::decode::<(f64, f64)>(&sample.value, sample.at_ms).map(|_| ()),
            _ => input::decode::<f64>(&sample.value, sample.at_ms).map(|_| ()),
        }
    }
    fn step(&mut self, sample: &Sample) -> Result<Option<Value>, TaError> {
        self.history.push(sample.value.clone());
        let n = self.history.len();
        if n < self.warmup {
            return Ok(None);
        }
        let info = self.id.info();
        let mut real = (0..info.inputs.len())
            .map(|_| Vec::with_capacity(n))
            .collect::<Vec<_>>();
        let mut integers = (0..info.inputs.len())
            .map(|_| Vec::with_capacity(n))
            .collect::<Vec<_>>();
        let mut price = (0..6).map(|_| Vec::with_capacity(n)).collect::<Vec<_>>();
        for value in &self.history {
            if kind(info) == "Candle" {
                for (i, key) in ["open", "high", "low", "close", "volume"]
                    .iter()
                    .enumerate()
                {
                    price[i].push(
                        value[key]
                            .as_f64()
                            .ok_or_else(|| invalid("missing candle component"))?,
                    );
                }
                for (i, input) in info.inputs.iter().enumerate() {
                    if input.kind == api::InputType::Real {
                        real[i].push(
                            value["close"]
                                .as_f64()
                                .ok_or_else(|| invalid("missing close input"))?,
                        );
                    }
                }
            } else {
                for (i, input) in info.inputs.iter().enumerate() {
                    let v = if info.inputs.len() == 1 {
                        value
                    } else {
                        &value[i]
                    };
                    let x = v.as_f64().ok_or_else(|| invalid("missing numeric input"))?;
                    if input.kind == api::InputType::Integer {
                        if x.fract() != 0.0 || x < i32::MIN as f64 || x > i32::MAX as f64 {
                            return Err(invalid("expected integer input"));
                        }
                        integers[i].push(x as i32);
                    } else {
                        real[i].push(x);
                    }
                }
            }
        }
        let core = Core::new();
        let mut call = self.id.new_call(&core);
        parameters(&mut call, &self.operation)?;
        for (i, input) in info.inputs.iter().enumerate() {
            match input.kind {
                api::InputType::Price => {
                    call.set_price_input(
                        i,
                        Some(&price[0]),
                        Some(&price[1]),
                        Some(&price[2]),
                        Some(&price[3]),
                        Some(&price[4]),
                        None,
                    )
                    .map_err(error)?;
                }
                api::InputType::Real => {
                    call.set_input(i, &real[i]).map_err(error)?;
                }
                api::InputType::Integer => {
                    call.set_int_input(i, &integers[i]).map_err(error)?;
                }
            }
        }
        let mut outputs = info
            .outputs
            .iter()
            .map(|o| match o.kind {
                api::OutputType::Real => Output::Real(vec![0.0; n]),
                api::OutputType::Integer => Output::Integer(vec![0; n]),
            })
            .collect::<Vec<_>>();
        for (i, output) in outputs.iter_mut().enumerate() {
            match output {
                Output::Real(v) => {
                    call.set_output(i, v).map_err(error)?;
                }
                Output::Integer(v) => {
                    call.set_int_output(i, v).map_err(error)?;
                }
            }
        }
        let range = call.call(0, n - 1).map_err(error)?;
        if range.count == 0 || range.beg_idx + range.count != n {
            return Ok(None);
        }
        let last = range.count - 1;
        let mut values = serde_json::Map::new();
        for (spec, output) in info.outputs.iter().zip(outputs) {
            values.insert(
                spec.param_name.into(),
                match output {
                    Output::Real(v) => json!(v[last]),
                    Output::Integer(v) => json!(v[last]),
                },
            );
        }
        Ok(Some(if info.outputs.len() == 1 {
            values
                .remove(info.outputs[0].param_name)
                .ok_or_else(|| invalid("missing TA-Lib output"))?
        } else {
            Value::Object(values)
        }))
    }
    fn warmup(&self) -> usize {
        self.warmup
    }
    fn clone_box(&self) -> Box<dyn Runner> {
        Box::new(self.clone())
    }
}
