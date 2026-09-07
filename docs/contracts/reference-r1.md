# R1 外部指标覆盖规格卡

需求：FR-IND-009、AC-IND-009，关联原 alma / supertrend / stoch_rsi / vortex / ulcer 扩展 ID。
统一引擎输入使用已收盘、按时间递增的正价格 OHLCV；finite 且绝对值不超过 1e100，volume 非负。
统一引擎参数为固定 Profile，未开放任意参数；迁移的低层 `roze_ta::wickra` 构造器可指定参数，周期上限4096。核心不引入外部依赖。算法版本 `reference-r1-v1`。

| Profile | 参数 | 输出与单位 | 最少 bar | 状态规模 / 每次更新 |
| --- | --- | --- | --- | --- |
| alma.9 | n=9、offset=.85、sigma=6、中心不取整 | alma：price | 9 | O(n) / O(n) |
| supertrend.10_3 | ATR n=10、倍数3、首个就绪方向+1 | supertrend：price；direction：±1 | 10 | O(1) / O(1) |
| stoch_rsi.14_14 | RSI n=14、随机窗口14、不额外平滑 | stoch_rsi：0..100 | 28 | O(n) / O(n) |
| vortex.14 | 14 个相邻 bar 差分 | vi_plus、vi_minus：非负无量纲比率 | 15 | O(n) / O(n) |
| ulcer.14 | trailing-max14 后接 RMS14 | ulcer：百分数 | 27 | O(n) / O(n) |

## ALMA

窗口从旧到新编号 i=0..n-1，m=.85(n-1)，s=n/6，w_i=exp(-(i-m)^2/(2s²))。
输出 sum(w_i close_i)/sum(w_i)，只使用真实9根收盘价，不填充历史；常量序列返回常量。

## SuperTrend

首根 TR=high-low；其后 TR=max(high-low,abs(high-prev_close),abs(low-prev_close))。
前10个 TR 的算术均值初始化 ATR，其后 ATR=(9*prev_ATR+TR)/10。
BU=(high+low)/2+3ATR，BL=(high+low)/2-3ATR。
FU 在 BU<prev_FU 或 prev_close>prev_FU 时取 BU，否则保持 prev_FU；FL 在 BL>prev_FL 或 prev_close<prev_FL 时取 BL，否则保持 prev_FL。
首个就绪方向+1；前方向-1时 close>FU 才翻为+1；前方向+1时 close<FL 才翻为-1。等于边界不翻转。
方向+1输出 FL，方向-1输出 FU。零 TR 合法；不将负的理论下轨强制截断为零。

## StochRSI

先积累14个真实相邻收盘差分，gain=max(delta,0)、loss=max(-delta,0)；算术均值初始化后使用 Wilder 1/14 递推。
RSI=100*avg_gain/(avg_gain+avg_loss)，双零为未定义；纯上涨为100，纯下跌为0。
就绪后对最近14个 RSI 求 min/max，输出100*(current-min)/(max-min)。任何窗口成员未定义或 max=min 时为 undefined_result。
首个 RSI 在第15根，首个 StochRSI 在第28根。未定义样本消费并保留在窗口中，过期后可恢复。

## Vortex

每个相邻 bar 对：VM+=abs(high-prev_low)，VM-=abs(low-prev_high)，TR 使用含前收盘的真波幅。
最近14对分别求和，VI+=sum(VM+)/sum(TR)，VI-=sum(VM-)/sum(TR)。sum(TR)=0 时为 undefined_result。
使用有界窗口重算，避免长期累加/扣减残差让全零窗口误报就绪。

## Ulcer Index

第14根起计算 D_t=100*(close_t/max(close[t-13..t])-1)，第27根起输出 sqrt(mean(D²[t-13..t]))。
每个 D 使用它自己的历史高点窗口，不以当前窗口最高点回算全部历史回撤。正价格常量和单调上涨输出0。

## 来源、变体与验收

公式审查参考 Wickra 的 alma、super_trend、stoch_rsi、vortex、ulcer_index 源文件，文件 blob SHA 见[来源证据](../evidence/reference-r1-source.json)。
本次直接迁入 Wickra 的五个算法及 ATR、RSI、RollingSum、OHLCV 辅助源码，按 MIT 保留作者版权；Yata 原始与派生代码不变。
低层模块保留 Wickra 的 flat StochRSI=50、flat Vortex=0，统一引擎以附加状态及分母检查映射为 undefined_result，不宣称逐位兼容。
低层 RSI 的双零中点不作为有效 StochRSI 成员；适配器记录最近14个 RSI 的有效标记，使未定义样本在过期前持续阻止输出。
迁移记录和升级影响见[来源补丁说明](../patches/wickra-r1-migration.md)。
ALMA 实际需遍历权重窗口，不能声明为不随周期变化的 O(1)。

验收以手工样本及独立历史切片公式为参考，容差 abs_error<=1e-10*(1+abs(expected))。
同一实现的批量/流式/恢复须精确一致；检查短序列、常量、单调、交替、窗口过期、零量、无效输入、未来可知时间及旧45个 Profile 快照。
新增 Kernel 变体只追加，保持旧枚举序号和参数不变。MCP 通过现有目录、批量及流式入口调用核心。
