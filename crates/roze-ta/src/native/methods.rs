// Dispatch only; all arithmetic lives in crate::methods.
use super::*;
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Period {
    period: u8,
}
fn period(v: &Value) -> Result<u8, TaError> {
    let p: Period = decode(v)?;
    if p.period == 255 {
        return Err(invalid("native period must be <=254"));
    }
    Ok(p.period)
}
fn count(v: &Value) -> Result<usize, TaError> {
    Ok(usize::from(period(v)?))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Unit {}
fn unit(v: &Value) -> Result<(), TaError> {
    let _: Unit = decode(v)?;
    Ok(())
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Two {
    first: u8,
    second: u8,
}
fn two(v: &Value) -> Result<(u8, u8), TaError> {
    let p: Two = decode(v)?;
    Ok((p.first, p.second))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Weights {
    weights: Vec<f64>,
}
fn weights(v: &Value) -> Result<Vec<f64>, TaError> {
    let p: Weights = decode(v)?;
    let sum: f64 = p.weights.iter().sum();
    if !sum.is_finite() || sum == 0.0 {
        return Err(invalid("weights need a finite nonzero sum"));
    }
    Ok(p.weights)
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RenkoParams {
    size: f64,
    source: crate::core::Source,
}
fn renko(v: &Value) -> Result<(f64, crate::core::Source), TaError> {
    let p: RenkoParams = decode(v)?;
    Ok((p.size, p.source))
}
pub(super) fn runner<'a>(
    symbol: &str,
    params: &Value,
    data: &'a Input,
) -> Result<Runner<'a>, TaError> {
    match symbol {
        "ADI" => bars::<crate::methods::ADI>(period(params)?, data),
        "CCI" => scalar::<crate::methods::CCI>(period(params)?, data),
        "CollapseTimeframe" => {
            owned_bars::<crate::methods::CollapseTimeframe<Candle>>(count(params)?, data)
        }
        "Conv" => scalar::<crate::methods::Conv>(weights(params)?, data),
        "Cross" => pair::<crate::methods::Cross>(unit(params)?, data),
        "CrossAbove" => pair::<crate::methods::CrossAbove>(unit(params)?, data),
        "CrossUnder" => pair::<crate::methods::CrossUnder>(unit(params)?, data),
        "Derivative" => scalar::<crate::methods::Derivative>(period(params)?, data),
        "EMA" => scalar::<crate::methods::EMA>(period(params)?, data),
        "DMA" => scalar::<crate::methods::DMA>(period(params)?, data),
        "TMA" => scalar::<crate::methods::TMA>(period(params)?, data),
        "DEMA" => scalar::<crate::methods::DEMA>(period(params)?, data),
        "TEMA" => scalar::<crate::methods::TEMA>(period(params)?, data),
        "HeikinAshi" => bars::<crate::methods::HeikinAshi>(unit(params)?, data),
        "HighestLowestDelta" => scalar::<crate::methods::HighestLowestDelta>(period(params)?, data),
        "Highest" => scalar::<crate::methods::Highest>(period(params)?, data),
        "Lowest" => scalar::<crate::methods::Lowest>(period(params)?, data),
        "HighestIndex" => scalar::<crate::methods::HighestIndex>(period(params)?, data),
        "LowestIndex" => scalar::<crate::methods::LowestIndex>(period(params)?, data),
        "HMA" => scalar::<crate::methods::HMA>(period(params)?, data),
        "Integral" => scalar::<crate::methods::Integral>(period(params)?, data),
        "LinReg" => scalar::<crate::methods::LinReg>(period(params)?, data),
        "MeanAbsDev" => scalar::<crate::methods::MeanAbsDev>(period(params)?, data),
        "MedianAbsDev" => scalar::<crate::methods::MedianAbsDev>(period(params)?, data),
        "Momentum" => scalar::<crate::methods::Momentum>(period(params)?, data),
        "Past" => past(period(params)?, data),
        "RateOfChange" => scalar::<crate::methods::RateOfChange>(period(params)?, data),
        "Renko" => bars::<crate::methods::Renko>(renko(params)?, data),
        "ReversalSignal" => scalar::<crate::methods::ReversalSignal>(two(params)?, data),
        "UpperReversalSignal" => scalar::<crate::methods::UpperReversalSignal>(two(params)?, data),
        "LowerReversalSignal" => scalar::<crate::methods::LowerReversalSignal>(two(params)?, data),
        "RMA" => scalar::<crate::methods::RMA>(period(params)?, data),
        "SMA" => scalar::<crate::methods::SMA>(period(params)?, data),
        "SMM" => scalar::<crate::methods::SMM>(period(params)?, data),
        "StDev" => scalar::<crate::methods::StDev>(period(params)?, data),
        "SWMA" => scalar::<crate::methods::SWMA>(period(params)?, data),
        "TR" => bars::<crate::methods::TR>(unit(params)?, data),
        "TRIMA" => scalar::<crate::methods::TRIMA>(period(params)?, data),
        "TSI" => scalar::<crate::methods::TSI>(two(params)?, data),
        "Vidya" => scalar::<crate::methods::Vidya>(period(params)?, data),
        "LinearVolatility" => scalar::<crate::methods::LinearVolatility>(period(params)?, data),
        "VWMA" => pair::<crate::methods::VWMA>(period(params)?, data),
        "WMA" => scalar::<crate::methods::WMA>(period(params)?, data),
        "WSMA" => scalar::<crate::methods::WSMA>(period(params)?, data),
        _ => Err(invalid("unknown native method")),
    }
}
