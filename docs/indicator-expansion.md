# 指标扩展清单

版本：V0.2 需求草案；2026-09-03。

本清单是待实现及待验收的范围，需求起始原型为 23 类、35 个固定 Profile。
2026-09-03 A1 增量后统一目录为 33 类、45 个 Profile；这不表示 A 批全部验收。
下表“当前基础”保留需求基线；实施状态见 [状态页](implementation-status.md) 与机器任务表。
目标为 **60 类核心指标 + 12 类形态规则 + 12 类结构/多周期特征**，共 84 个候选能力条目。
概率和统计专项另见 [概率与统计需求](probability-statistics.md)，其中已有统计 ID 复用本清单，不重复计数。
机器可读任务清单：[indicator-backlog.csv](indicator-backlog.csv)。

## 批次和计数

- A 批：40 类核心指标；将现有原型纳入正式规格，同时补齐常用均线、量价和振荡指标。
- B 批：20 类核心指标；核心指标族总数达到 60。
- C 批：12 类形态、12 类结构/多周期特征，分别统计，不合并宣传为 84 种经典指标。
- 周期、阈值和市场不同只增加 Profile，不增加指标族计数。
- MACD 柱归入 MACD；布林 %B/带宽归入 Bollinger；NATR 归入 ATR；KDJ 的 J 归入 Stochastic 族。
- Yata TRIX、标准百分比 TRIX 分别提供 formula_variant，不偷偷替换已有实现。
- 目录中的“原型”仅代表有初始实现，仍需完成统一流式、恢复、参考值和单位验收。

## 60 类核心指标

### 均线与平滑

| ID | 指标族 | 批次 | 当前基础 |
| --- | --- | --- | --- |
| sma | SMA | A | 原型 |
| ema | EMA | A | 原型 |
| wma | WMA | A | 上游待封装 |
| rma | RMA | A | 上游待封装 |
| dema | DEMA | A | 上游待封装 |
| tema | TEMA | A | 上游待封装 |
| hma | HMA | A | 原型 |
| kama | KAMA | A | 原型 |
| vwma | VWMA | A | 上游待封装 |
| alma | ALMA | B | 待实现 |

### 趋势与方向

| ID | 指标族 | 批次 | 当前基础 |
| --- | --- | --- | --- |
| adx | ADX/DI | A | 原型 |
| aroon | Aroon | A | 原型 |
| sar | SAR | A | 原型 |
| ichimoku | Ichimoku | A | 原型 |
| supertrend | SuperTrend | A | 待实现 |
| vortex | Vortex | B | 待实现 |

### 动量与振荡

| ID | 指标族 | 批次 | 当前基础 |
| --- | --- | --- | --- |
| rsi | RSI | A | 原型 |
| macd | MACD | A | 原型 |
| stochastic | Stochastic/KDJ | A | 原型 |
| stoch_rsi | Stochastic RSI | A | 待实现 |
| cci | CCI | A | 原型 |
| cmo | CMO | A | 原型 |
| roc | ROC | A | 上游待封装 |
| momentum | Momentum | A | 上游待封装 |
| true_strength | True Strength Index | A | 上游待封装 |
| relative_vigor | Relative Vigor Index | A | 上游待封装 |
| ao | Awesome Oscillator | A | 原型 |
| ac | Accelerator Oscillator | B | 待实现 |
| ppo | PPO | B | 待实现 |
| trix | TRIX（公式变体分别标识） | A | 原型 |
| williams_r | Williams %R | A | 待实现 |
| ultimate | Ultimate Oscillator | B | 待实现 |
| kst | Know Sure Thing | B | 上游待封装 |
| fisher | Fisher Transform | B | 上游待封装 |

### 波动与通道

| ID | 指标族 | 批次 | 当前基础 |
| --- | --- | --- | --- |
| atr | TR/ATR/NATR | A | 原型 |
| bollinger | Bollinger Bands/%B/Bandwidth | A | 原型 |
| donchian | Donchian Channel | A | 原型 |
| keltner | Keltner Channel | A | 原型 |
| historical_vol | Historical Volatility | B | 待实现 |
| parkinson | Parkinson Volatility | B | 待实现 |
| garman_klass | Garman-Klass Volatility | B | 待实现 |
| ulcer | Ulcer Index | B | 待实现 |

### 量价

| ID | 指标族 | 批次 | 当前基础 |
| --- | --- | --- | --- |
| obv | OBV | A | 待实现 |
| ad | Accumulation/Distribution | A | 上游待封装 |
| cmf | CMF | A | 原型 |
| chaikin | Chaikin Oscillator | A | 原型 |
| mfi | MFI | A | 原型 |
| efi | Elder Force Index | A | 原型 |
| emv | Ease of Movement | B | 上游待封装 |
| kvo | Klinger Volume Oscillator | B | 上游待封装 |
| vwap | VWAP（会话/滚动/锚定） | A | 待实现 |
| rvol | Relative Volume | B | 待实现 |

### 滚动统计

| ID | 指标族 | 批次 | 当前基础 |
| --- | --- | --- | --- |
| stat_stddev | 标准差/方差 | A | 上游待封装 |
| stat_zscore | Z-score | A | 待实现 |
| stat_linreg | 线性回归/斜率 | B | 待封装及补齐 |
| stat_corr | 滚动相关 | B | 待实现 |
| stat_beta | 滚动 Beta | B | 待实现 |
| stat_percentile | 分位数/分位排名 | B | 待实现 |
| stat_skew | 偏度 | B | 待实现 |
| stat_kurt | 峰度 | B | 待实现 |

## 形态与结构

| ID | 规则/特征 | 分类 |
| --- | --- | --- |
| pattern_01 | 十字星 | 形态规则 |
| pattern_02 | 锤头 | 形态规则 |
| pattern_03 | 倒锤头 | 形态规则 |
| pattern_04 | 上吊线 | 形态规则 |
| pattern_05 | 射击之星 | 形态规则 |
| pattern_06 | 吞没 | 形态规则 |
| pattern_07 | 孕线 | 形态规则 |
| pattern_08 | 刺透 | 形态规则 |
| pattern_09 | 乌云盖顶 | 形态规则 |
| pattern_10 | 早晨之星 | 形态规则 |
| pattern_11 | 黄昏之星 | 形态规则 |
| pattern_12 | 三兵/三鸦 | 形态规则 |
| feature_01 | 已确认峰谷 | 结构与多周期 |
| feature_02 | Pivot Levels | 结构与多周期 |
| feature_03 | 支撑阻力区域 | 结构与多周期 |
| feature_04 | 突破识别 | 结构与多周期 |
| feature_05 | 假突破识别 | 结构与多周期 |
| feature_06 | 价格与指标背离 | 结构与多周期 |
| feature_07 | 趋势/震荡状态 | 结构与多周期 |
| feature_08 | 波动状态 | 结构与多周期 |
| feature_09 | 多周期方向一致性 | 结构与多周期 |
| feature_10 | 区间位置 | 结构与多周期 |
| feature_11 | ATR 标准化距离 | 结构与多周期 |
| feature_12 | 趋势持续时间 | 结构与多周期 |

形态阈值必须参数化并定义趋势背景、实体/影线比例和比较基准。确认时刻单独记录；
不能把需要后续 K 线确认的峰谷、背离或假突破提前写入历史结果。
多周期输出须列出参与周期及缺失/冲突状态。

## 专用数据依赖

- VWAP 必须说明价格源、volume 类型、会话边界、时区与锚定时刻；bar 近似不能冒充逐笔成交精确值。
- Historical/Parkinson/Garman-Klass 波动率分别注明公式假设、采样周期和年化因子，不固定套用交易日数量。
- 相关和 Beta 必须说明基准序列、对齐方式、收益定义和缺失值策略。
- 盘口价差、盘口不平衡、CVD、Volume Profile 列为条件扩展，不计入以上 84 项。
  它们分别需要报价/盘口、主动买卖分类成交或分价成交数据；只有 OHLCV 时不能伪造订单流。

## 扩展验收

每个 ID 均须完成：公式规格卡、参数范围、单位和多输出映射、逐项预热规则、批量/流式/恢复一致性、
独立参考值、边界测试、as-of 时间隔离和 MCP 一致性。零分母、零成交量和短序列应有明确状态。
复杂指标不能长期共用一个没有解释的 256 根预热值。

指标数达到目标但上述验收未完成，不算批次交付。上游 36 个指标模块仍需建立覆盖映射；
未纳入首批 60 类的模块保留候选清单和排除/延期理由，不要求为凑数开放未经审核的信号。
