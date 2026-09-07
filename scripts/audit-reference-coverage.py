"""Build the reviewable source-export to local-formula mapping (no completion flags)."""
import json
import re
from pathlib import Path

ROOT=Path(__file__).resolve().parents[1]
def read(path): return json.loads((ROOT/path).read_text(encoding="utf-8"))
sources=read("docs/reference-indicator-sources.json")
inventory=read("crates/roze-ta/src/reference_all/inventory.json")
def norm(s): return re.sub(r"[^a-z0-9]", "", s.lower())
lookup={norm(r["name"]):r["id"] for r in inventory}
extra_names=["Acos","Asin","Atan","Ceil","Cos","Cosh","Exp","Floor","Ln","Log10","Sin","Sinh","Sqrt","Tan","Tanh","Add","Subtract","Multiply","Divide","Sum","Minimum","Maximum","MinIndex","MaxIndex","MinMax","MinMaxIndex","MeanDeviation","RelativeVolume","EfficiencyRatio","PositiveCmo","WilderSum","Lag","Lags","SingleFactorModel","VariableSma","SignalNoiseRatio","TrendDetectionIndex","SmoothedObv","Sfx","PriceBands","CumulativeSum","CumulativeReturn","InternalBarStrength","CloseLocationValue","Guppy","Growth","Kdj","Dvi","PivotsHL"]
lookup.update({norm(n):"extra."+n for n in extra_names})
aliases={
"AC":"AcceleratorOscillator","ACCBANDS":"AccelerationBands","AD":"Adl","AccuDist":"Adl","AccDistIndex":"Adl","ADOSC":"ChaikinOscillator","AO":"AwesomeOscillator","BB":"BollingerBands","BBANDS":"BollingerBands","BOP":"BalanceOfPower","CHOP":"ChoppinessIndex","ChaikinOsc":"ChaikinOscillator","CoppockCurve":"Coppock","DonchianChannels":"Donchian","DonchianChannel":"Donchian","EMV":"EaseOfMovement","FibonacciRetracement":"FibRetracement","IBS":"InternalBarStrength","KeltnerChannels":"Keltner","KeltnerChannel":"Keltner","MACD":"MacdIndicator","MassI":"MassIndex","MeanDev":"MeanDeviation","ParabolicSAR":"Psar","RogersSatchell":"RogersSatchellVolatility","SOBV":"SmoothedObv","Stoch":"Stochastic","TTM":"TtmSqueeze","UO":"UltimateOscillator","VTX":"Vortex","Williams":"WilliamsR","AverageTrueRange":"Atr","DailyLogReturn":"LogReturn","DailyReturn":"Roc","OnBalanceVolume":"Obv","NegativeVolumeIndex":"Nvi","PercentagePriceOscillator":"Ppo","PercentageVolumeOscillator":"VolumeOscillator","VolumeWeightedAveragePrice":"RollingVwap",
"AVGDEV":"MeanDeviation","AVGPRICE":"AvgPrice","AROONOSC":"AroonOscillator","CMF":"ChaikinMoneyFlow","COPPOCK":"Coppock","CORREL":"PearsonCorrelation","CUMSUM":"CumulativeSum","DIV":"Divide","DONCHIAN":"Donchian","EFI":"ForceIndex","ER":"EfficiencyRatio","ERI":"ElderRay","FRACTAL":"WilliamsFractals","HA":"HeikinAshi","HT_DCPERIOD":"HilbertDominantCycle","HT_DCPHASE":"HtDcPhase","HT_PHASOR":"HtPhasor","HT_SINE":"SineWave","HT_TRENDLINE":"InstantaneousTrendline","HT_TRENDMODE":"HtTrendMode","IMI":"IntradayMomentumIndex","KC":"Keltner","LINEARREG":"LinearRegression","LINEARREG_ANGLE":"LinRegAngle","LINEARREG_INTERCEPT":"LinRegIntercept","LINEARREG_SLOPE":"LinRegSlope","MA":"Sma","MACDEXT":"MacdExt","MACDFIX":"MacdFix","MARKETFI":"MarketFacilitationIndex","MAVP":"VariableSma","MAX":"Maximum","MAXINDEX":"MaxIndex","MEDPRICE":"MedianPrice","MIN":"Minimum","MININDEX":"MinIndex","MINMAX":"MinMax","MINMAXINDEX":"MinMaxIndex","MINUS_DI":"MinusDi","MINUS_DM":"MinusDm","MULT":"Multiply","PERCENTILE":"RollingQuantile","PERCENTRANK":"RollingPercentileRank","PLUS_DI":"PlusDi","PLUS_DM":"PlusDm","PVO":"VolumeOscillator","PVT":"VolumePriceTrend","RMA":"Smma","RVOL":"RelativeVolume","SAR":"Psar","SAREXT":"SarExt","STDDEV":"StdDev","STOCHF":"Stochastic","SUB":"Subtract","TRANGE":"TrueRange","TYPPRICE":"TypicalPrice","ULTOSC":"UltimateOscillator","VAR":"Variance","VHF":"VerticalHorizontalFilter","WCLPRICE":"WeightedClose","WILLR":"WilliamsR",
"CLV":"CloseLocationValue","CTI":"CorrelationTrendIndicator","GMMA":"Guppy","PBands":"PriceBands","SNR":"SignalNoiseRatio","TDI":"TrendDetectionIndex","TR":"TrueRange","WPR":"WilliamsR","chaikinAD":"Adl","keltnerChannels":"Keltner","momentum":"Mom","rollSFM":"SingleFactorModel","runCor":"RollingCorrelation","runCov":"RollingCovariance","runMAD":"MedianAbsoluteDeviation","runMax":"Maximum","runMean":"Sma","runMedian":"MedianMa","runMin":"Minimum","runPercentRank":"RollingPercentileRank","runRange":"MinMax","runSD":"StdDev","runSum":"Sum","runVar":"Variance","ultimateOscillator":"UltimateOscillator","volatility":"HistoricalVolatility","williamsAD":"Wad",
}
candles={
"2CROWS":"TwoCrows","3BLACKCROWS":"ThreeSoldiersOrCrows","3INSIDE":"ThreeInside","3LINESTRIKE":"ThreeLineStrike","3OUTSIDE":"ThreeOutside","3STARSINSOUTH":"ThreeStarsInSouth","3WHITESOLDIERS":"ThreeSoldiersOrCrows","ABANDONEDBABY":"AbandonedBaby","ADVANCEBLOCK":"AdvanceBlock","BELTHOLD":"BeltHold","BREAKAWAY":"Breakaway","CLOSINGMARUBOZU":"ClosingMarubozu","CONCEALBABYSWALL":"ConcealingBabySwallow","COUNTERATTACK":"Counterattack","DARKCLOUDCOVER":"PiercingDarkCloud","DOJI":"Doji","DOJISTAR":"DojiStar","DRAGONFLYDOJI":"DragonflyDoji","ENGULFING":"Engulfing","EVENINGDOJISTAR":"EveningDojiStar","EVENINGSTAR":"MorningEveningStar","GAPSIDESIDEWHITE":"GapSideBySideWhite","GRAVESTONEDOJI":"GravestoneDoji","HAMMER":"Hammer","HANGINGMAN":"HangingMan","HARAMI":"Harami","HARAMICROSS":"HaramiCross","HIGHWAVE":"HighWave","HIKKAKE":"Hikkake","HIKKAKEMOD":"HikkakeModified","HOMINGPIGEON":"HomingPigeon","IDENTICAL3CROWS":"IdenticalThreeCrows","INNECK":"InNeck","INVERTEDHAMMER":"InvertedHammer","KICKING":"Kicking","KICKINGBYLENGTH":"KickingByLength","LADDERBOTTOM":"LadderBottom","LONGLEGGEDDOJI":"LongLeggedDoji","LONGLINE":"LongLine","MARUBOZU":"Marubozu","MATCHINGLOW":"MatchingLow","MATHOLD":"MatHold","MORNINGDOJISTAR":"MorningDojiStar","MORNINGSTAR":"MorningEveningStar","ONNECK":"OnNeck","PIERCING":"PiercingDarkCloud","RICKSHAWMAN":"RickshawMan","RISEFALL3METHODS":"RisingThreeMethods","SEPARATINGLINES":"SeparatingLines","SHOOTINGSTAR":"ShootingStar","SHORTLINE":"ShortLine","SPINNINGTOP":"SpinningTop","STALLEDPATTERN":"StalledPattern","STICKSANDWICH":"StickSandwich","TAKURI":"Takuri","TASUKIGAP":"TasukiGap","THRUSTING":"Thrusting","TRISTAR":"Tristar","UNIQUE3RIVER":"UniqueThreeRiver","UPSIDEGAP2CROWS":"UpsideGapTwoCrows","XSIDEGAP3METHODS":"UpsideGapThreeMethods",
}
aliases.update({"CDL"+k:v for k,v in candles.items()})
aliases.update({"StochasticOscillator":"Stochastic", "ADR":"AverageBarRange", "CMOU":"Cmo", "CVI":"ChaikinVolatility", "FOSC":"TsfOscillator", "PVO":"PercentageVolumeOscillator", "PercentageVolumeOscillator":"PercentageVolumeOscillator"})
lookup.update({norm(n):"extra."+n for n in ["AverageBarRange","SmoothedCmo","PercentageVolumeOscillator","RelativeVolatilityIndex"]})
for export,name in {"CDL3BLACKCROWS":"BlackCrows","CDL3WHITESOLDIERS":"WhiteSoldiers","CDLDARKCLOUDCOVER":"DarkCloudCover","CDLPIERCING":"Piercing","CDLEVENINGSTAR":"EveningStar","CDLMORNINGSTAR":"MorningStar","CDLRISEFALL3METHODS":"RiseFallThreeMethods","CDLXSIDEGAP3METHODS":"SideGapThreeMethods"}.items():
    aliases[export]=name
    lookup[norm(name)]="extra."+name
lookup[norm("SlowStochastic")]="extra.SlowStochastic"
for export,name in {"runVar":"SampleVariance","runSD":"SampleStdDev","runCov":"SampleCovariance","runMAD":"ScaledMedianDeviation"}.items():
    aliases[export]=name
    lookup[norm(name)]="extra."+name
aliases={norm(k):v for k,v in aliases.items()}
non_indicators={"getYahooData":"network data acquisition", "stockSymbols":"network symbol discovery", "naCheck":"missing-data validation helper", "adjRatios":"corporate-action adjustment factors, a preprocessing contract rather than an indicator"}
rows=[]
for source in sources["sources"]:
    for name in source["entries"]:
        key=norm(name)
        # Exact names in Wickra must never be overridden by other libraries' aliases.
        if source["id"]=="wickra": candidate=lookup.get(key)
        else:
            key=key.removesuffix("indicator")
            candidate=lookup.get(norm(aliases[key])) if key in aliases else lookup.get(key)
        if source["id"]=="ta-lib" and name=="RVI": candidate="extra.RelativeVolatilityIndex"
        if source["id"]=="ta-lib" and name=="CMO": candidate="extra.SmoothedCmo"
        if source["id"]=="ta-lib" and name=="STOCH": candidate="extra.SlowStochastic"
        rows.append({"source":source["id"],"name":name,"candidate":candidate,"classification":"non_indicator" if source["id"]=="ttr" and name in non_indicators else "formula_review","reason":non_indicators.get(name)})
(ROOT/"docs/evidence/reference-coverage-audit.json").write_text(json.dumps(rows,indent=2,ensure_ascii=False)+"\n",encoding="utf-8",newline="\n")
missing=[(r["source"],r["name"]) for r in rows if not r["candidate"] and r["classification"]!="non_indicator"]
print("Unmapped",len(missing),missing)
