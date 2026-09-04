//! Bounded Heston calibration sharing the core European pricing implementation.
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum HestonParameter {
    InitialVariance,
    Reversion,
    LongRunVariance,
    VolOfVariance,
    Correlation,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum OptionKind {
    Call,
    Put,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct HestonModel {
    pub initial_variance: f64,
    pub reversion: f64,
    pub long_run_variance: f64,
    pub vol_of_variance: f64,
    pub correlation: f64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct HestonQuote {
    pub spot: f64,
    pub strike: f64,
    pub years: f64,
    pub rate: f64,
    pub dividend_yield: f64,
    pub kind: OptionKind,
    pub price: f64,
    pub weight: f64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct HestonCalibration {
    pub model: HestonModel,
    pub active_parameters: Vec<HestonParameter>,
    pub initial_weighted_mse: f64,
    pub weighted_mse: f64,
    pub weighted_rmse: f64,
    pub fitted_prices: Vec<f64>,
    pub residuals: Vec<f64>,
    pub integration_differences: Vec<f64>,
    pub iterations: usize,
    pub evaluations: usize,
    pub converged: bool,
    pub normalized_step: f64,
    pub bound_hits: Vec<HestonParameter>,
}
impl HestonModel {
    fn coordinate(&self, p: HestonParameter) -> f64 {
        match p {
            HestonParameter::InitialVariance => self.initial_variance,
            HestonParameter::Reversion => self.reversion,
            HestonParameter::LongRunVariance => self.long_run_variance,
            HestonParameter::VolOfVariance => self.vol_of_variance,
            HestonParameter::Correlation => self.correlation,
        }
    }
    fn set(&mut self, p: HestonParameter, v: f64) {
        match p {
            HestonParameter::InitialVariance => self.initial_variance = v,
            HestonParameter::Reversion => self.reversion = v,
            HestonParameter::LongRunVariance => self.long_run_variance = v,
            HestonParameter::VolOfVariance => self.vol_of_variance = v,
            HestonParameter::Correlation => self.correlation = v,
        }
    }
    fn for_quote(&self, q: &HestonQuote) -> HestonParameters {
        HestonParameters {
            spot: q.spot,
            strike: q.strike,
            years: q.years,
            rate: q.rate,
            dividend_yield: q.dividend_yield,
            initial_variance: self.initial_variance,
            reversion: self.reversion,
            long_run_variance: self.long_run_variance,
            vol_of_variance: self.vol_of_variance,
            correlation: self.correlation,
        }
    }
}
pub(super) fn validate(spec: &StochasticSpec, r: &Request) -> Result<(usize, usize), TaError> {
    let StochasticTask::HestonCalibrate {
        quotes,
        initial,
        parameters,
        lower,
        upper,
        max_iterations,
        fit_tolerance,
        integration_limit,
        intervals,
        pricing_tolerance,
    } = &spec.task
    else {
        return Err(bad("expected Heston calibration"));
    };
    let k = parameters.len();
    if !(1..=5).contains(&k)
        || quotes.len() < k
        || quotes.len() > 16
        || lower.len() != k
        || upper.len() != k
        || !(1..=1000).contains(max_iterations)
        || !fit_tolerance.is_finite()
        || !(1e-8..=0.01).contains(fit_tolerance)
    {
        return Err(bad("Heston fit requires 1..5 unique active parameters, k..16 quotes, matching bounds, iterations 1..1000 and tolerance 1e-8..0.01"));
    }
    for (i, p) in parameters.iter().enumerate() {
        let (lo, hi) = (lower[i], upper[i]);
        if parameters[..i].contains(p)
            || !finite(lo)
            || !finite(hi)
            || lo >= hi
            || initial.coordinate(*p) < lo
            || initial.coordinate(*p) > hi
        {
            return Err(bad("unique active parameters and finite ordered bounds containing the initial model required"));
        }
        let valid = match p {
            HestonParameter::Correlation => lo >= -1.0 && hi <= 1.0,
            HestonParameter::Reversion => lo > 0.0,
            _ => lo >= 0.0,
        };
        if !valid {
            return Err(bad("Heston bounds violate model parameter domain"));
        }
    }
    let mut work = 0usize;
    for q in quotes {
        if !finite(q.price) || q.price < 0.0 || !finite(q.weight) || q.weight <= 0.0 {
            return Err(bad(
                "positive finite quote weights and nonnegative prices required",
            ));
        }
        let (w, _) = StochasticSpec {
            available_at_ms: spec.available_at_ms,
            task: StochasticTask::Heston {
                parameters: initial.for_quote(q),
                integration_limit: *integration_limit,
                intervals: *intervals,
                tolerance: *pricing_tolerance,
            },
        }
        .validate(r)?;
        let s = q.spot * (-q.dividend_yield * q.years).exp();
        let b = q.strike * (-q.rate * q.years).exp();
        let (minimum, maximum) = match q.kind {
            OptionKind::Call => ((s - b).max(0.0), s),
            OptionKind::Put => ((b - s).max(0.0), b),
        };
        if !s.is_finite() || !b.is_finite() || q.price < minimum - 1e-8 || q.price > maximum + 1e-8
        {
            return Err(bad("quote violates finite European no-arbitrage bounds"));
        }
        work += w;
    }
    Ok((
        work.saturating_mul(2 * k * max_iterations + 2),
        128 + quotes.len() * 3,
    ))
}
struct Evaluation {
    objective: f64,
    prices: Vec<f64>,
    differences: Vec<f64>,
}
fn scalar(out: &StochasticResult, name: &str) -> Result<f64, TaError> {
    out.values
        .iter()
        .find_map(|v| match v.value {
            Scalar::Ready { value } if v.name == name && value.is_finite() => Some(value),
            _ => None,
        })
        .ok_or_else(|| err(ErrorCode::NumericalFailure, "nonfinite Heston price"))
}
#[allow(clippy::too_many_arguments)]
fn evaluate(
    model: &HestonModel,
    quotes: &[HestonQuote],
    available: i64,
    limit: f64,
    intervals: usize,
    tolerance: f64,
    checkpoint: &mut impl FnMut() -> Result<(), TaError>,
) -> Result<Evaluation, TaError> {
    let mut prices = vec![];
    let mut differences = vec![];
    let mut objective = 0.0;
    let mut weight = 0.0;
    for q in quotes {
        checkpoint()?;
        let out = calculate(
            &StochasticSpec {
                available_at_ms: available,
                task: StochasticTask::Heston {
                    parameters: model.for_quote(q),
                    integration_limit: limit,
                    intervals,
                    tolerance,
                },
            },
            checkpoint,
        )?;
        if !out.converged {
            return Err(err(
                ErrorCode::NumericalFailure,
                "Heston calibration pricing did not converge",
            ));
        }
        let price = scalar(
            &out,
            match q.kind {
                OptionKind::Call => "call",
                OptionKind::Put => "put",
            },
        )?;
        objective += q.weight * (price - q.price).powi(2);
        weight += q.weight;
        prices.push(price);
        differences.push(scalar(&out, "integration_difference")?);
    }
    objective /= weight;
    if !objective.is_finite() {
        return Err(err(
            ErrorCode::NumericalFailure,
            "nonfinite calibration loss",
        ));
    }
    Ok(Evaluation {
        objective,
        prices,
        differences,
    })
}
pub(super) fn fit(
    spec: &StochasticSpec,
    checkpoint: &mut impl FnMut() -> Result<(), TaError>,
) -> Result<HestonCalibration, TaError> {
    let StochasticTask::HestonCalibrate {
        quotes,
        initial,
        parameters,
        lower,
        upper,
        max_iterations,
        fit_tolerance,
        integration_limit,
        intervals,
        pricing_tolerance,
    } = &spec.task
    else {
        return Err(bad("expected Heston calibration"));
    };
    let k = parameters.len();
    let mut best: Vec<_> = parameters
        .iter()
        .enumerate()
        .map(|(i, p)| (initial.coordinate(*p) - lower[i]) / (upper[i] - lower[i]))
        .collect();
    let decode = |coordinates: &[f64]| {
        let mut model = initial.clone();
        for (i, p) in parameters.iter().enumerate() {
            model.set(*p, lower[i] + coordinates[i] * (upper[i] - lower[i]));
        }
        model
    };
    let price = |model: &HestonModel, checkpoint: &mut _| {
        evaluate(
            model,
            quotes,
            spec.available_at_ms,
            *integration_limit,
            *intervals,
            *pricing_tolerance,
            checkpoint,
        )
    };
    let mut result = price(&decode(&best), checkpoint)?;
    let initial_weighted_mse = result.objective;
    let mut evaluations = 1;
    let mut step = 0.25;
    let mut iterations = 0;
    for iteration in 0..*max_iterations {
        iterations = iteration + 1;
        let mut candidate = best.clone();
        let mut improved = None;
        let mut score = result.objective;
        for i in 0..k {
            for direction in [-1.0, 1.0] {
                checkpoint()?;
                let mut trial = best.clone();
                trial[i] = (trial[i] + direction * step).clamp(0.0, 1.0);
                evaluations += 1;
                match price(&decode(&trial), checkpoint) {
                    Ok(value) if value.objective < score => {
                        score = value.objective;
                        candidate = trial;
                        improved = Some(value);
                    }
                    Ok(_) => {}
                    Err(e) if e.code == ErrorCode::NumericalFailure => {}
                    Err(e) => return Err(e),
                }
            }
        }
        if let Some(value) = improved {
            best = candidate;
            result = value;
        } else {
            step *= 0.5;
        }
        if step <= *fit_tolerance {
            break;
        }
    }
    checkpoint()?;
    Ok(HestonCalibration {
        model: decode(&best),
        active_parameters: parameters.clone(),
        initial_weighted_mse,
        weighted_mse: result.objective,
        weighted_rmse: result.objective.sqrt(),
        residuals: result
            .prices
            .iter()
            .zip(quotes)
            .map(|(p, q)| p - q.price)
            .collect(),
        fitted_prices: result.prices,
        integration_differences: result.differences,
        iterations,
        evaluations,
        converged: step <= *fit_tolerance,
        normalized_step: step,
        bound_hits: parameters
            .iter()
            .zip(best)
            .filter_map(|(p, v)| if v == 0.0 || v == 1.0 { Some(*p) } else { None })
            .collect(),
    })
}
