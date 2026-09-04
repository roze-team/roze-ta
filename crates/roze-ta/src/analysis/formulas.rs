//! Explicit-parameter financial formula evaluation; no trading or implicit fitting.
use super::*;
use research::NamedValue;
use statrs::distribution::{Continuous, ContinuousCDF, Normal};

pub const VERSION: &str = "roze-ta-financial-formulas-v1";

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct FormulaSpec {
    /// All parameters and data in this task were available at this time.
    pub available_at_ms: i64,
    pub task: FormulaTask,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum Side {
    Buy,
    Sell,
}
impl Side {
    fn sign(self) -> f64 {
        match self {
            Self::Buy => 1.0,
            Self::Sell => -1.0,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct Fill {
    pub quantity: f64,
    pub price: f64,
    pub fee: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct Quote {
    pub bid: f64,
    pub bid_size: f64,
    pub ask: f64,
    pub ask_size: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum FormulaTask {
    Turnover {
        previous_weights: Vec<f64>,
        current_weights: Vec<f64>,
    },
    ContinuousKelly {
        mean: f64,
        risk_free: f64,
        variance: f64,
        fraction: f64,
    },
    TradeExpectancy {
        win_probability: f64,
        average_win: f64,
        average_loss: f64,
        cost: f64,
    },
    CashflowReturn {
        previous_price: f64,
        price: f64,
        cashflow: f64,
    },
    RollYield {
        near: f64,
        far: f64,
    },
    Kelly {
        win_probability: f64,
        payoff_ratio: f64,
        fraction: f64,
    },
    Futures {
        entry: f64,
        exit: f64,
        multiplier: f64,
        contracts: f64,
        side: Side,
        margin_rate: f64,
        equity: f64,
        costs: f64,
    },
    StopSizing {
        equity: f64,
        risk_fraction: f64,
        entry: f64,
        stop: f64,
        multiplier: f64,
        cost_per_contract: f64,
    },
    Carry {
        spot: f64,
        futures: f64,
        rate: f64,
        storage_rate: f64,
        convenience_yield: f64,
        years: f64,
    },
    Hedge {
        correlation: f64,
        spot_volatility: f64,
        futures_volatility: f64,
        exposure_value: f64,
        contract_value: f64,
    },
    ExecutionQuality {
        side: Side,
        requested_quantity: f64,
        decision_price: f64,
        arrival_price: f64,
        terminal_price: f64,
        multiplier: f64,
        fills: Vec<Fill>,
    },
    CostModel {
        gross_return: f64,
        turnover: f64,
        commission_bps: f64,
        half_spread_bps: f64,
        slippage_bps: f64,
        impact_bps: f64,
    },
    AlmgrenChriss {
        quantity: f64,
        horizon: f64,
        volatility: f64,
        risk_aversion: f64,
        temporary_impact: f64,
        steps: usize,
    },
    AvellanedaStoikov {
        mid: f64,
        inventory: f64,
        volatility: f64,
        remaining_time: f64,
        risk_aversion: f64,
        intensity_decay: f64,
    },
    OrderFlowImbalance {
        quotes: Vec<Quote>,
    },
    BlackScholes {
        spot: f64,
        strike: f64,
        years: f64,
        rate: f64,
        dividend_yield: f64,
        volatility: f64,
    },
    DiscreteProbability {
        probabilities: Vec<f64>,
        values: Vec<f64>,
        conditional_variances: Vec<f64>,
    },
    Bayes {
        prior: f64,
        likelihood_if_true: f64,
        likelihood_if_false: f64,
    },
    Losses {
        actual: Vec<f64>,
        predicted: Vec<f64>,
        huber_delta: f64,
    },
    Information {
        p: Vec<f64>,
        q: Vec<f64>,
    },
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct FormulaResult {
    pub method_version: String,
    pub values: Vec<NamedValue>,
    pub series: Vec<Vec<NamedValue>>,
    pub assumptions: Vec<String>,
}
fn parameter(s: &str) -> TaError {
    err(ErrorCode::InvalidParameter, s)
}
fn finite(v: f64) -> Result<(), TaError> {
    if v.is_finite() && v.abs() <= 1e30 {
        Ok(())
    } else {
        Err(parameter(
            "formula values must be finite with magnitude <=1e30",
        ))
    }
}
fn positive(v: f64) -> Result<(), TaError> {
    finite(v)?;
    if v > 0.0 {
        Ok(())
    } else {
        Err(parameter("strictly positive parameter required"))
    }
}
fn nonnegative(v: f64) -> Result<(), TaError> {
    finite(v)?;
    if v >= 0.0 {
        Ok(())
    } else {
        Err(parameter("nonnegative parameter required"))
    }
}
fn probability(v: f64) -> Result<(), TaError> {
    finite(v)?;
    if (0.0..=1.0).contains(&v) {
        Ok(())
    } else {
        Err(parameter("probability/fraction must be in [0,1]"))
    }
}
fn probability_vector(v: &[f64]) -> Result<(), TaError> {
    if v.is_empty() || v.len() > MAX_SAMPLES {
        return Err(err(
            ErrorCode::LimitExceeded,
            "probability vector length must be 1..4096",
        ));
    }
    for &p in v {
        probability(p)?;
    }
    if (v.iter().sum::<f64>() - 1.0).abs() > 1e-12 {
        return Err(parameter("probabilities must sum to one"));
    }
    Ok(())
}
fn nv(name: &str, v: f64) -> NamedValue {
    NamedValue {
        name: name.into(),
        value: Scalar::number(v),
    }
}
fn undef(name: &str, reason: &str) -> NamedValue {
    NamedValue {
        name: name.into(),
        value: Scalar::undefined(reason),
    }
}

impl FormulaSpec {
    pub(super) fn validate(&self, request: &Request) -> Result<(usize, usize), TaError> {
        if !request.points.is_empty()
            || !request.events.is_empty()
            || request.input_kind != "explicit_formula_parameters"
        {
            return Err(parameter(
                "formula tasks require explicit_formula_parameters and empty points/events",
            ));
        }
        if self.available_at_ms <= 0 || self.available_at_ms > request.fit_cutoff_ms {
            return Err(err(
                ErrorCode::InvalidTime,
                "all formula inputs must be available by fit cutoff",
            ));
        }
        let mut count = 32;
        match &self.task {
            FormulaTask::Turnover {
                previous_weights,
                current_weights,
            } => {
                if previous_weights.is_empty()
                    || previous_weights.len() > 16
                    || previous_weights.len() != current_weights.len()
                {
                    return Err(parameter("turnover requires aligned 1..16 asset weights"));
                }
                for v in previous_weights.iter().chain(current_weights) {
                    finite(*v)?;
                }
            }
            FormulaTask::ContinuousKelly {
                mean,
                risk_free,
                variance,
                fraction,
            } => {
                finite(*mean)?;
                finite(*risk_free)?;
                positive(*variance)?;
                probability(*fraction)?;
            }
            FormulaTask::TradeExpectancy {
                win_probability,
                average_win,
                average_loss,
                cost,
            } => {
                probability(*win_probability)?;
                positive(*average_win)?;
                positive(*average_loss)?;
                nonnegative(*cost)?;
            }
            FormulaTask::CashflowReturn {
                previous_price,
                price,
                cashflow,
            } => {
                positive(*previous_price)?;
                positive(*price)?;
                finite(*cashflow)?;
            }
            FormulaTask::RollYield { near, far } => {
                positive(*near)?;
                positive(*far)?;
            }
            FormulaTask::Kelly {
                win_probability,
                payoff_ratio,
                fraction,
            } => {
                probability(*win_probability)?;
                positive(*payoff_ratio)?;
                probability(*fraction)?;
            }
            FormulaTask::Futures {
                entry,
                exit,
                multiplier,
                contracts,
                margin_rate,
                equity,
                costs,
                ..
            } => {
                finite(*entry)?;
                finite(*exit)?;
                positive(*multiplier)?;
                nonnegative(*contracts)?;
                probability(*margin_rate)?;
                positive(*equity)?;
                nonnegative(*costs)?;
            }
            FormulaTask::StopSizing {
                equity,
                risk_fraction,
                entry,
                stop,
                multiplier,
                cost_per_contract,
            } => {
                positive(*equity)?;
                probability(*risk_fraction)?;
                finite(*entry)?;
                finite(*stop)?;
                positive(*multiplier)?;
                nonnegative(*cost_per_contract)?;
                if entry == stop {
                    return Err(parameter("stop distance must be nonzero"));
                }
            }
            FormulaTask::Carry {
                spot,
                futures,
                rate,
                storage_rate,
                convenience_yield,
                years,
            } => {
                positive(*spot)?;
                positive(*futures)?;
                finite(*rate)?;
                finite(*storage_rate)?;
                finite(*convenience_yield)?;
                positive(*years)?;
            }
            FormulaTask::Hedge {
                correlation,
                spot_volatility,
                futures_volatility,
                exposure_value,
                contract_value,
            } => {
                finite(*correlation)?;
                if correlation.abs() > 1.0 {
                    return Err(parameter("correlation must be in [-1,1]"));
                }
                nonnegative(*spot_volatility)?;
                positive(*futures_volatility)?;
                finite(*exposure_value)?;
                positive(*contract_value)?;
            }
            FormulaTask::ExecutionQuality {
                requested_quantity,
                decision_price,
                arrival_price,
                terminal_price,
                multiplier,
                fills,
                ..
            } => {
                positive(*requested_quantity)?;
                positive(*decision_price)?;
                positive(*arrival_price)?;
                positive(*terminal_price)?;
                positive(*multiplier)?;
                if fills.len() > MAX_SAMPLES {
                    return Err(err(ErrorCode::LimitExceeded, "at most 4096 fills"));
                }
                let mut total = 0.0;
                for f in fills {
                    positive(f.quantity)?;
                    positive(f.price)?;
                    nonnegative(f.fee)?;
                    total += f.quantity;
                }
                if total > *requested_quantity {
                    return Err(parameter("filled quantity exceeds requested quantity"));
                }
                count += fills.len();
            }
            FormulaTask::CostModel {
                gross_return,
                turnover,
                commission_bps,
                half_spread_bps,
                slippage_bps,
                impact_bps,
            } => {
                finite(*gross_return)?;
                for v in [
                    turnover,
                    commission_bps,
                    half_spread_bps,
                    slippage_bps,
                    impact_bps,
                ] {
                    nonnegative(*v)?;
                }
            }
            FormulaTask::AlmgrenChriss {
                quantity,
                horizon,
                volatility,
                risk_aversion,
                temporary_impact,
                steps,
            } => {
                nonnegative(*quantity)?;
                positive(*horizon)?;
                nonnegative(*volatility)?;
                nonnegative(*risk_aversion)?;
                positive(*temporary_impact)?;
                if *steps == 0 || *steps > MAX_SAMPLES {
                    return Err(err(
                        ErrorCode::LimitExceeded,
                        "execution grid requires 1..4096 steps",
                    ));
                }
                count += steps * 3;
            }
            FormulaTask::AvellanedaStoikov {
                mid,
                inventory,
                volatility,
                remaining_time,
                risk_aversion,
                intensity_decay,
            } => {
                positive(*mid)?;
                finite(*inventory)?;
                nonnegative(*volatility)?;
                nonnegative(*remaining_time)?;
                nonnegative(*risk_aversion)?;
                positive(*intensity_decay)?;
            }
            FormulaTask::OrderFlowImbalance { quotes } => {
                if quotes.len() > MAX_SAMPLES {
                    return Err(err(ErrorCode::LimitExceeded, "at most 4096 quotes"));
                }
                for q in quotes {
                    finite(q.bid)?;
                    finite(q.ask)?;
                    nonnegative(q.bid_size)?;
                    nonnegative(q.ask_size)?;
                    if q.bid > q.ask {
                        return Err(parameter("crossed book"));
                    }
                }
                count += quotes.len();
            }
            FormulaTask::BlackScholes {
                spot,
                strike,
                years,
                rate,
                dividend_yield,
                volatility,
            } => {
                positive(*spot)?;
                positive(*strike)?;
                positive(*years)?;
                finite(*rate)?;
                finite(*dividend_yield)?;
                positive(*volatility)?;
            }
            FormulaTask::DiscreteProbability {
                probabilities,
                values,
                conditional_variances,
            } => {
                probability_vector(probabilities)?;
                if values.len() != probabilities.len()
                    || conditional_variances.len() != values.len()
                {
                    return Err(parameter("mixture arrays must have equal length"));
                }
                for v in values {
                    finite(*v)?;
                }
                for v in conditional_variances {
                    nonnegative(*v)?;
                }
                count += values.len();
            }
            FormulaTask::Bayes {
                prior,
                likelihood_if_true,
                likelihood_if_false,
            } => {
                probability(*prior)?;
                probability(*likelihood_if_true)?;
                probability(*likelihood_if_false)?;
            }
            FormulaTask::Losses {
                actual,
                predicted,
                huber_delta,
            } => {
                positive(*huber_delta)?;
                if actual.is_empty()
                    || actual.len() > MAX_SAMPLES
                    || actual.len() != predicted.len()
                {
                    return Err(parameter("loss arrays must have equal length in 1..4096"));
                }
                for v in actual.iter().chain(predicted) {
                    finite(*v)?;
                }
                count += actual.len();
            }
            FormulaTask::Information { p, q } => {
                probability_vector(p)?;
                probability_vector(q)?;
                if p.len() != q.len() {
                    return Err(parameter("information arrays must have equal length"));
                }
                count += p.len();
            }
        }
        Ok((count * 16, count))
    }
}

pub(super) fn calculate(
    spec: &FormulaSpec,
    checkpoint: &mut impl FnMut() -> Result<(), TaError>,
) -> Result<FormulaResult, TaError> {
    let mut r=FormulaResult{method_version:VERSION.into(),values:vec![],series:vec![],assumptions:vec!["explicit frozen inputs; scalar formulas do not estimate parameters or authorize execution; common currency and consistent time units are caller responsibilities".into()]};
    match &spec.task {
        FormulaTask::Turnover {
            previous_weights,
            current_weights,
        } => {
            let gross = previous_weights
                .iter()
                .zip(current_weights)
                .map(|(a, b)| (b - a).abs())
                .sum::<f64>();
            r.values = vec![
                nv("gross_turnover", gross),
                nv("one_way_turnover", gross / 2.0),
            ];
            r.assumptions.push("aligned portfolio weights; one-way convention is half the absolute weight changes; no automatic drift adjustment".into());
        }
        FormulaTask::ContinuousKelly {
            mean,
            risk_free,
            variance,
            fraction,
        } => {
            r.values = vec![
                nv("full_kelly_approximation", (mean - risk_free) / variance),
                nv(
                    "fractional_kelly_approximation",
                    fraction * (mean - risk_free) / variance,
                ),
            ];
        }
        FormulaTask::TradeExpectancy {
            win_probability: p,
            average_win: w,
            average_loss: l,
            cost: c,
        } => {
            r.values = vec![
                nv("net_expectancy", p * w - (1.0 - p) * l - c),
                nv("break_even_probability", (l + c) / (w + l)),
                nv("payoff_ratio", w / l),
            ];
            r.assumptions.push(
                "break-even threshold can exceed one when costs make profitability impossible"
                    .into(),
            );
        }
        FormulaTask::CashflowReturn {
            previous_price,
            price,
            cashflow,
        } => {
            r.values.push(nv(
                "simple_total_return",
                (price + cashflow) / previous_price - 1.0,
            ));
        }
        FormulaTask::RollYield { near, far } => {
            r.values.push(nv("long_roll_yield", (near - far) / near));
        }
        FormulaTask::Kelly {
            win_probability: p,
            payoff_ratio: b,
            fraction: f,
        } => {
            let k = p - (1.0 - p) / b;
            r.values = vec![
                nv("full_kelly", k),
                nv("fractional_kelly", f * k),
                nv("break_even_probability", 1.0 / (1.0 + b)),
            ];
            r.assumptions.push("binary independent bets, payoff is net win/net loss; negative fractions are retained, no leverage clipping".into());
        }
        FormulaTask::Futures {
            entry,
            exit,
            multiplier,
            contracts,
            side,
            margin_rate,
            equity,
            costs,
        } => {
            let notional = entry.abs() * multiplier * contracts;
            r.values = vec![
                nv("entry_notional", notional),
                nv(
                    "pnl",
                    side.sign() * (exit - entry) * multiplier * contracts - costs,
                ),
                nv("initial_margin", notional * margin_rate),
                nv("gross_leverage", notional / equity),
            ];
            r.assumptions.push("linear futures; negative settlement prices allowed; absolute notional margin approximation, not exchange margin rules".into());
        }
        FormulaTask::StopSizing {
            equity,
            risk_fraction,
            entry,
            stop,
            multiplier,
            cost_per_contract,
        } => {
            let risk = (entry - stop).abs() * multiplier + cost_per_contract;
            r.values = vec![
                nv("risk_per_contract", risk),
                nv("risk_budget", equity * risk_fraction),
                nv("contracts", (equity * risk_fraction / risk).floor()),
            ];
        }
        FormulaTask::Carry {
            spot,
            futures,
            rate,
            storage_rate,
            convenience_yield,
            years,
        } => {
            r.values = vec![
                nv("basis_futures_minus_spot", futures - spot),
                nv("relative_basis", (futures - spot) / spot),
                nv("annualized_simple_basis", (futures / spot - 1.0) / years),
                nv(
                    "fair_futures",
                    spot * ((rate + storage_rate - convenience_yield) * years).exp(),
                ),
                nv("annualized_log_carry", (futures / spot).ln() / years),
            ];
        }
        FormulaTask::Hedge {
            correlation,
            spot_volatility,
            futures_volatility,
            exposure_value,
            contract_value,
        } => {
            let h = correlation * spot_volatility / futures_volatility;
            r.values = vec![
                nv("minimum_variance_hedge_ratio", h),
                nv("hedge_contracts", h * exposure_value / contract_value),
            ];
        }
        FormulaTask::ExecutionQuality {
            side,
            requested_quantity,
            decision_price,
            arrival_price,
            terminal_price,
            multiplier,
            fills,
        } => {
            let qty: f64 = fills.iter().map(|f| f.quantity).sum();
            let value: f64 = fills.iter().map(|f| f.quantity * f.price).sum();
            let fee: f64 = fills.iter().map(|f| f.fee).sum();
            let delay = side.sign() * qty * (arrival_price - decision_price) * multiplier;
            let trading = side.sign() * (value - qty * arrival_price) * multiplier;
            let opportunity = side.sign()
                * (requested_quantity - qty)
                * (terminal_price - decision_price)
                * multiplier;
            r.values = vec![
                nv("fill_ratio", qty / requested_quantity),
                nv("delay_cost", delay),
                nv("trading_cost", trading),
                nv("opportunity_cost", opportunity),
                nv("fees", fee),
                nv(
                    "implementation_shortfall",
                    delay + trading + opportunity + fee,
                ),
            ];
            r.values.push(if qty > 0.0 {
                nv("average_fill_price", value / qty)
            } else {
                undef("average_fill_price", "no_fills")
            });
        }
        FormulaTask::CostModel {
            gross_return,
            turnover,
            commission_bps,
            half_spread_bps,
            slippage_bps,
            impact_bps,
        } => {
            let cost = turnover * (commission_bps + half_spread_bps + slippage_bps + impact_bps)
                / 10_000.0;
            r.values = vec![
                nv("cost_return", cost),
                nv("net_return", gross_return - cost),
            ];
            r.assumptions.push("turnover=sum absolute traded notional/equity; each basis-point cost applies once; impact is explicitly supplied rather than fitted".into());
        }
        FormulaTask::AlmgrenChriss {
            quantity,
            horizon,
            volatility,
            risk_aversion,
            temporary_impact,
            steps,
        } => {
            let k = (risk_aversion / temporary_impact).sqrt() * volatility;
            if !k.is_finite() || !(k * horizon).is_finite() {
                return Err(err(
                    ErrorCode::NumericalFailure,
                    "execution curvature overflow",
                ));
            }
            r.values.push(nv("kappa", k));
            for i in 0..=*steps {
                checkpoint()?;
                let t = horizon * i as f64 / *steps as f64;
                let (inventory, speed) = if k * horizon < 1e-7 {
                    (quantity * (1.0 - t / horizon), quantity / horizon)
                } else {
                    // Scaled hyperbolic functions avoid overflow at large k*T.
                    let denominator = -(-2.0 * k * horizon).exp_m1();
                    let decay = (-k * t).exp();
                    (
                        quantity * decay * (-(-2.0 * k * (horizon - t)).exp_m1()) / denominator,
                        quantity * k * decay * (1.0 + (-2.0 * k * (horizon - t)).exp())
                            / denominator,
                    )
                };
                r.series.push(vec![
                    nv("time", t),
                    nv("remaining_inventory", inventory),
                    nv("trading_speed", speed),
                ]);
            }
            r.assumptions.push("continuous-time linear temporary impact, arithmetic price volatility; sampled analytical liquidation curve; zero risk aversion uses TWAP limit".into());
        }
        FormulaTask::AvellanedaStoikov {
            mid,
            inventory,
            volatility,
            remaining_time,
            risk_aversion: g,
            intensity_decay: k,
        } => {
            let risk = g * volatility * volatility * remaining_time;
            let reserve = mid - inventory * risk;
            let liquidity = if *g == 0.0 {
                2.0 / k
            } else {
                2.0 * (g / k).ln_1p() / g
            };
            let spread = risk + liquidity;
            r.values = vec![
                nv("reservation_price", reserve),
                nv("total_spread", spread),
                nv("bid", reserve - spread / 2.0),
                nv("ask", reserve + spread / 2.0),
            ];
            r.assumptions.push("approximate unconstrained quote model; no tick rounding, positive-price guarantee or fill guarantee".into());
        }
        FormulaTask::OrderFlowImbalance { quotes } => {
            let mut total = 0.0;
            for pair in quotes.windows(2) {
                checkpoint()?;
                let a = &pair[0];
                let b = &pair[1];
                let mut e = 0.0;
                if b.bid >= a.bid {
                    e += b.bid_size;
                }
                if b.bid <= a.bid {
                    e -= a.bid_size;
                }
                if b.ask <= a.ask {
                    e -= b.ask_size;
                }
                if b.ask >= a.ask {
                    e += a.ask_size;
                }
                total += e;
                r.series.push(vec![nv("event_ofi", e)]);
            }
            r.values.push(if quotes.len() < 2 {
                NamedValue {
                    name: "ofi".into(),
                    value: Scalar::InsufficientData,
                }
            } else {
                nv("ofi", total)
            });
            r.assumptions.push("quotes supplied in event order; first quote is the baseline; tied prices use both size-change terms".into());
        }
        FormulaTask::BlackScholes {
            spot: s,
            strike: k,
            years: t,
            rate,
            dividend_yield: q,
            volatility: v,
        } => {
            let norm =
                Normal::new(0.0, 1.0).map_err(|_| parameter("normal construction failed"))?;
            let root = t.sqrt();
            let d1 = ((s / k).ln() + (rate - q + v * v / 2.0) * t) / (v * root);
            let d2 = d1 - v * root;
            let ds = (-q * t).exp();
            let dk = (-rate * t).exp();
            let common = -s * ds * norm.pdf(d1) * v / (2.0 * root);
            r.values = vec![
                nv("call", s * ds * norm.cdf(d1) - k * dk * norm.cdf(d2)),
                nv("put", k * dk * norm.cdf(-d2) - s * ds * norm.cdf(-d1)),
                nv("call_delta", ds * norm.cdf(d1)),
                nv("put_delta", -ds * norm.cdf(-d1)),
                nv("gamma", ds * norm.pdf(d1) / (s * v * root)),
                nv("vega", s * ds * norm.pdf(d1) * root),
                nv(
                    "call_theta",
                    common - rate * k * dk * norm.cdf(d2) + q * s * ds * norm.cdf(d1),
                ),
                nv(
                    "put_theta",
                    common + rate * k * dk * norm.cdf(-d2) - q * s * ds * norm.cdf(-d1),
                ),
                nv("call_rho", k * t * dk * norm.cdf(d2)),
                nv("put_rho", -k * t * dk * norm.cdf(-d2)),
            ];
            r.assumptions.push("European options with continuous dividend yield; time in years; vega/rho per unit (1.0) change, theta per year".into());
        }
        FormulaTask::DiscreteProbability {
            probabilities: p,
            values: x,
            conditional_variances: vars,
        } => {
            let m: f64 = p.iter().zip(x).map(|(p, x)| p * x).sum();
            let between: f64 = p.iter().zip(x).map(|(p, x)| p * (x - m).powi(2)).sum();
            let within: f64 = p.iter().zip(vars).map(|(p, v)| p * v).sum();
            r.values = vec![
                nv("expectation", m),
                nv("variance_of_conditional_means", between),
                nv("expected_conditional_variance", within),
                nv("total_variance", between + within),
            ];
        }
        FormulaTask::Bayes {
            prior: p,
            likelihood_if_true: a,
            likelihood_if_false: b,
        } => {
            let evidence = p * a + (1.0 - p) * b;
            r.values.push(nv("evidence_probability", evidence));
            r.values.push(if evidence > 0.0 {
                nv("posterior_probability", p * a / evidence)
            } else {
                undef("posterior_probability", "zero_probability_evidence")
            });
        }
        FormulaTask::Losses {
            actual,
            predicted,
            huber_delta: d,
        } => {
            let (mut square, mut absolute, mut huber) = (0.0, 0.0, 0.0);
            for (a, p) in actual.iter().zip(predicted) {
                let e = (a - p).abs();
                square += e * e;
                absolute += e;
                huber += if e <= *d {
                    e * e / 2.0
                } else {
                    d * (e - d / 2.0)
                };
            }
            let n = actual.len() as f64;
            r.values = vec![
                nv("mse", square / n),
                nv("mae", absolute / n),
                nv("huber", huber / n),
            ];
        }
        FormulaTask::Information { p, q } => {
            let entropy = -p
                .iter()
                .filter(|&&v| v > 0.0)
                .map(|v| v * v.ln())
                .sum::<f64>();
            r.values.push(nv("entropy_nats", entropy));
            if p.iter().zip(q).any(|(&p, &q)| p > 0.0 && q == 0.0) {
                r.values.push(undef(
                    "kl_divergence_nats",
                    "positive_mass_with_zero_reference_probability",
                ));
            } else {
                r.values.push(nv(
                    "kl_divergence_nats",
                    p.iter()
                        .zip(q)
                        .filter(|(p, _)| **p > 0.0)
                        .map(|(p, q)| p * (p / q).ln())
                        .sum(),
                ));
            }
        }
    }
    checkpoint()?;
    Ok(r)
}
