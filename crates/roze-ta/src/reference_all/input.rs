use super::{invalid, limit, upstream, MAX_ITEMS};
use crate::{error::TaError, wickra_all as w};
use serde::de::DeserializeOwned;
use serde_json::Value;

/// Decoding alone never establishes validity: every rich input is reconstructed
/// through its validated constructor before an algorithm sees it.
pub(super) trait WireInput: DeserializeOwned + Clone + Send + 'static {
    fn validate(self, at_ms: i64) -> Result<Self, TaError>;
}
fn same_time(timestamp: i64, at_ms: i64) -> Result<(), TaError> {
    if timestamp != at_ms {
        return Err(invalid(
            "payload timestamp must equal the sample at_ms (UTC milliseconds)",
        ));
    }
    Ok(())
}
impl WireInput for f64 {
    fn validate(self, _: i64) -> Result<Self, TaError> {
        if !self.is_finite() || self.abs() > 1e50 {
            return Err(invalid("scalar must be finite and within 1e50"));
        }
        Ok(self)
    }
}
impl<A: WireInput, B: WireInput> WireInput for (A, B) {
    fn validate(self, at_ms: i64) -> Result<Self, TaError> {
        Ok((self.0.validate(at_ms)?, self.1.validate(at_ms)?))
    }
}
impl WireInput for w::Candle {
    fn validate(self, at_ms: i64) -> Result<Self, TaError> {
        same_time(self.timestamp, at_ms)?;
        if self.low <= 0.0 {
            return Err(invalid("reference candle prices must be positive"));
        }
        w::Candle::new(
            self.open,
            self.high,
            self.low,
            self.close,
            self.volume,
            self.timestamp,
        )
        .map_err(upstream)
    }
}
impl WireInput for w::Tick {
    fn validate(self, at_ms: i64) -> Result<Self, TaError> {
        same_time(self.timestamp, at_ms)?;
        if self.price <= 0.0 {
            return Err(invalid("tick price must be positive"));
        }
        w::Tick::new(self.price, self.volume, self.timestamp).map_err(upstream)
    }
}
impl WireInput for w::Trade {
    fn validate(self, at_ms: i64) -> Result<Self, TaError> {
        same_time(self.timestamp, at_ms)?;
        if self.price <= 0.0 {
            return Err(invalid("trade price must be positive"));
        }
        w::Trade::new(self.price, self.size, self.side, self.timestamp).map_err(upstream)
    }
}
impl WireInput for w::TradeQuote {
    fn validate(self, at_ms: i64) -> Result<Self, TaError> {
        w::TradeQuote::new(self.trade.validate(at_ms)?, self.mid).map_err(upstream)
    }
}
impl WireInput for w::OrderBook {
    fn validate(self, _: i64) -> Result<Self, TaError> {
        if self.bids.len() > MAX_ITEMS || self.asks.len() > MAX_ITEMS {
            return Err(limit("order book sides are limited to 128 levels"));
        }
        if self.bids.iter().chain(&self.asks).any(|l| l.price <= 0.0) {
            return Err(invalid("order book prices must be positive"));
        }
        w::OrderBook::new(self.bids, self.asks).map_err(upstream)
    }
}
impl WireInput for w::CrossSection {
    fn validate(self, at_ms: i64) -> Result<Self, TaError> {
        same_time(self.timestamp, at_ms)?;
        if self.members.len() > MAX_ITEMS {
            return Err(limit("cross sections are limited to 128 members"));
        }
        w::CrossSection::new(self.members, self.timestamp).map_err(upstream)
    }
}
impl WireInput for w::DerivativesTick {
    fn validate(self, at_ms: i64) -> Result<Self, TaError> {
        same_time(self.timestamp, at_ms)?;
        w::DerivativesTick::new(
            self.funding_rate,
            self.mark_price,
            self.index_price,
            self.futures_price,
            self.open_interest,
            self.long_size,
            self.short_size,
            self.taker_buy_volume,
            self.taker_sell_volume,
            self.long_liquidation,
            self.short_liquidation,
            self.timestamp,
        )
        .map_err(upstream)
    }
}

pub(super) fn decode<I: WireInput>(value: &Value, at_ms: i64) -> Result<I, TaError> {
    serde_json::from_value::<I>(value.clone())
        .map_err(|e| invalid(&format!("invalid typed input: {e}")))?
        .validate(at_ms)
}

pub(super) fn validate_json(value: &Value, depth: usize) -> Result<(), TaError> {
    if depth > 8 {
        return Err(limit("input JSON depth exceeds 8"));
    }
    match value {
        Value::Number(n) if n.as_f64().is_none_or(|x| !x.is_finite() || x.abs() > 1e50) => {
            Err(invalid("numbers must be finite and within 1e50"))
        }
        Value::Array(a) => {
            if a.len() > MAX_ITEMS {
                return Err(limit("input arrays exceed 128 items"));
            }
            for x in a {
                validate_json(x, depth + 1)?;
            }
            Ok(())
        }
        Value::Object(o) => {
            if o.len() > 32 {
                return Err(limit("input object exceeds 32 fields"));
            }
            for x in o.values() {
                validate_json(x, depth + 1)?;
            }
            Ok(())
        }
        Value::String(s) if s.len() > 128 => Err(limit("input string exceeds 128 bytes")),
        _ => Ok(()),
    }
}
