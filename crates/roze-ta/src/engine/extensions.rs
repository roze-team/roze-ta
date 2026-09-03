//! A1 fixed-profile kernels. Existing Kernel variants stay in their original order.
use super::*;
use crate::{
    core::OHLCV,
    methods::{Momentum, DEMA, RMA, ROC, TEMA, WMA},
    prelude::Method,
};
use std::collections::VecDeque;

const PERIOD: usize = 14;
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(super) struct Accumulator {
    previous: Option<f64>,
    sum: f64,
}

macro_rules! methods {
    ($($variant:ident => ($id:literal,$method:ty)),+ $(,)?) => {
        #[derive(Clone,Debug,Serialize,Deserialize)]
        pub(super) enum Extension {
            $($variant(Option<$method>)),+,
            Vwma(VecDeque<(f64,f64)>),
            Ad(Accumulator), Obv(Accumulator),
            Williams(VecDeque<(f64,f64)>),
        }
        impl Extension {
            pub(super) fn new(id:&str)->Result<Self,TaError> {
                match id {
                    $($id=>Ok(Self::$variant(None))),+,
                    "vwma.14"=>Ok(Self::Vwma(VecDeque::new())),
                    "ad.cumulative"=>Ok(Self::Ad(Accumulator::default())),
                    "obv.zero_seed"=>Ok(Self::Obv(Accumulator::default())),
                    "williams_r.14"=>Ok(Self::Williams(VecDeque::new())),
                    _=>Err(TaError::new(ErrorCode::UnsupportedProfile,"unregistered extension profile"))
                }
            }
            pub(super) fn kind(&self)->&'static str {
                match self {
                    $(Self::$variant(_)=>$id),+,
                    Self::Vwma(_)=>"vwma.14",Self::Ad(_)=>"ad.cumulative",Self::Obv(_)=>"obv.zero_seed",Self::Williams(_)=>"williams_r.14"
                }
            }
            pub(super) fn valid_for(&self,id:&str,samples:u64)->bool {
                if self.kind()!=id {return false;}
                let expected=samples.min(PERIOD as u64) as usize;
                match self {
                    $(Self::$variant(s)=>s.is_some()==(samples>0)),+,
                    Self::Vwma(w)=>w.len()==expected&&w.iter().all(|(p,v)|p.is_finite()&&*p>0.&&*p<=1e100&&v.is_finite()&&*v>=0.&&*v<=1e100),
                    Self::Williams(w)=>w.len()==expected&&w.iter().all(|(h,l)|h.is_finite()&&l.is_finite()&&*l>0.&&h>=l&&*h<=1e100),
                    Self::Ad(s)|Self::Obv(s)=>s.sum.is_finite()&&s.previous.is_some()==(samples>0)&&s.previous.is_none_or(|v|v.is_finite()&&v>0.&&v<=1e100)&& (samples>0||s.sum==0.)
                }
            }
            pub(super) fn step(&mut self,bar:&Candle)->Result<Vec<f64>,TaError> {
                let value=match self {
                    $(Self::$variant(state)=>{
                        if state.is_none(){*state=Some(<$method>::new(PERIOD as u8,&bar.close).map_err(|_|failure())?);}
                        state.as_mut().ok_or_else(failure)?.next(&bar.close)
                    }),+,
                    Self::Vwma(window)=>{
                        push(window,(bar.close,bar.volume));
                        let volume: f64=window.iter().map(|p|p.1).sum();
                        // Recompute the bounded 14-bar sums. Rolling subtraction can leave
                        // residual weights after the last nonzero-volume bar expires.
                        if volume==0. {f64::NAN} else {window.iter().map(|(p,v)|p*v).sum::<f64>()/volume}
                    }
                    Self::Ad(state)=>{
                        let candle=[bar.open,bar.high,bar.low,bar.close,bar.volume];
                        state.sum+=candle.clv()*bar.volume;
                        state.previous=Some(bar.close);
                        state.sum
                    }
                    Self::Obv(state)=>{
                        if let Some(previous)=state.previous {
                            if bar.close>previous {state.sum+=bar.volume;} else if bar.close<previous {state.sum-=bar.volume;}
                        }
                        state.previous=Some(bar.close);
                        state.sum
                    }
                    Self::Williams(window)=>{
                        push(window,(bar.high,bar.low));
                        let high=window.iter().map(|p|p.0).fold(f64::NEG_INFINITY,f64::max);
                        let low=window.iter().map(|p|p.1).fold(f64::INFINITY,f64::min);
                        if high==low {f64::NAN} else {-100.*((high-bar.close)/(high-low))}
                    }
                };
                Ok(vec![value])
            }
        }
    }
}
fn push(window: &mut VecDeque<(f64, f64)>, value: (f64, f64)) {
    if window.len() == PERIOD {
        window.pop_front();
    }
    window.push_back(value);
}
fn failure() -> TaError {
    TaError::new(
        ErrorCode::UpstreamFailure,
        "A1 method initialization failed",
    )
}
methods! {
    Wma => ("wma.14",WMA), Rma => ("rma.14",RMA),
    Dema => ("dema.14",DEMA), Tema => ("tema.14",TEMA),
    Roc => ("roc.14",ROC), Momentum => ("momentum.14",Momentum),
}
