use super::*;
use crate::core::OHLCV;
use crate::methods::SMA;
use crate::{
    indicators::*,
    prelude::{IndicatorConfig, IndicatorInstance, Method},
};

// Yata 0.7.0 cannot deserialize the empty Window used by windowless ADI.
// Keep its default Chaikin arithmetic, using upstream EMA/CLV and an explicit
// accumulator. This avoids modifying vendor files or storing unbounded history.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(super) struct ChaikinState {
    sum: f64,
    short: Option<crate::methods::EMA>,
    long: Option<crate::methods::EMA>,
}
impl ChaikinState {
    fn step(&mut self, candle: &[f64; 5]) -> Result<Vec<f64>, TaError> {
        if self.short.is_none() {
            self.short = Some(crate::methods::EMA::new(3, &0.0).map_err(|_| upstream_error())?);
            self.long = Some(crate::methods::EMA::new(10, &0.0).map_err(|_| upstream_error())?);
        }
        self.sum += candle.clv() * candle.volume();
        let short = self
            .short
            .as_mut()
            .ok_or_else(upstream_error)?
            .next(&self.sum);
        let long = self
            .long
            .as_mut()
            .ok_or_else(upstream_error)?
            .next(&self.sum);
        Ok(vec![short - long])
    }
}

/// Variants are an internal, versioned snapshot layout. Never reorder silently.
macro_rules! kernels {
    ($($variant:ident => ($id:literal, $config:ty)),+ $(,)?) => {
        #[derive(Clone, Debug, Serialize, Deserialize)]
        pub(super) enum Kernel {
            Ema(crate::EmaState), Sma(u8, Option<SMA>),
            Rsi(crate::RsiState), Atr(crate::AtrState),
            Chaikin(ChaikinState),
            $($variant(Option<Box<<$config as IndicatorConfig>::Instance>>)),+,
            Extension(Box<super::extensions::Extension>),
        }
        impl Kernel {
            pub(super) fn new(id: &str) -> Result<Self, TaError> {
                if let Some(period) = id.strip_prefix("ema.").and_then(|p| p.parse::<usize>().ok()) {
                    return crate::EmaState::new(period).map(Self::Ema).ok_or_else(upstream_error);
                }
                if let Some(period) = id.strip_prefix("sma.").and_then(|p| p.parse::<u8>().ok()) {
                    return Ok(Self::Sma(period, None));
                }
                match id {
                    "rsi.14" => crate::RsiState::new(14).map(Self::Rsi).ok_or_else(upstream_error),
                    "atr.14" => crate::AtrState::new(14).map(Self::Atr).ok_or_else(upstream_error),
                    "chaikin.default" => Ok(Self::Chaikin(ChaikinState::default())),
                    $($id => Ok(Self::$variant(None))),+,
                    _ => super::extensions::Extension::new(id).map(|s|Self::Extension(Box::new(s))),
                }
            }
            pub(super) fn step(&mut self, bar: &Candle) -> Result<Vec<f64>, TaError> {
                let candle = [bar.open, bar.high, bar.low, bar.close, bar.volume];
                match self {
                    Self::Ema(state) => state.next(bar.close).map(|v|vec![v]).ok_or_else(upstream_error),
                    Self::Sma(period, state) => {
                        if state.is_none() { *state = Some(SMA::new(*period, &bar.close).map_err(|_|upstream_error())?); }
                        Ok(vec![state.as_mut().ok_or_else(upstream_error)?.next(&bar.close)])
                    }
                    Self::Rsi(state) => Ok(state.next(bar.close).into_iter().collect()),
                    Self::Atr(state) => state.next(crate::Bar { open:bar.open, high:bar.high, low:bar.low, close:bar.close, volume:bar.volume })
                        .map(|v|vec![v]).ok_or_else(upstream_error),
                    Self::Chaikin(state) => state.step(&candle),
                    Self::Extension(state) => state.step(bar),
                    $(Self::$variant(state) => {
                        if state.is_none() { *state = Some(Box::new(<$config>::default().init(&candle).map_err(|_|upstream_error())?)); }
                        Ok(state.as_mut().ok_or_else(upstream_error)?.next(&candle).values().to_vec())
                    }),+
                }
            }
            pub(super) fn kind(&self) -> &'static str {
                match self {
                    Self::Ema(_) => "ema", Self::Sma(_,_) => "sma", Self::Rsi(_) => "rsi.14", Self::Atr(_) => "atr.14",
                    Self::Chaikin(_) => "chaikin.default",
                    Self::Extension(state) => state.kind(),
                    $(Self::$variant(_) => $id),+
                }
            }
            pub(super) fn valid_for(&self, id: &str, samples: u64) -> bool {
                let initialized = samples > 0;
                match self {
                    Self::Ema(s) => id == format!("ema.{}",s.period) && s.method.is_some()==initialized && s.value.is_some()==initialized,
                    Self::Sma(period,s) => id == format!("sma.{period}") && s.is_some()==initialized,
                    Self::Rsi(s) => id=="rsi.14" && s.period==14 && s.previous.is_some()==initialized
                        && s.gains.is_some()==(samples>1) && s.losses.is_some()==(samples>1) && s.value.is_some()==(samples>1),
                    Self::Atr(s) => id=="atr.14" && s.period==14 && s.samples==samples && s.true_range.is_some()==initialized
                        && s.average.is_some()==initialized && s.value.is_some()==initialized,
                    Self::Chaikin(s) => id=="chaikin.default" && s.short.is_some()==initialized && s.long.is_some()==initialized,
                    Self::Extension(s) => s.valid_for(id,samples),
                    $(Self::$variant(s) => id==$id && s.is_some()==initialized && s.as_ref().is_none_or(|state|
                        serde_json::to_value(state.config()).ok() == serde_json::to_value(<$config>::default()).ok()
                    )),+
                }
            }
        }
    };
}
fn upstream_error() -> TaError {
    TaError::new(ErrorCode::UpstreamFailure, "indicator calculation failed")
}

kernels! {
    Macd => ("macd.default", MACD),
    Bollinger => ("bollinger.default", BollingerBands),
    Adx => ("adx.default", AverageDirectionalIndex),
    Cci => ("cci.default", CommodityChannelIndex),
    Stochastic => ("stochastic.default", StochasticOscillator),
    Donchian => ("donchian.default", DonchianChannel),
    Aroon => ("aroon.default", Aroon),
    Cmf => ("cmf.default", ChaikinMoneyFlow),
    Sar => ("sar.default", ParabolicSAR),
    Hma => ("hma.default", HullMovingAverage),
    Kama => ("kama.default", Kaufman),
    Ichimoku => ("ichimoku.default", IchimokuCloud),
    Keltner => ("keltner.default", KeltnerChannel),
    Mfi => ("mfi.default", MoneyFlowIndex),
    Trix => ("trix.default", Trix),
    Cmo => ("cmo.default", ChandeMomentumOscillator),
    Efi => ("efi.default", EldersForceIndex),
    Ao => ("ao.default", AwesomeOscillator),
}
