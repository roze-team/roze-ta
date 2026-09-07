# 外部覆盖补充公式规格

需求 FR-IND-009 / AC-IND-009；实现 `reference_all/extras.rs`，变体版本 `roze-reference-extras-v1`。这是明示参数与种子的纯 Rust 数学实现，不复制 TTR 的 GPL 代码，不冒充各源库所有配置和缺失值策略的兼容层。

共同输入、时间、取消、快照和资源契约见 [Reference V1](reference-all-v1.md)。下表 n 对应参数 period，范围2..128；未列参数的项采用表内固定值。标量价/量由调用方按用途明确提供，不隐式从 Candle 改取其他字段。所有除零或实函数域外结果为 undefined_result/null，不回填上一结果。

## 逐项公式卡

| ID（省略 extra.） | 公式、输出与种子 | 首个结果 |
| --- | --- | --- |
| Acos、Asin、Atan | 反余弦、反正弦、反正切，弧度 | 1 |
| Cos、Sin、Tan | 弧度输入的三角函数 | 1 |
| Cosh、Sinh、Tanh | 双曲函数，无量纲 | 1 |
| Exp、Ln、Log10、Sqrt | exp(x)、自然对数、常用对数、非负平方根 | 1 |
| Ceil、Floor | 向上/下取整，保持原单位 | 1 |
| Add、Subtract、Multiply、Divide | 双序列 (x,y) 的 x+y、x-y、xy、x/y | 1 |
| Sum | 最近 n 项之和 | n |
| Minimum、Maximum | 最近 n 项最小/最大值 | n |
| MinIndex、MaxIndex | 对应极值在整个本次输入中的从0起索引；并列取最近者 | n |
| MinMax、MinMaxIndex | 分别输出 min/max 或 min_index/max_index | n |
| MeanDeviation | mean(abs(x-mean(x)))，未缩放均值绝对偏差 | n |
| ScaledMedianDeviation | 1.4826 × median(abs(x-median(x)))；偶数中位数取两中值平均 | n |
| SampleVariance、SampleStdDev | sum((x-mean(x))²)/(n-1)，及其平方根 | n |
| SampleCovariance | sum((x-mean(x))(y-mean(y)))/(n-1) | n |
| RelativeVolume | 当前量 / 含当前项的 n 项均量；输入标量须表示量 | n |
| EfficiencyRatio | abs(x[t]-x[t-n])/sum(abs(diff(x)))，n 个真实差分 | n+1 |
| PositiveCmo | 100×sum(max(diff,0))/sum(abs(diff))；独立的正向占比，不标为 CMOU | n+1 |
| SmoothedCmo | 100×(Wilder(gain)-Wilder(loss))/(Wilder(gain)+Wilder(loss))；前 n 个差分均值种子，后续权重1/n | n+1 |
| WilderSum | 初始 n 项之和，其后 S←S(1-1/n)+x | n |
| Lag、Lags | x[t-n]；或按 lag=1..n 顺序返回所有滞后值，不向前填充 | n+1 |
| VariableSma | 输入 (value,period_t)，以 period_t 个最新真实样本求 SMA；整数2..n，超范围拒绝，不隐式裁剪 | period_t |
| SingleFactorModel | 输入(资产超额收益,基准超额收益)；β=cov/var(基准)、α=mean(资产)-βmean(基准)、R²=cov²/(var资产var基准) | n |
| CumulativeSum | 从0累计求和 | 1 |
| CumulativeReturn | 100×(x[t]/x[0]-1)，首项为0（首价非零时） | 1 |
| InternalBarStrength | (close-low)/(high-low)，0..1 | 1 |
| CloseLocationValue | (2close-high-low)/(high-low)，-1..1 | 1 |
| AverageBarRange | mean(high-low,n)，不聚合交易日；区别于 Wickra AverageDailyRange 的会话算法 | n |
| SignalNoiseRatio | abs(close[t]-close[t-n])/ATR(n)；ATR 首根TR=high-low，之后用前收盘，Wilder均值种子 | n+1 |
| SmoothedObv | SMA(OBV,n)；OBV首根种子0，后续按价格方向加减该根量，相等不变 | n |
| Sfx | 输出 atr、std_dev、ma_std_dev；分别为 ATR(n)、close 的总体 SD(n)、SMA(SD,n)，三个周期在本变体中相同 | 2n-1 |
| Guppy | EMA周期顺序3,5,8,10,12,15,30,35,40,45,50,60；每条使用其完整周期 SMA 种子 | 60 |
| Growth | 输入(价格,信号)，G[0]=1；G[t]=G[t-1]×(1+signal[t-1]×(price[t]/price[t-1]-1))；信号使用前一时点，不作实际订单或费用模拟 | 1 |
| TrendDetectionIndex | m[t]=x[t]-x[t-n]；di=sum(m,n)；tdi=abs(di)-[sum(abs(m),2n)-sum(abs(m),n)]，multiple固定2 | 3n |
| PriceBands | a=SMA(price,n)，b=SMA(price,2)，s=总体SD(a-b,n)；lower=a-2s、center=a、upper=a+2s；centered=false、lavg=false | 2n-1 |
| Kdj | 9根 RSV=100(close-min(low))/range；K=(2Kprev+RSV)/3，D=(2Dprev+K)/3，J=3K-2D；K/D初始50 | 9 |
| SlowStochastic | 最近 n 根原始%K，再SMA(3)得慢K，再SMA(3)得慢D；输出k/d为0..100 | n+4 |
| PercentageVolumeOscillator | 输入非负标量量；PVO=100(EMA12-EMA26)/EMA26，signal=EMA9(PVO)，histogram=PVO-signal；完整均值种子 | 34 |
| RelativeVolatilityIndex | close总体SD固定10；上涨/下跌时分别向up/down输入SD，否则输入0；分别Wilder平滑n后输出100up/(up+down) | n+9 |
| Dvi | 固定原始DVI窗口，详见下节；输出 magnitude/stretch/dvi | 357 |
| PivotsHL | 固定14根高/低查找窗口；首次以低点建立临时状态，突破高低切换/更新；输出带 provisional=true 的修订事件，绝不回填已发出的行 | 1，后续事件驱动 |
| BlackCrows、WhiteSoldiers | 对 ThreeSoldiersOrCrows 的负/正方向筛选，其余为0 | 原始形态预热 |
| DarkCloudCover、Piercing | 对 PiercingDarkCloud 的负/正方向筛选 | 原始形态预热 |
| EveningStar、MorningStar | 对 MorningEveningStar 的负/正方向筛选 | 原始形态预热 |
| RiseFallThreeMethods | 合并 RisingThreeMethods 与 FallingThreeMethods 的方向输出 | 原始形态预热 |
| SideGapThreeMethods | 合并 UpsideGapThreeMethods 与 DownsideGapThreeMethods 的方向输出 | 原始形态预热 |

方向筛选和合并复用已迁入的形态状态机，幅度保留 -1/0/+1 变体，不冒充 TA-Lib 的±100及其 candle-settings。PivotsHL 的临时极值不是已经过未来反向确认的 Swing，调用方必须读取 provisional 标记。

## DVI 固定变体

r=price/SMA(price,3)-1；m=SMA((SMA(r,5)+SMA(r,100)/10)/2,5)。
b=1（本根价>前价），否则-1；s=SMA((SUM(b,10)+SUM(b,100)/10)/2,2)。
分别对 m、s 使用252个有效历史值作包含当前样本的百分位排名；并列按 <= 全计入。
输出 magnitude=rank(m)、stretch=rank(s)、dvi=.8×magnitude+.2×stretch；禁止用预热零值参与排名。

## 成本与独立验证

这些补充项保存至多4096条已经校验的数值历史，供审计和独立重算；状态规模 O(T+n)，候选副本也在相同上限内。初等运算本身 O(1)；滚动统计 O(n)，中位数 O(n log n)，回归 O(n)，滞后向量 O(n)，Guppy固定12条状态机。Wilder参考重算与DVI按历史重放，分别 O(T)、O(T×100)，不宣称它们为O(1)更新。实际 wire 更新另含候选状态复制成本。

`tests/reference_all.rs` 使用闭式函数值、手算滚动窗口、精确直线回归、Wilder和ATR样本、常量Guppy/DVI等独立参考，另对全部调用项执行恢复、重置与输入契约测试。所有补充项走与原生迁入项相同的批量/流式/MCP路径。

数学参考：[TA-Lib 函数定义](https://github.com/TA-Lib/ta-lib/blob/main/ta_func_api.xml)、[talipp 指标说明](https://nardew.github.io/talipp/2.5.0/)、[TTR SNR](https://search.r-project.org/CRAN/refmans/TTR/html/SNR.html)、[TTR TDI](https://search.r-project.org/CRAN/refmans/TTR/html/TDI.html)、[TTR DVI](https://search.r-project.org/CRAN/refmans/TTR/html/DVI.html)、[TTR PBands](https://search.r-project.org/CRAN/refmans/TTR/html/priceBands.html)、[TTR rolling](https://search.r-project.org/CRAN/refmans/TTR/html/runFun.html)。源库的参数组合超出本卡列出的变体时，不据此宣称兼容。
