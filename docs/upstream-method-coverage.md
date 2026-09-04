# 上游 Method 与指标模块覆盖

更新：全部 36 个原生指标模块和 44 个独立 Method 现已通过 `native_catalog` / `native_batch_calculate` 开放。
见 [完整原生覆盖清单](native-mcp-coverage.csv) 和 [原生 MCP 契约](usage/native-mcp.md)。
下文的延期状态描述固定 Profile、公式审查或业务特征的推进状态，不再表示原生算法无法通过 MCP 调用。

Yata 0.7.0 的 `indicators/mod.rs` 声明 36 个实际指标模块；`example` 为示例，不纳入该计数。
[36 项映射](upstream-indicator-coverage.csv) 每项均记录目标 ID、当前入口及延期/排除原因。
覆盖映射不是“36 项全部通过验收”：其中直接/外层映射 20 项，余项保留 A/B/C 或候选状态。
SMA/EMA/ATR 等基于 Method 的目录家族不增加上游 Indicator 模块数量。

| Method 模块/符号 | 当前映射或延期理由 |
| --- | --- |
| sma / SMA | sma 固定 Profiles；基础独立参考已有 |
| ema / EMA | ema 与多个平滑子算法；基础独立参考已有 |
| ema / DEMA、TEMA | A1 新增 dema.14、tema.14，首值种子变体明确 |
| ema / DMA、TMA | 多级平滑内部工具，不能把其别名或窗口计为额外目标家族 |
| wma / WMA | A1 wma.14；完整窗口后输出 |
| rma / RMA（MMA、SMMA 别名） | A1 rma.14；RSI/ATR 子算法已有；别名不另计数 |
| vwma / VWMA | A1 外层 14 根窗口重算同一公式，避免零权重减法残留；不是逐笔 VWAP |
| momentum / Momentum（Change、MTM 别名） | A1 momentum.14；不同于 Indicator MomentumIndex 的平滑信号 |
| rate_of_change / ROC | A1 roc.14；ratio 单位，非百分数 |
| adi / ADI | A1 ad.cumulative、原 Chaikin 外层累计；原 windowless 状态恢复缺陷保持独立记录 |
| tr / TR | ATR 封装的真实波幅子算法；独立 TR/NATR 输出 Profile 后续补齐 |
| hma / HMA | 现有 hma Indicator 内部调用，窗口舍入待正式审核 |
| tsi / TSI | true_strength A 批待封装，不能与 trend_strength_index 混同 |
| st_dev / StDev | Bollinger 内部使用；stat_stddev 流式 Profile 待实现，S1 描述方差不算完成此项 |
| lin_reg / LinearRegression | stat_linreg B 批待统一流式 Profile；S1 成对 OLS 为部分基础能力 |
| cci / CCI | 已通过原型 CCI Indicator 使用；缩放/零差值仍待逐项参考 |
| highest_lowest / 极值 | Donchian 等内部工具；未把每个 helper 当成额外指标 |
| highest_lowest_index | Aroon 等内部工具，极值相等时的时间选择待审核 |
| cross、reversal | 上游策略信号工具；形态/结构确认时间未验收前不单独暴露 |
| past、derivative、integral | 内部历史和变换工具；S1 提供部分显式 lag/diff，不重复计数 |
| mean_abs_dev、median_abs_dev | 统计偏差工具；S1 MAD 有独立样本规范，尚无统计 Stream Profile |
| volatility | 不是自动完成 historical_vol；必须另定收益/频率/年化规格 |
| wsma、smm、swma、conv、trima、vidya | 目标 60 外的平滑候选，保留原生重导出，未额外注册 |
| heikin_ashi、renko | 图表变换候选，需单独事件/合成 bar 时间契约 |
| collapse_timeframe | 多周期聚合候选，需市场日历/完整性/可知时间合同 |

`roze_ta::methods` 与 `roze_ta::indicators` 继续保留全部上游原生访问。
这些类型通过明确保留原生初始化、输出和时间语义的批量接口开放；其正式 Profile 与业务特征审核仍按原计划推进。
