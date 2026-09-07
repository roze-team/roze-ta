"""Audit existing mappings against pinned Python sources; mismatches stay visible.

This does not relabel a partial output projection as full API compatibility.
"""
import dataclasses,enum,inspect,json,math,random,subprocess,sys,warnings
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
from parity_sources import verify_source
verify_source('ta');verify_source('talipp')
sys.path[:0]=[str(ROOT/'target/cross-library/ta'),str(ROOT/'target/cross-library/talipp')]
import pandas as pd
import talipp.indicators as ti
from talipp.ohlcv import OHLCV
import ta.trend,ta.momentum,ta.volume,ta.volatility,ta.others
warnings.filterwarnings('ignore',category=RuntimeWarning)
mapping=json.loads((ROOT/'docs/reference-indicator-mapping.json').read_text(encoding='utf8'))['entries']
bridge=subprocess.Popen(['rtk','proxy',str(ROOT/'target/debug/examples/reference_compare.exe')],stdin=subprocess.PIPE,stdout=subprocess.PIPE,text=True,encoding='utf8')

def bars(case,n=256):
    rng=random.Random(51881); result=[]
    for i in range(n):
        if case=='flat':o=c=h=l=100.;v=0.
        elif case=='trend':o=100.+i*.1;c=o+.05;h=c+.1;l=o-.1;v=100.+i%5
        else:o=100+10*math.sin(i*.41)+rng.uniform(-4,4);c=o+rng.uniform(-4,4);h=max(o,c)+rng.uniform(0,4);l=min(o,c)-rng.uniform(0,4);v=10+rng.random()*100
        result.append(dict(open=o,high=h,low=l,close=c,volume=v,timestamp=i+1))
    return result

def clean(v):
    if dataclasses.is_dataclass(v):return clean(dataclasses.asdict(v))
    if isinstance(v,enum.Enum):return v.name
    if isinstance(v,dict):return {k:clean(x) for k,x in v.items()}
    if isinstance(v,(tuple,list)):return [clean(x) for x in v]
    if isinstance(v,(int,float)) or hasattr(v,'item'):
        v=float(v);return v if math.isfinite(v) else None
    return v

def equal(a,b):
    if a is None or b is None:return a is b
    if isinstance(a,dict):return isinstance(b,dict) and a.keys()==b.keys() and all(equal(a[k],b[k]) for k in a)
    if isinstance(a,(int,float)) and isinstance(b,(int,float)):return abs(a-b)<=2e-9*(1+abs(a))
    return a==b

def talipp_kwargs(name,p):
    kw=dict(p)
    def rename(a,b):
        if a in kw:kw[b]=kw.pop(a)
    for a,b in [('fast','fast_period'),('slow','slow_period'),('signal','signal_period'),('offset','offset')]:
        if a!=b:rename(a,b)
    if name=='ADX':kw={'di_period':p['period'],'adx_period':p['period']}
    elif name=='BB':rename('multiplier','std_dev_mult')
    elif name=='ChandeKrollStop':rename('atr_multiplier','atr_mult');rename('stop_period','period')
    elif name=='CoppockCurve':rename('roc_long_period','slow_roc_period');rename('roc_short_period','fast_roc_period')
    elif name=='EMV':kw['volume_div']=100000000
    elif name=='Ichimoku':rename('displacement','chikou_lag_period');rename('senkou_b_period','senkou_slow_period');kw['senkou_lookup_period']=p['displacement']
    elif name=='KAMA':kw={'period':p['er_period'],'fast_ema_constant_period':p['fast'],'slow_ema_constant_period':p['slow']}
    elif name=='KST':kw={**{f'roc{i}_period':p[f'roc{i}'] for i in range(1,5)},**{f'roc{i}_ma_period':p[f'sma{i}'] for i in range(1,5)},'signal_period':p['signal']}
    elif name=='KeltnerChannels':kw={'ma_period':p['ema_period'],'atr_period':p['atr_period'],'atr_mult_up':p['multiplier'],'atr_mult_down':p['multiplier']}
    elif name=='MassIndex':kw={'ma_period':p['ema_period'],'ma_ma_period':p['ema_period'],'ma_ratio_period':p['sum_period']}
    elif name=='ParabolicSAR':kw={'init_accel_factor':p['af_start'],'accel_factor_inc':p['af_step'],'max_accel_factor':p['af_max']}
    elif name=='PivotsHL':kw={'high_period':14,'low_period':14}
    elif name=='RogersSatchell':kw={'period':p['period']}
    elif name=='SFX':kw={k:p['period'] for k in ['atr_period','std_dev_period','std_dev_smoothing_period']}
    elif name=='STC':kw={'fast_macd_period':p['fast'],'slow_macd_period':p['slow'],'stoch_period':p['schaff_period'],'stoch_smoothing_period':3}
    elif name=='Stoch':kw={'period':p['k_period'],'smoothing_period':p['d_period']}
    elif name=='StochRSI':kw.update(k_smoothing_period=3,d_smoothing_period=3)
    elif name=='SuperTrend':rename('multiplier','mult')
    elif name=='T3':rename('v','factor')
    elif name=='TSI':kw={'fast_period':p['short'],'slow_period':p['long']}
    elif name=='TTM':rename('bb_mult','bb_std_dev_mult');rename('kc_mult','kc_atr_mult')
    elif name=='UO':kw={'fast_period':p['short'],'mid_period':p['mid'],'slow_period':p['long']}
    elif name=='ZigZag':kw={'sensitivity':p['threshold'],'min_trend_length':1}
    return kw

ALIASES={'plus_di':'plus_di','minus_di':'minus_di','lb':'lower','cb':'middle','ub':'upper','histogram':'histogram','base_line':'kijun','conversion_line':'tenkan','lagging_line':'chikou','cloud_leading_fast_line':'senkou_a','cloud_leading_slow_line':'senkou_b','plus_vtx':'plus','minus_vtx':'minus','std_dev':'stddev','ma_std_dev':'smoothed_stddev'}
def talipp_expected(name,p,candles):
    if name=='FibonacciRetracement':
        return [ti.FibonacciRetracement.get_retracement_value(.618,c['close']) for c in candles],{'fibonacci_coefficient':.618},'static helper is not the mapped pivot indicator'
    cls=getattr(ti,name);kw=talipp_kwargs(name,p);obj=cls(**kw)
    annotation=str(inspect.signature(cls).parameters['input_values'].annotation)
    result=[]
    for c in candles:
        obj.add(OHLCV(c['open'],c['high'],c['low'],c['close'],c['volume']) if 'OHLCV' in annotation else c['close'])
        v=clean(obj[-1]) if len(obj) else None
        if isinstance(v,dict):
            v={ALIASES.get(k,k):x for k,x in v.items()}
            if name=='TTM':v['momentum']=v.pop('histogram')
            if name=='SuperTrend':v['direction']=1. if v.pop('trend')=='UP' else -1.
        result.append(v)
    return result,kw,'event output' if name in ('PivotsHL','ZigZag') else ''

# Explicit getter names: no inference from resulting values.
TA_METHODS={
 'ADXIndicator':{'adx':'adx','plus_di':'adx_pos','minus_di':'adx_neg'},'AccDistIndexIndicator':'acc_dist_index',
 'AroonIndicator':{'up':'aroon_up','down':'aroon_down'},'AverageTrueRange':'average_true_range','AwesomeOscillatorIndicator':'awesome_oscillator',
 'BollingerBands':{'lower':'bollinger_lband','middle':'bollinger_mavg','upper':'bollinger_hband'},'CCIIndicator':'cci','ChaikinMoneyFlowIndicator':'chaikin_money_flow',
 'CumulativeReturnIndicator':'cumulative_return','DPOIndicator':'dpo','DailyLogReturnIndicator':'daily_log_return','DailyReturnIndicator':'daily_return',
 'DonchianChannel':{'lower':'donchian_channel_lband','middle':'donchian_channel_mband','upper':'donchian_channel_hband'},'EMAIndicator':'ema_indicator',
 'EaseOfMovementIndicator':'sma_ease_of_movement','ForceIndexIndicator':'force_index','IchimokuIndicator':{'tenkan':'ichimoku_conversion_line','kijun':'ichimoku_base_line','senkou_a':'ichimoku_a','senkou_b':'ichimoku_b'},
 'KAMAIndicator':'kama','KSTIndicator':{'kst':'kst','signal':'kst_sig'},'KeltnerChannel':{'lower':'keltner_channel_lband','middle':'keltner_channel_mband','upper':'keltner_channel_hband'},
 'MACD':{'macd':'macd','signal':'macd_signal','histogram':'macd_diff'},'MFIIndicator':'money_flow_index','MassIndex':'mass_index','NegativeVolumeIndexIndicator':'negative_volume_index',
 'OnBalanceVolumeIndicator':'on_balance_volume','PSARIndicator':'psar','PercentagePriceOscillator':'ppo','PercentageVolumeOscillator':'pvo','ROCIndicator':'roc','RSIIndicator':'rsi',
 'SMAIndicator':'sma_indicator','STCIndicator':'stc','StochRSIIndicator':'stochrsi','StochasticOscillator':{'k':'stoch','d':'stoch_signal'},'TRIXIndicator':'trix','TSIIndicator':'tsi',
 'UlcerIndex':'ulcer_index','UltimateOscillator':'ultimate_oscillator','VolumePriceTrendIndicator':'volume_price_trend','VolumeWeightedAveragePrice':'volume_weighted_average_price',
 'VortexIndicator':{'plus':'vortex_indicator_pos','minus':'vortex_indicator_neg'},'WMAIndicator':'wma','WilliamsRIndicator':'williams_r'}

def ta_expected(name,p,candles):
    cls=next(getattr(m,name) for m in [ta.trend,ta.momentum,ta.volume,ta.volatility,ta.others] if hasattr(m,name))
    sig=inspect.signature(cls); df=pd.DataFrame(candles);kw={k:df[k] for k in sig.parameters if k in df}
    candidates={'window':p.get('period',p.get('er_period',p.get('rsi_period',p.get('k_period',p.get('ema_period'))))), 'window_dev':p.get('multiplier'),'window_fast':p.get('fast',p.get('short',p.get('ema_period'))),'window_slow':p.get('slow',p.get('long',p.get('sum_period'))),'window_sign':p.get('signal'), 'window_atr':p.get('atr_period'),'multiplier':p.get('multiplier'),'pow1':p.get('fast'),'pow2':p.get('slow'),'nsig':p.get('signal'),'step':p.get('af_step'),'max_step':p.get('af_max'),'cycle':p.get('schaff_period'),'smooth_window':p.get('d_period'),'lbp':p.get('period')}
    if name=='AwesomeOscillatorIndicator':candidates.update(window1=p['fast'],window2=p['slow'])
    if name=='IchimokuIndicator':candidates.update(window1=p['tenkan_period'],window2=p['kijun_period'],window3=p['senkou_b_period'])
    if name=='UltimateOscillator':candidates.update(window1=p['short'],window2=p['mid'],window3=p['long'])
    if name=='KSTIndicator':candidates.update({**{f'roc{i}':p[f'roc{i}'] for i in range(1,5)},**{f'window{i}':p[f'sma{i}'] for i in range(1,5)}})
    kw.update({k:v for k,v in candidates.items() if v is not None and k in sig.parameters})
    obj=cls(**kw);methods=TA_METHODS[name]
    selected=[methods] if isinstance(methods,str) else list(methods.values())
    outputs={m:getattr(obj,m)().tolist() for m in selected}
    expected=[clean(outputs[methods][i] if isinstance(methods,str) else {k:outputs[m][i] for k,m in methods.items()}) for i in range(len(candles))]
    unused=[k for k,m in inspect.getmembers(cls,inspect.isfunction) if not k.startswith('_') and k not in selected]
    return expected,{k:v for k,v in kw.items() if not isinstance(v,pd.Series)},'additional source getters: '+','.join(unused) if unused else ''

records=[];datasets={};fixtures=[]
for case in ('mixed','flat','trend'):
    candles=bars(case)
    for row in mapping:
        if row['source'] not in ('ta','talipp'):continue
        source,name=row['source'],row['name'];record={'source':source,'name':name,'case':case,'operation_id':row['operation_id'],'parameters':row['parameters']}
        try:
            expected,params,note=(talipp_expected if source=='talipp' else ta_expected)(name,row['parameters'],candles)
            record.update(source_parameters=params,scope_note=note)
            values=candles if row['input_type']=='Candle' else [c['volume' if source=='ta' and name=='PercentageVolumeOscillator' else 'close'] for c in candles]
            if row['input_type']=='(f64, f64)':raise ValueError('pair input requires explicit source binding')
            request={'schema_version':1,'identity':{'series_id':'parity','instrument':'TEST','timeframe':'1ms','source':'independent-Python','data_version':'v1'},'operation':{'id':row['operation_id'],'params':row['parameters']},'as_of_ms':257,'samples':[{'at_ms':i+1,'available_at_ms':i+2,'value':v} for i,v in enumerate(values)]}
            bridge.stdin.write(json.dumps(request)+'\n');bridge.stdin.flush();actual=json.loads(bridge.stdout.readline())
            if 'error' in actual:record.update(status='local_error',error=actual['error'])
            else:
                assert len(actual['rows'])==len(expected)
                diffs=[{'index':i,'expected':e,'actual':r['value']} for i,(e,r) in enumerate(zip(expected,actual['rows'])) if not equal(e,r['value'])]
                record.update(status='mismatch' if diffs else 'pass_selected_outputs',differences=len(diffs),first_difference=diffs[:1])
            key=case+':'+row['input_type']+(':volume' if source=='ta' and name=='PercentageVolumeOscillator' else '');datasets[key]=request.pop('samples')
            fixtures.append({'source':source,'name':name,'case':case,'dataset':key,'request':request,'expected':expected})
        except Exception as e:record.update(status='oracle_or_binding_error',error=type(e).__name__+': '+str(e))
        records.append(record)
    print(case,{s:sum(r['case']==case and r['status']==s for r in records) for s in sorted({r['status'] for r in records})},flush=True)
bridge.stdin.close();bridge.wait(timeout=30)
(ROOT/'docs/evidence/python-parity.json').write_text(json.dumps({'schema_version':1,'tolerance':{'absolute':2e-9,'relative':2e-9},'records':records},indent=2)+'\n',encoding='utf8',newline='\n')
(ROOT/'crates/roze-ta/tests/fixtures/python-parity.json').write_text(json.dumps({'datasets':datasets,'fixtures':fixtures},separators=(',',':'),allow_nan=False)+'\n',encoding='utf8',newline='\n')
