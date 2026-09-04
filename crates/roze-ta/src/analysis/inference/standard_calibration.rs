//! MacKinnon response surfaces and sequential Johansen rank selection.
use super::*;
use calibration_tables::*;
use statrs::distribution::Normal;

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum MacKinnonCase {
    AdfNone,
    AdfConstant,
    AdfLinear,
    EngleGrangerConstant,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct MacKinnonCalibration {
    pub method_version: String,
    pub case: MacKinnonCase,
    pub approximate_p_value: f64,
    pub critical_values: [f64; 3],
    pub critical_value_observations: usize,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum Significance {
    TenPercent,
    FivePercent,
    OnePercent,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum RankTest {
    Trace,
    MaxEigenvalue,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct RankSelection {
    pub method_version: String,
    pub test: RankTest,
    pub significance: Significance,
    pub selected_rank: usize,
    pub tested_ranks: Vec<usize>,
    pub statistics: Vec<f64>,
    pub critical_values: Vec<f64>,
    pub rejected: Vec<bool>,
}
/// 1994 asymptotic tau p-value and 2010 finite-sample critical values (1%, 5%, 10%).
/// `observations` is effective ADF regression n, or original EG sample length minus one.
pub fn calibrate_statistic(
    statistic: f64,
    case: MacKinnonCase,
    observations: usize,
) -> Result<MacKinnonCalibration, TaError> {
    if !statistic.is_finite() || !(1..=MAX_SAMPLES).contains(&observations) {
        return Err(bad(
            "MacKinnon requires a finite tau statistic and observations 1..4096",
        ));
    }
    let i = match case {
        MacKinnonCase::AdfNone => 0,
        MacKinnonCase::AdfConstant => 1,
        MacKinnonCase::AdfLinear => 2,
        MacKinnonCase::EngleGrangerConstant => 3,
    };
    let (minimum, maximum, star) = [
        (-19.04, f64::INFINITY, -1.04),
        (-18.83, 2.74, -1.61),
        (-16.18, 0.7, -2.89),
        (-18.86, 0.92, -2.62),
    ][i];
    let polynomial =
        |coefficients: &[f64], x: f64| coefficients.iter().rev().fold(0.0, |v, c| v * x + c);
    let approximate_p_value = if statistic < minimum {
        0.0
    } else if statistic > maximum {
        1.0
    } else {
        let coefficients: &[f64] = if statistic <= star {
            &SMALL[i]
        } else {
            &LARGE[i]
        };
        Normal::new(0.0, 1.0)
            .map_err(|_| bad("normal construction failed"))?
            .cdf(polynomial(coefficients, statistic))
    };
    let critical_values = CRITICAL[i].map(|c| polynomial(&c, 1.0 / observations as f64));
    Ok(MacKinnonCalibration {
        method_version: "mackinnon-1994-tau-2010-critical/statsmodels-0.14.6-subset-v1".into(),
        case,
        approximate_p_value,
        critical_values,
        critical_value_observations: observations,
    })
}
fn base(spec: &InferenceSpec) -> Result<InferenceSpec, TaError> {
    let task = match &spec.task {
        InferenceTask::AdfMacKinnon {
            samples,
            lags,
            trend,
        } => InferenceTask::Adf {
            samples: samples.clone(),
            lags: *lags,
            trend: *trend,
        },
        InferenceTask::EngleGrangerMacKinnon {
            dependent,
            independent,
            lags,
        } => InferenceTask::EngleGranger {
            dependent: dependent.clone(),
            independent: independent.clone(),
            lags: *lags,
        },
        InferenceTask::JohansenRank {
            observations,
            lagged_differences,
            include_constant,
            ..
        } => InferenceTask::Johansen {
            observations: observations.clone(),
            lagged_differences: *lagged_differences,
            include_constant: *include_constant,
            rank: 1,
        },
        _ => return Err(bad("expected standard calibration task")),
    };
    Ok(InferenceSpec {
        available_at_ms: spec.available_at_ms,
        task,
    })
}
pub(super) fn validate(spec: &InferenceSpec, r: &Request) -> Result<(usize, usize), TaError> {
    let (work, output) = base(spec)?.validate(r)?;
    if let InferenceTask::JohansenRank {
        observations,
        lagged_differences,
        include_constant,
        forecast_steps,
        ..
    } = &spec.task
    {
        let (extra, out) = InferenceSpec {
            available_at_ms: spec.available_at_ms,
            task: InferenceTask::Vecm {
                observations: observations.clone(),
                lagged_differences: *lagged_differences,
                include_constant: *include_constant,
                rank: 1,
                steps: forecast_steps.unwrap_or(1),
            },
        }
        .validate(r)?;
        Ok((work + extra, out + 64))
    } else {
        Ok((work + 64, output + 16))
    }
}
fn number(out: &InferenceResult, name: &str) -> Result<f64, TaError> {
    out.values
        .iter()
        .find_map(|v| match v.value {
            Scalar::Ready { value } if v.name == name && value.is_finite() => Some(value),
            _ => None,
        })
        .ok_or_else(|| linalg::failure("undefined statistic cannot be calibrated"))
}
pub(super) fn run(
    spec: &InferenceSpec,
    checkpoint: &mut impl FnMut() -> Result<(), TaError>,
) -> Result<InferenceResult, TaError> {
    let mut out = calculate(&base(spec)?, checkpoint)?;
    let (case, n) = match &spec.task {
        InferenceTask::AdfMacKinnon {
            samples,
            lags,
            trend,
        } => (
            match trend {
                Trend::None => MacKinnonCase::AdfNone,
                Trend::Constant => MacKinnonCase::AdfConstant,
                Trend::Linear => MacKinnonCase::AdfLinear,
            },
            samples.len() - lags - 1,
        ),
        InferenceTask::EngleGrangerMacKinnon { dependent, .. } => {
            (MacKinnonCase::EngleGrangerConstant, dependent.len() - 1)
        }
        InferenceTask::JohansenRank {
            observations,
            lagged_differences,
            include_constant,
            significance,
            test,
            forecast_steps,
        } => {
            let column = match significance {
                Significance::TenPercent => 0,
                Significance::FivePercent => 1,
                Significance::OnePercent => 2,
            };
            let table = match (test, include_constant) {
                (RankTest::Trace, false) => TJCP0,
                (RankTest::Trace, true) => TJCP1,
                (RankTest::MaxEigenvalue, false) => EJCP0,
                (RankTest::MaxEigenvalue, true) => EJCP1,
            };
            let d = observations[0].len();
            let mut selection = RankSelection {
                method_version: "johansen-mhm-tables/statsmodels-0.14.6-subset-v1".into(),
                test: *test,
                significance: *significance,
                selected_rank: d,
                tested_ranks: vec![],
                statistics: vec![],
                critical_values: vec![],
                rejected: vec![],
            };
            for rank in 0..d {
                checkpoint()?;
                let prefix = match test {
                    RankTest::Trace => "trace",
                    RankTest::MaxEigenvalue => "max_eigen",
                };
                let statistic = number(&out, &format!("{prefix}_rank_{rank}"))?;
                let critical = table[d - rank - 1][column];
                let reject = statistic > critical;
                selection.tested_ranks.push(rank);
                selection.statistics.push(statistic);
                selection.critical_values.push(critical);
                selection.rejected.push(reject);
                if !reject {
                    selection.selected_rank = rank;
                    break;
                }
            }
            out = calculate(
                &InferenceSpec {
                    available_at_ms: spec.available_at_ms,
                    task: InferenceTask::Vecm {
                        observations: observations.clone(),
                        lagged_differences: *lagged_differences,
                        include_constant: *include_constant,
                        rank: selection.selected_rank,
                        steps: forecast_steps.unwrap_or(1),
                    },
                },
                checkpoint,
            )?;
            if forecast_steps.is_none() {
                out.vecm = None;
            }
            out.rank_selection = Some(selection);
            out.assumptions
                .retain(|a| !a.contains("requested rank is supplied"));
            out.assumptions.push("Sequential Johansen trace or maximum-eigenvalue test, stopping at first non-rejection (statistic <= tabulated critical value); rank=dimension if all nulls rejected. MacKinnon-Haug-Michelis tables as distributed by statsmodels 0.14.6, no deterministic part or unrestricted constant only, dimension 2..4, significance 10/5/1 percent. Asymptotic selection is not a finite-sample guarantee; fixed lag order; optional VECM forecasts use the selected rank but exclude selection uncertainty.".into());
            return Ok(out);
        }
        _ => return Err(bad("expected standard calibration task")),
    };
    out.mackinnon = Some(calibrate_statistic(
        number(&out, "adf_statistic")?,
        case,
        n,
    )?);
    out.assumptions
        .retain(|a| !a.contains("no p-value") && !a.contains("no ordinary Student-t"));
    out.assumptions.push("MacKinnon 1994 approximate asymptotic tau p-value, with 2010 finite-sample critical values ordered 1%,5%,10%. ADF uses N=1 and effective regression observations; intercept EG uses N=2 and original sample length minus one, regardless of residual ADF lag count. Deterministic terms and lag count fixed by caller. Small-sample accuracy, structural breaks and data-dependent model selection are not calibrated by these response surfaces.".into());
    checkpoint()?;
    Ok(out)
}
