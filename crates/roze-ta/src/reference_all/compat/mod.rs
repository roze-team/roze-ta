//! Explicit source variants; formulas and alignment are specified separately.
mod math;
mod python;
mod stateful;
mod ttr;
use super::*;
use math::*;

pub(super) fn entries() -> Result<Vec<Value>, TaError> {
    serde_json::from_str(include_str!("inventory.json"))
        .map_err(|_| invalid("invalid source variant inventory"))
}
pub(super) fn build(operation: &Operation) -> Result<Box<dyn Runner>, TaError> {
    let entry = entries()?
        .into_iter()
        .find(|e| e["id"] == operation.id)
        .ok_or_else(|| invalid("unknown source variant"))?;
    let source = entry["source_library"]
        .as_str()
        .ok_or_else(|| invalid("missing source"))?
        .to_owned();
    let name = entry["name"]
        .as_str()
        .ok_or_else(|| invalid("missing source name"))?
        .to_owned();
    let base_id = entry["base_operation"]["id"]
        .as_str()
        .ok_or_else(|| invalid("missing base"))?;
    let base = registry::build(&Operation {
        id: base_id.into(),
        params: operation.params.clone(),
    })?;
    Ok(Box::new(SourceRunner {
        source,
        name,
        kind: entry["input_type"]
            .as_str()
            .ok_or_else(|| invalid("missing kind"))?
            .into(),
        params: operation.params.clone(),
        base,
        history: Vec::new(),
        outputs: Vec::new(),
    }))
}
struct SourceRunner {
    source: String,
    name: String,
    kind: String,
    params: Value,
    base: Box<dyn Runner>,
    history: Vec<Sample>,
    outputs: Vec<Value>,
}
impl Runner for SourceRunner {
    fn validate(&self, s: &Sample) -> Result<(), TaError> {
        match self.kind.as_str() {
            "Candle" => input::decode::<w::Candle>(&s.value, s.at_ms).map(|_| ()),
            "(f64, f64)" => input::decode::<(f64, f64)>(&s.value, s.at_ms).map(|_| ()),
            _ => input::decode::<f64>(&s.value, s.at_ms).map(|_| ()),
        }
    }
    fn step(&mut self, s: &Sample) -> Result<Option<Value>, TaError> {
        self.history.push(s.clone());
        let base = if self.name == "FibonacciRetracement" {
            Value::Null
        } else {
            self.base.step(s)?.unwrap_or(Value::Null)
        };
        self.outputs.push(base);
        let data = Data::new(&self.history, &self.kind);
        let result = if self.source == "ttr" {
            ttr::evaluate(&self.name, &self.params, &data, &self.outputs)
        } else {
            python::evaluate(&self.source, &self.name, &self.params, &data, &self.outputs)
        }?;
        Ok(Some(result))
    }
    fn warmup(&self) -> usize {
        self.base.warmup()
    }
    fn clone_box(&self) -> Box<dyn Runner> {
        Box::new(Self {
            source: self.source.clone(),
            name: self.name.clone(),
            kind: self.kind.clone(),
            params: self.params.clone(),
            base: self.base.clone_box(),
            history: self.history.clone(),
            outputs: self.outputs.clone(),
        })
    }
}
struct Data {
    times: Vec<i64>,
    x: Vec<f64>,
    y: Vec<f64>,
    o: Vec<f64>,
    h: Vec<f64>,
    l: Vec<f64>,
    c: Vec<f64>,
    v: Vec<f64>,
}
impl Data {
    fn new(samples: &[Sample], kind: &str) -> Self {
        let field = |key: &str| {
            samples
                .iter()
                .map(|s| s.value[key].as_f64().unwrap_or(f64::NAN))
                .collect::<Vec<_>>()
        };
        let x = samples
            .iter()
            .map(|s| {
                if kind == "Candle" {
                    s.value["close"].as_f64()
                } else if kind == "(f64, f64)" {
                    s.value[0].as_f64()
                } else {
                    s.value.as_f64()
                }
                .unwrap_or(f64::NAN)
            })
            .collect::<Vec<_>>();
        Self {
            times: samples.iter().map(|s| s.at_ms).collect(),
            c: x.clone(),
            x,
            y: samples
                .iter()
                .map(|s| s.value[1].as_f64().unwrap_or(f64::NAN))
                .collect(),
            o: field("open"),
            h: field("high"),
            l: field("low"),
            v: field("volume"),
        }
    }
    fn len(&self) -> usize {
        self.x.len()
    }
    fn tp(&self) -> Vec<f64> {
        (0..self.len())
            .map(|i| (self.h[i] + self.l[i] + self.c[i]) / 3.)
            .collect()
    }
    fn tr(&self, first: bool) -> Vec<f64> {
        (0..self.len())
            .map(|i| {
                if i == 0 {
                    if first {
                        self.h[i] - self.l[i]
                    } else {
                        f64::NAN
                    }
                } else {
                    self.h[i].max(self.c[i - 1]) - self.l[i].min(self.c[i - 1])
                }
            })
            .collect()
    }
}
fn number(p: &Value, key: &str, default: f64) -> f64 {
    p[key].as_f64().unwrap_or(default)
}
fn period(p: &Value, key: &str, default: usize) -> usize {
    p[key]
        .as_u64()
        .and_then(|n| usize::try_from(n).ok())
        .unwrap_or(default)
}
fn last(a: &[f64]) -> f64 {
    a.last().copied().unwrap_or(f64::NAN)
}
fn obj(fields: &[(&str, f64)]) -> Value {
    Value::Object(
        fields
            .iter()
            .map(|(k, v)| ((*k).into(), json!(v)))
            .collect(),
    )
}
fn fallback(base: &[Value]) -> Value {
    base.last().cloned().unwrap_or(Value::Null)
}
