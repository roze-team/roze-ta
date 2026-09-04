//! Fixed-exposure linear portfolio risk. No account access or execution authority.
use super::*;
use std::collections::BTreeSet;

pub const VERSION: &str = "roze-ta-portfolio-risk-v1";
pub const MAX_ASSETS: usize = 16;
pub const MAX_SCENARIOS: usize = 64;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct Asset {
    pub id: String,
    /// Signed linear exposure / positive portfolio equity, in a common currency.
    pub weight: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct Observation {
    pub at_ms: i64,
    pub available_at_ms: i64,
    /// Simple total returns in asset order; common currency and period.
    pub returns: Vec<f64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct Scenario {
    pub id: String,
    pub available_at_ms: i64,
    /// Full vector of price shocks in asset order. No inferred missing shocks.
    pub shocks: Vec<f64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct PortfolioSpec {
    pub assets: Vec<Asset>,
    pub equity: f64,
    pub currency: String,
    pub weights_available_at_ms: i64,
    pub periods_per_year: f64,
    pub ddof: u8,
    pub observations: Vec<Observation>,
    pub scenarios: Vec<Scenario>,
}

impl PortfolioSpec {
    pub(super) fn validate(&self, request: &Request) -> Result<(), TaError> {
        validate_ddof(self.ddof)?;
        text_field(&self.currency)?;
        if self.assets.is_empty()
            || self.assets.len() > MAX_ASSETS
            || self.observations.len() > MAX_SAMPLES
            || self.scenarios.len() > MAX_SCENARIOS
        {
            return Err(err(
                ErrorCode::LimitExceeded,
                "portfolio limits: 1..16 assets, 4096 observations, 64 scenarios",
            ));
        }
        if request.input_kind != "portfolio_simple_return"
            || request.units != "ratio"
            || !request.points.is_empty()
            || !request.events.is_empty()
        {
            return Err(err(ErrorCode::InvalidParameter, "portfolio requires input_kind=portfolio_simple_return, units=ratio and empty points/events; data belongs in spec"));
        }
        if !self.equity.is_finite()
            || !(0.0..=1e30).contains(&self.equity)
            || self.equity == 0.0
            || !self.periods_per_year.is_finite()
            || !(1.0..=1_000_000.0).contains(&self.periods_per_year)
        {
            return Err(err(
                ErrorCode::InvalidParameter,
                "equity must be positive <=1e30; periods_per_year must be 1..1000000",
            ));
        }
        known(self.weights_available_at_ms, request.fit_cutoff_ms)?;
        let mut ids = BTreeSet::new();
        for asset in &self.assets {
            text_field(&asset.id)?;
            if !ids.insert(&asset.id) || !asset.weight.is_finite() || asset.weight.abs() > 100.0 {
                return Err(err(
                    ErrorCode::InvalidParameter,
                    "unique asset IDs and finite weights with magnitude <=100 required",
                ));
            }
        }
        let mut previous = 0;
        let mut unavailable_seen = false;
        for row in &self.observations {
            if row.at_ms <= previous || row.available_at_ms < row.at_ms {
                return Err(err(ErrorCode::InvalidTime, "portfolio rows require increasing positive times and availability >= observation"));
            }
            previous = row.at_ms;
            vector(&row.returns, self.assets.len())?;
            if row.available_at_ms > request.fit_cutoff_ms {
                unavailable_seen = true;
            } else if unavailable_seen {
                return Err(err(
                    ErrorCode::InvalidTime,
                    "portfolio selection must be a contiguous available prefix",
                ));
            }
        }
        ids.clear();
        for scenario in &self.scenarios {
            text_field(&scenario.id)?;
            if !ids.insert(&scenario.id) {
                return Err(err(
                    ErrorCode::InvalidParameter,
                    "scenario IDs must be unique",
                ));
            }
            known(scenario.available_at_ms, request.fit_cutoff_ms)?;
            vector(&scenario.shocks, self.assets.len())?;
        }
        Ok(())
    }
}

fn known(at: i64, cutoff: i64) -> Result<(), TaError> {
    if at <= 0 || at > cutoff {
        Err(err(
            ErrorCode::InvalidTime,
            "weights/scenarios must be available by fit_cutoff",
        ))
    } else {
        Ok(())
    }
}

fn vector(values: &[f64], assets: usize) -> Result<(), TaError> {
    if values.len() != assets
        || values
            .iter()
            .any(|v| !v.is_finite() || !(-1.0..=100.0).contains(v))
    {
        Err(err(
            ErrorCode::InvalidSample,
            "each return/shock vector must match assets and contain finite ratios in [-1,100]",
        ))
    } else {
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct AssetRisk {
    pub id: String,
    pub signed_exposure: f64,
    pub gross_weight_share: Scalar,
    pub annualized_volatility: Scalar,
    /// Euler component; may be negative for a hedge.
    pub volatility_contribution: Scalar,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct ScenarioResult {
    pub id: String,
    pub pnl_by_asset: Vec<f64>,
    pub pnl: f64,
    pub loss: f64,
    pub return_on_equity: f64,
    pub equity_after: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct PortfolioResult {
    pub method_version: String,
    pub currency: String,
    pub selected_observations: usize,
    pub selected_range_ms: Option<[i64; 2]>,
    pub selection_hash: String,
    pub gross_exposure_ratio: f64,
    pub net_exposure_ratio: f64,
    pub concentration_hhi: Scalar,
    pub effective_asset_count: Scalar,
    pub covariance_per_period: Vec<Vec<Scalar>>,
    pub correlation: Vec<Vec<Scalar>>,
    pub annualized_volatility: Scalar,
    pub diversification_ratio: Scalar,
    pub assets: Vec<AssetRisk>,
    pub scenarios: Vec<ScenarioResult>,
    pub assumptions: Vec<String>,
}

pub(super) fn calculate(
    spec: &PortfolioSpec,
    cutoff: i64,
    checkpoint: &mut impl FnMut() -> Result<(), TaError>,
) -> Result<PortfolioResult, TaError> {
    let rows: Vec<_> = spec
        .observations
        .iter()
        .take_while(|r| r.available_at_ms <= cutoff)
        .collect();
    let n = rows.len();
    let m = spec.assets.len();
    let enough = n >= 2;
    let gross: f64 = spec.assets.iter().map(|a| a.weight.abs()).sum();
    let net = spec.assets.iter().map(|a| a.weight).sum();
    let hhi: f64 = if gross > 0.0 {
        spec.assets.iter().map(|a| (a.weight / gross).powi(2)).sum()
    } else {
        0.0
    };
    let means: Vec<f64> = (0..m)
        .map(|i| {
            if n > 0 {
                rows.iter().map(|r| r.returns[i]).sum::<f64>() / n as f64
            } else {
                0.0
            }
        })
        .collect();
    let mut cov = vec![vec![0.0; m]; m];
    if enough {
        for i in 0..m {
            for j in 0..=i {
                checkpoint()?;
                let c = rows
                    .iter()
                    .map(|r| (r.returns[i] - means[i]) * (r.returns[j] - means[j]))
                    .sum::<f64>()
                    / (n - spec.ddof as usize) as f64;
                cov[i][j] = c;
                cov[j][i] = c;
            }
        }
    }
    // Sum squared weighted deviations directly to avoid negative cancellation in w'Σw.
    let variance = if enough {
        rows.iter()
            .map(|r| {
                spec.assets
                    .iter()
                    .enumerate()
                    .map(|(i, a)| a.weight * (r.returns[i] - means[i]))
                    .sum::<f64>()
                    .powi(2)
            })
            .sum::<f64>()
            / (n - spec.ddof as usize) as f64
    } else {
        0.0
    };
    let scale = spec.periods_per_year.sqrt();
    let sd = variance.sqrt();
    let estimate = |v| {
        if enough {
            Scalar::number(v)
        } else {
            Scalar::InsufficientData
        }
    };
    let divided = |a, b| {
        if !enough {
            Scalar::InsufficientData
        } else if b == 0.0 {
            Scalar::undefined("zero_volatility")
        } else {
            Scalar::number(a / b)
        }
    };
    let mut assets = Vec::with_capacity(m);
    for (i, a) in spec.assets.iter().enumerate() {
        checkpoint()?;
        let marginal: f64 = spec
            .assets
            .iter()
            .enumerate()
            .map(|(j, b)| cov[i][j] * b.weight)
            .sum();
        assets.push(AssetRisk {
            id: a.id.clone(),
            signed_exposure: a.weight * spec.equity,
            gross_weight_share: if gross == 0.0 {
                Scalar::undefined("zero_gross_exposure")
            } else {
                Scalar::number(a.weight.abs() / gross)
            },
            annualized_volatility: estimate(cov[i][i].sqrt() * scale),
            volatility_contribution: divided(a.weight * marginal * scale, sd),
        });
    }
    let mut scenarios = Vec::with_capacity(spec.scenarios.len());
    for s in &spec.scenarios {
        checkpoint()?;
        let pnl_by_asset: Vec<_> = spec
            .assets
            .iter()
            .zip(&s.shocks)
            .map(|(a, r)| spec.equity * a.weight * r)
            .collect();
        let pnl: f64 = pnl_by_asset.iter().sum();
        scenarios.push(ScenarioResult {
            id: s.id.clone(),
            pnl_by_asset,
            pnl,
            loss: -pnl,
            return_on_equity: pnl / spec.equity,
            equity_after: spec.equity + pnl,
        });
    }
    Ok(PortfolioResult {
        method_version:VERSION.into(), currency:spec.currency.clone(), selected_observations:n,
        selected_range_ms:rows.first().zip(rows.last()).map(|(a,b)| [a.at_ms,b.at_ms]),
        selection_hash:fingerprint::digest(&rows)?, gross_exposure_ratio:gross, net_exposure_ratio:net,
        concentration_hhi:if gross == 0.0 {Scalar::undefined("zero_gross_exposure")} else {Scalar::number(hhi)},
        effective_asset_count:if gross == 0.0 {Scalar::undefined("zero_gross_exposure")} else {Scalar::number(1.0/hhi)},
        covariance_per_period:cov.iter().map(|row| row.iter().map(|v| estimate(*v)).collect()).collect(),
        correlation:(0..m).map(|i| (0..m).map(|j| divided(cov[i][j],cov[i][i].sqrt()*cov[j][j].sqrt())).collect()).collect(),
        annualized_volatility:estimate(sd*scale),
        diversification_ratio:divided(spec.assets.iter().enumerate().map(|(i,a)| a.weight.abs()*cov[i][i].sqrt()).sum(),sd),
        assets,scenarios,
        assumptions:vec![
            "analysis_only; not an order approval or account risk check".into(),
            "fixed signed linear exposures; common-currency aligned simple total returns supplied by caller; no FX conversion".into(),
            "historical covariance applied to supplied weights is a scenario estimate, not a realized strategy backtest".into(),
            "annualization uses square-root-time scaling; serial dependence may invalidate this assumption".into(),
            "stress shocks are caller-defined, not probabilities; no nonlinear derivatives, fees, slippage, funding or liquidation model".into(),
            "zero volatility makes correlation/contribution/diversification undefined; fewer than two selected rows is insufficient data".into(),
        ],
    })
}
