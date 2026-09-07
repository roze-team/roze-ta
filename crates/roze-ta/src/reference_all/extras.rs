//! Additional mathematical and composed indicator variants.
//! Implemented from the formula cards in docs/contracts/reference-extras.md.
use super::*;
use w::Indicator;

const UNARY: &[&str] = &[
    "Acos", "Asin", "Atan", "Ceil", "Cos", "Cosh", "Exp", "Floor", "Ln", "Log10", "Sin", "Sinh",
    "Sqrt", "Tan", "Tanh",
];
const BINARY: &[&str] = &["Add", "Subtract", "Multiply", "Divide"];
const ROLLING: &[&str] = &[
    "Sum",
    "Minimum",
    "Maximum",
    "MinIndex",
    "MaxIndex",
    "MinMax",
    "MinMaxIndex",
    "MeanDeviation",
    "RelativeVolume",
    "EfficiencyRatio",
    "PositiveCmo",
    "WilderSum",
    "Lag",
    "Lags",
    "SingleFactorModel",
    "VariableSma",
    "SignalNoiseRatio",
    "TrendDetectionIndex",
    "SmoothedObv",
    "Sfx",
    "PriceBands",
    "AverageBarRange",
    "SmoothedCmo",
    "RelativeVolatilityIndex",
    "SlowStochastic",
    "SampleVariance",
    "SampleStdDev",
    "SampleCovariance",
    "ScaledMedianDeviation",
];
const OTHER: &[&str] = &[
    "CumulativeSum",
    "CumulativeReturn",
    "InternalBarStrength",
    "CloseLocationValue",
    "Guppy",
    "Growth",
    "Kdj",
    "Dvi",
    "PivotsHL",
    "PercentageVolumeOscillator",
    "BlackCrows",
    "WhiteSoldiers",
    "DarkCloudCover",
    "Piercing",
    "EveningStar",
    "MorningStar",
    "RiseFallThreeMethods",
    "SideGapThreeMethods",
];

fn input_kind(name: &str) -> &'static str {
    if BINARY.contains(&name)
        || [
            "SingleFactorModel",
            "VariableSma",
            "Growth",
            "SampleCovariance",
        ]
        .contains(&name)
    {
        "(f64, f64)"
    } else if [
        "InternalBarStrength",
        "CloseLocationValue",
        "SignalNoiseRatio",
        "SmoothedObv",
        "Sfx",
        "Kdj",
        "PivotsHL",
        "AverageBarRange",
        "SlowStochastic",
        "BlackCrows",
        "WhiteSoldiers",
        "DarkCloudCover",
        "Piercing",
        "EveningStar",
        "MorningStar",
        "RiseFallThreeMethods",
        "SideGapThreeMethods",
    ]
    .contains(&name)
    {
        "Candle"
    } else {
        "f64"
    }
}
pub(super) fn entries() -> Vec<Value> {
    UNARY.iter().chain(BINARY).chain(ROLLING).chain(OTHER).map(|name| {
        let params = if ROLLING.contains(name) {json!({"period":{"type":"integer","minimum":2,"maximum":128}})} else {json!({})};
        let required = if ROLLING.contains(name) {json!(["period"])} else {json!([])};
        json!({"id":format!("extra.{name}"),"name":name,"kind":"Indicator","input_type":input_kind(name),"output_type":"documented_named_values",
            "parameters":{"type":"object","properties":params,"required":required,"additionalProperties":false},
            "example_params":if ROLLING.contains(name) {json!({"period":5})} else {json!({})},
            "formula_variant":"roze-reference-extras-v1","documentation":"See docs/contracts/reference-extras.md for formulas, seed, period and output semantics.",
            "source":"crates/roze-ta/src/reference_all/extras.rs"})
    }).collect()
}

#[derive(Clone)]
struct Extra {
    name: String,
    period: usize,
    x: Vec<f64>,
    y: Vec<f64>,
    candles: Vec<w::Candle>,
    atr: w::Atr,
    obv: w::Obv,
    ma: w::Sma,
    std: w::StdDev,
    ppo: w::Ppo,
    signal: w::Ema,
    guppy: Vec<w::Ema>,
    cumulative: f64,
    auxiliary: Vec<f64>,
    pivot: Option<(w::Candle, bool)>,
}
pub(super) fn build(operation: &Operation) -> Result<Box<dyn Runner>, TaError> {
    let name = operation
        .id
        .strip_prefix("extra.")
        .ok_or_else(|| invalid("invalid extra indicator"))?;
    if !UNARY
        .iter()
        .chain(BINARY)
        .chain(ROLLING)
        .chain(OTHER)
        .any(|n| *n == name)
    {
        return Err(invalid("unknown extra indicator"));
    }
    let period = operation.params["period"].as_u64().unwrap_or(14) as usize;
    let pattern = match name {
        "BlackCrows" => Some((indicator(w::ThreeSoldiersOrCrows::new()), None, -1)),
        "WhiteSoldiers" => Some((indicator(w::ThreeSoldiersOrCrows::new()), None, 1)),
        "DarkCloudCover" => Some((indicator(w::PiercingDarkCloud::new()), None, -1)),
        "Piercing" => Some((indicator(w::PiercingDarkCloud::new()), None, 1)),
        "EveningStar" => Some((indicator(w::MorningEveningStar::new()), None, -1)),
        "MorningStar" => Some((indicator(w::MorningEveningStar::new()), None, 1)),
        "RiseFallThreeMethods" => Some((
            indicator(w::RisingThreeMethods::new()),
            Some(indicator(w::FallingThreeMethods::new())),
            0,
        )),
        "SideGapThreeMethods" => Some((
            indicator(w::UpsideGapThreeMethods::new()),
            Some(indicator(w::DownsideGapThreeMethods::new())),
            0,
        )),
        _ => None,
    };
    if let Some((first, second, direction)) = pattern {
        return Ok(Box::new(Pattern {
            first,
            second,
            direction,
        }));
    }
    Ok(Box::new(Extra {
        name: name.into(),
        period,
        x: Vec::new(),
        y: Vec::new(),
        candles: Vec::new(),
        atr: w::Atr::new(period).map_err(upstream)?,
        obv: w::Obv::new(),
        ma: w::Sma::new(period).map_err(upstream)?,
        std: w::StdDev::new(if name == "RelativeVolatilityIndex" {
            10
        } else {
            period
        })
        .map_err(upstream)?,
        ppo: w::Ppo::new(12, 26).map_err(upstream)?,
        signal: w::Ema::new(9).map_err(upstream)?,
        guppy: [3, 5, 8, 10, 12, 15, 30, 35, 40, 45, 50, 60]
            .into_iter()
            .map(|n| w::Ema::new(n).map_err(upstream))
            .collect::<Result<Vec<_>, _>>()?,
        cumulative: if name == "Growth" { 1.0 } else { 0.0 },
        auxiliary: Vec::new(),
        pivot: None,
    }))
}
fn mean(a: &[f64]) -> f64 {
    a.iter().sum::<f64>() / a.len() as f64
}
fn window(a: &[f64], n: usize) -> Option<&[f64]> {
    (a.len() >= n).then(|| &a[a.len() - n..])
}
fn ratio(a: f64, b: f64) -> f64 {
    if b == 0.0 {
        f64::NAN
    } else {
        a / b
    }
}
fn rank(a: &[f64]) -> f64 {
    let last = a[a.len() - 1];
    a.iter().filter(|x| **x <= last).count() as f64 / a.len() as f64
}
fn wilder_average(a: &[f64], n: usize) -> Option<f64> {
    if a.len() < n {
        return None;
    }
    Some(
        a[n..]
            .iter()
            .fold(mean(&a[..n]), |avg, v| avg + (v - avg) / n as f64),
    )
}
impl Runner for Extra {
    fn validate(&self, s: &Sample) -> Result<(), TaError> {
        match input_kind(&self.name) {
            "Candle" => input::decode::<w::Candle>(&s.value, s.at_ms).map(|_| ()),
            "(f64, f64)" => input::decode::<(f64, f64)>(&s.value, s.at_ms).map(|_| ()),
            _ => input::decode::<f64>(&s.value, s.at_ms).map(|_| ()),
        }
    }
    fn step(&mut self, s: &Sample) -> Result<Option<Value>, TaError> {
        match input_kind(&self.name) {
            "Candle" => {
                let c: w::Candle = input::decode(&s.value, s.at_ms)?;
                self.x.push(c.close);
                self.candles.push(c);
            }
            "(f64, f64)" => {
                let (x, y) = input::decode::<(f64, f64)>(&s.value, s.at_ms)?;
                self.x.push(x);
                self.y.push(y);
            }
            _ => self.x.push(input::decode::<f64>(&s.value, s.at_ms)?),
        }
        let x = self.x[self.x.len() - 1];
        let n = self.period;
        let scalar = match self.name.as_str() {
            "Acos" => x.acos(),
            "Asin" => x.asin(),
            "Atan" => x.atan(),
            "Ceil" => x.ceil(),
            "Cos" => x.cos(),
            "Cosh" => x.cosh(),
            "Exp" => x.exp(),
            "Floor" => x.floor(),
            "Ln" => x.ln(),
            "Log10" => x.log10(),
            "Sin" => x.sin(),
            "Sinh" => x.sinh(),
            "Sqrt" => x.sqrt(),
            "Tan" => x.tan(),
            "Tanh" => x.tanh(),
            "Add" => x + self.y[self.y.len() - 1],
            "Subtract" => x - self.y[self.y.len() - 1],
            "Multiply" => x * self.y[self.y.len() - 1],
            "Divide" => ratio(x, self.y[self.y.len() - 1]),
            "CumulativeSum" => {
                self.cumulative += x;
                self.cumulative
            }
            "CumulativeReturn" => 100.0 * (ratio(x, self.x[0]) - 1.0),
            "Growth" => {
                if self.x.len() > 1 {
                    let i = self.x.len() - 1;
                    self.cumulative *= 1.0 + self.y[i - 1] * (ratio(x, self.x[i - 1]) - 1.0);
                }
                self.cumulative
            }
            "InternalBarStrength" | "CloseLocationValue" => {
                let c = self.candles[self.candles.len() - 1];
                if self.name == "InternalBarStrength" {
                    ratio(c.close - c.low, c.high - c.low)
                } else {
                    ratio(2.0 * c.close - c.high - c.low, c.high - c.low)
                }
            }
            "Lag" | "Lags" => {
                if self.x.len() <= n {
                    return Ok(None);
                }
                if self.name == "Lags" {
                    return Ok(Some(json!((1..=n)
                        .map(|lag| self.x[self.x.len() - lag - 1])
                        .collect::<Vec<_>>())));
                }
                self.x[self.x.len() - n - 1]
            }
            "Guppy" => {
                let values: Vec<_> = self.guppy.iter_mut().map(|e| e.update(x)).collect();
                if values.iter().any(Option::is_none) {
                    return Ok(None);
                }
                return encode(values).map(Some);
            }
            "SmoothedObv" => {
                let c = self.candles[self.candles.len() - 1];
                return self
                    .obv
                    .update(c)
                    .and_then(|o| self.ma.update(o))
                    .map(encode)
                    .transpose();
            }
            "AverageBarRange" => {
                let c = self.candles[self.candles.len() - 1];
                return self.ma.update(c.high - c.low).map(encode).transpose();
            }
            "SmoothedCmo" | "RelativeVolatilityIndex" => {
                if self.x.len() < 2 {
                    return Ok(None);
                }
                let change = x - self.x[self.x.len() - 2];
                let (up, down) = if self.name == "SmoothedCmo" {
                    (change.max(0.0), (-change).max(0.0))
                } else {
                    // Include the first close in the population standard deviation.
                    if self.x.len() == 2 {
                        self.std.update(self.x[0]);
                    }
                    let Some(sd) = self.std.update(x) else {
                        return Ok(None);
                    };
                    (
                        if change > 0.0 { sd } else { 0.0 },
                        if change < 0.0 { sd } else { 0.0 },
                    )
                };
                self.y.push(up);
                self.auxiliary.push(down);
                let (Some(u), Some(d)) = (
                    wilder_average(&self.y, n),
                    wilder_average(&self.auxiliary, n),
                ) else {
                    return Ok(None);
                };
                if self.name == "SmoothedCmo" {
                    100.0 * ratio(u - d, u + d)
                } else {
                    100.0 * ratio(u, u + d)
                }
            }
            "PercentageVolumeOscillator" => {
                if x < 0.0 {
                    return Err(invalid("volume must be non-negative"));
                }
                let Some(ppo) = self.ppo.update(x) else {
                    return Ok(None);
                };
                let Some(signal) = self.signal.update(ppo) else {
                    return Ok(None);
                };
                return Ok(Some(
                    json!({"pvo":ppo,"signal":signal,"histogram":ppo-signal}),
                ));
            }
            "SignalNoiseRatio" | "Sfx" => {
                let c = self.candles[self.candles.len() - 1];
                let atr = self.atr.update(c);
                if self.name == "Sfx" {
                    let sd = self.std.update(x);
                    let smooth = sd.and_then(|v| self.ma.update(v));
                    return match (atr, sd, smooth) {
                        (Some(a), Some(d), Some(m)) => {
                            Ok(Some(json!({"atr":a,"std_dev":d,"ma_std_dev":m})))
                        }
                        _ => Ok(None),
                    };
                }
                if self.x.len() <= n {
                    return Ok(None);
                }
                match atr {
                    Some(a) => ratio((x - self.x[self.x.len() - n - 1]).abs(), a),
                    None => return Ok(None),
                }
            }
            "VariableSma" => {
                let selected = self.y[self.y.len() - 1];
                if selected.fract() != 0.0 || selected < 2.0 || selected > n as f64 {
                    return Err(invalid(
                        "VariableSma period input must be an integer in 2..period",
                    ));
                }
                match window(&self.x, selected as usize) {
                    Some(a) => mean(a),
                    None => return Ok(None),
                }
            }
            "TrendDetectionIndex" => {
                if self.x.len() <= n {
                    return Ok(None);
                }
                self.auxiliary.push(x - self.x[self.x.len() - n - 1]);
                let Some(long) = window(&self.auxiliary, 2 * n) else {
                    return Ok(None);
                };
                let short = &long[n..];
                let di: f64 = short.iter().sum();
                return Ok(Some(
                    json!({"tdi":di.abs()-(long.iter().map(|v|v.abs()).sum::<f64>()-short.iter().map(|v|v.abs()).sum::<f64>()),"di":di}),
                ));
            }
            "SlowStochastic" => {
                if self.candles.len() < n {
                    return Ok(None);
                }
                let a = &self.candles[self.candles.len() - n..];
                let high = a.iter().map(|c| c.high).fold(f64::NEG_INFINITY, f64::max);
                let low = a.iter().map(|c| c.low).fold(f64::INFINITY, f64::min);
                self.auxiliary.push(100.0 * ratio(x - low, high - low));
                let Some(k) = window(&self.auxiliary, 3) else {
                    return Ok(None);
                };
                let k = mean(k);
                self.y.push(k);
                let Some(d) = window(&self.y, 3) else {
                    return Ok(None);
                };
                return Ok(Some(json!({"k":k,"d":mean(d)})));
            }
            "PivotsHL" => return self.pivot_step(),
            "Kdj" => return self.kdj_step(),
            "Dvi" => return self.dvi_step(),
            "SingleFactorModel" | "SampleCovariance" => {
                let (Some(a), Some(b)) = (window(&self.x, n), window(&self.y, n)) else {
                    return Ok(None);
                };
                let (ma, mb) = (mean(a), mean(b));
                let cov = a
                    .iter()
                    .zip(b)
                    .map(|(x, y)| (x - ma) * (y - mb))
                    .sum::<f64>();
                if self.name == "SampleCovariance" {
                    return encode(cov / (n - 1) as f64).map(Some);
                }
                let va = a.iter().map(|x| (x - ma).powi(2)).sum::<f64>();
                let vb = b.iter().map(|x| (x - mb).powi(2)).sum::<f64>();
                let beta = ratio(cov, vb);
                return Ok(Some(
                    json!({"alpha":ma-beta*mb,"beta":beta,"r_squared":ratio(cov*cov,va*vb)}),
                ));
            }
            _ => {
                let Some(a) = window(&self.x, n) else {
                    return Ok(None);
                };
                let avg = mean(a);
                let min = a.iter().copied().fold(f64::INFINITY, f64::min);
                let max = a.iter().copied().fold(f64::NEG_INFINITY, f64::max);
                let min_i = a.iter().rposition(|v| *v == min).unwrap_or(0) + self.x.len() - n;
                let max_i = a.iter().rposition(|v| *v == max).unwrap_or(0) + self.x.len() - n;
                match self.name.as_str() {
                    "Sum" => a.iter().sum(),
                    "Minimum" => min,
                    "Maximum" => max,
                    "MinIndex" => min_i as f64,
                    "MaxIndex" => max_i as f64,
                    "MinMax" => return Ok(Some(json!({"min":min,"max":max}))),
                    "MinMaxIndex" => return Ok(Some(json!({"min_index":min_i,"max_index":max_i}))),
                    "MeanDeviation" => a.iter().map(|v| (v - avg).abs()).sum::<f64>() / n as f64,
                    "SampleVariance" => {
                        a.iter().map(|v| (v - avg).powi(2)).sum::<f64>() / (n - 1) as f64
                    }
                    "SampleStdDev" => {
                        (a.iter().map(|v| (v - avg).powi(2)).sum::<f64>() / (n - 1) as f64).sqrt()
                    }
                    "ScaledMedianDeviation" => {
                        fn median(a: &[f64]) -> f64 {
                            let mut v = a.to_vec();
                            v.sort_by(f64::total_cmp);
                            (v[(v.len() - 1) / 2] + v[v.len() / 2]) / 2.0
                        }
                        let center = median(a);
                        1.4826 * median(&a.iter().map(|v| (v - center).abs()).collect::<Vec<_>>())
                    }
                    "RelativeVolume" => ratio(x, avg),
                    "EfficiencyRatio" | "PositiveCmo" => {
                        let Some(a) = window(&self.x, n + 1) else {
                            return Ok(None);
                        };
                        let moves = a.windows(2).map(|w| w[1] - w[0]).collect::<Vec<_>>();
                        let denom = moves.iter().map(|v| v.abs()).sum();
                        if self.name == "EfficiencyRatio" {
                            ratio((a[n] - a[0]).abs(), denom)
                        } else {
                            100.0 * ratio(moves.iter().map(|v| v.max(0.0)).sum(), denom)
                        }
                    }
                    "WilderSum" => {
                        self.cumulative = if self.x.len() == n {
                            a.iter().sum()
                        } else {
                            self.cumulative * (1.0 - 1.0 / n as f64) + x
                        };
                        self.cumulative
                    }
                    "PriceBands" => {
                        self.auxiliary.push(avg - mean(&a[n - 2..]));
                        let Some(residuals) = window(&self.auxiliary, n) else {
                            return Ok(None);
                        };
                        let center_residual = mean(residuals);
                        let width = 2.0
                            * (residuals
                                .iter()
                                .map(|v| (v - center_residual).powi(2))
                                .sum::<f64>()
                                / n as f64)
                                .sqrt();
                        return Ok(Some(
                            json!({"lower":avg-width,"center":avg,"upper":avg+width}),
                        ));
                    }
                    _ => return Err(invalid("unimplemented extra dispatch")),
                }
            }
        };
        encode(scalar).map(Some)
    }
    fn warmup(&self) -> usize {
        match self.name.as_str() {
            "Guppy" => 60,
            "PercentageVolumeOscillator" => 34,
            "Kdj" => 9,
            "SmoothedCmo" => self.period + 1,
            "RelativeVolatilityIndex" => self.period + 9,
            "SlowStochastic" => self.period + 4,
            "Dvi" => 357,
            "TrendDetectionIndex" => 3 * self.period,
            "Sfx" | "PriceBands" => 2 * self.period - 1,
            "SignalNoiseRatio" | "EfficiencyRatio" | "PositiveCmo" | "Lag" | "Lags" => {
                self.period + 1
            }
            name if ROLLING.contains(&name) => self.period,
            _ => 1,
        }
    }
    fn clone_box(&self) -> Box<dyn Runner> {
        Box::new(self.clone())
    }
}

struct Pattern {
    first: Box<dyn Runner>,
    second: Option<Box<dyn Runner>>,
    direction: i8,
}
impl Runner for Pattern {
    fn validate(&self, s: &Sample) -> Result<(), TaError> {
        self.first.validate(s)
    }
    fn step(&mut self, s: &Sample) -> Result<Option<Value>, TaError> {
        let a = self.first.step(s)?;
        let b = match &mut self.second {
            Some(r) => r.step(s)?,
            None => None,
        };
        if a.is_none() && b.is_none() {
            return Ok(None);
        }
        let value = a.as_ref().and_then(Value::as_f64).unwrap_or(0.0)
            + b.as_ref().and_then(Value::as_f64).unwrap_or(0.0);
        let selected = match self.direction {
            1 => value.max(0.0),
            -1 => value.min(0.0),
            _ => value,
        };
        encode(selected).map(Some)
    }
    fn warmup(&self) -> usize {
        self.first
            .warmup()
            .max(self.second.as_ref().map_or(0, |s| s.warmup()))
    }
    fn clone_box(&self) -> Box<dyn Runner> {
        Box::new(Self {
            first: self.first.clone_box(),
            second: self.second.as_ref().map(|s| s.clone_box()),
            direction: self.direction,
        })
    }
}
impl Extra {
    fn pivot_step(&mut self) -> Result<Option<Value>, TaError> {
        let c = self.candles[self.candles.len() - 1];
        let start = self.candles.len().saturating_sub(14);
        let high = self.candles[start..]
            .iter()
            .map(|c| c.high)
            .fold(f64::NEG_INFINITY, f64::max);
        let low = self.candles[start..]
            .iter()
            .map(|c| c.low)
            .fold(f64::INFINITY, f64::min);
        let (next, changed) = match self.pivot {
            None => ((c, false), true),
            Some((prior, is_high)) if c.high >= high && (!is_high || c.high >= prior.high) => {
                ((c, true), true)
            }
            Some((prior, is_high))
                if c.high < high && c.low <= low && (is_high || c.low <= prior.low) =>
            {
                ((c, false), true)
            }
            Some(p) => (p, false),
        };
        self.pivot = Some(next);
        Ok(changed.then(||json!({"pivot_at_ms":next.0.timestamp,"observed_at_ms":c.timestamp,"kind":if next.1 {"high"} else {"low"},"price":if next.1 {next.0.high} else {next.0.low},"provisional":true,"revision_event":true})))
    }
    fn kdj_step(&mut self) -> Result<Option<Value>, TaError> {
        if self.candles.len() < 9 {
            return Ok(None);
        }
        let bars = &self.candles[self.candles.len() - 9..];
        let high = bars
            .iter()
            .map(|c| c.high)
            .fold(f64::NEG_INFINITY, f64::max);
        let low = bars.iter().map(|c| c.low).fold(f64::INFINITY, f64::min);
        let rsv = 100.0 * ratio(bars[8].close - low, high - low);
        if !rsv.is_finite() {
            return Ok(Some(Value::Null));
        }
        let (pk, pd) = if self.auxiliary.len() == 2 {
            (self.auxiliary[0], self.auxiliary[1])
        } else {
            (50.0, 50.0)
        };
        let k = (2.0 * pk + rsv) / 3.0;
        let d = (2.0 * pd + k) / 3.0;
        self.auxiliary = vec![k, d];
        Ok(Some(json!({"k":k,"d":d,"j":3.0*k-2.0*d})))
    }
    fn dvi_step(&mut self) -> Result<Option<Value>, TaError> {
        if self.x.len() < 3 {
            return Ok(None);
        }
        let returns = self
            .x
            .windows(3)
            .map(|a| ratio(a[2], mean(a)) - 1.0)
            .collect::<Vec<_>>();
        let signs = self
            .x
            .windows(2)
            .map(|a| if a[1] > a[0] { 1.0 } else { -1.0 })
            .collect::<Vec<_>>();
        fn component(a: &[f64], short: usize, smooth: usize, sum: bool) -> Vec<f64> {
            if a.len() < 100 {
                return vec![];
            }
            let base = (100..=a.len())
                .map(|end| {
                    let long = &a[end - 100..end];
                    let sh = &a[end - short..end];
                    if sum {
                        (sh.iter().sum::<f64>() + long.iter().sum::<f64>() / 10.0) / 2.0
                    } else {
                        (mean(sh) + mean(long) / 10.0) / 2.0
                    }
                })
                .collect::<Vec<_>>();
            base.windows(smooth).map(mean).collect()
        }
        let m = component(&returns, 5, 5, false);
        let st = component(&signs, 10, 2, true);
        let (Some(m), Some(st)) = (window(&m, 252), window(&st, 252)) else {
            return Ok(None);
        };
        let mag = rank(m);
        let stretch = rank(st);
        Ok(Some(
            json!({"magnitude":mag,"stretch":stretch,"dvi":0.8*mag+0.2*stretch}),
        ))
    }
}
