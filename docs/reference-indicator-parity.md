# 外部指标逐项覆盖需求

日期：2026-09-07；状态：目标需求，实施中。

## 范围与来源

FR-IND-009：用户要求其他指标库已有的指标在 roze-ta 中逐项实现。首轮核对前述 TA-Lib、talipp、ta、TTR、Wickra；后续 awesome-quant 指标库发现的新能力继续登记。
原 60 类指标、12 类形态与 12 类结构特征作为原有批次保留，不再作为覆盖上限。

[来源清单](reference-indicator-sources.json) 固定读取日期、文件路径与 Git blob SHA，初次记录 886 个源条目：Wickra 514、talipp 60、ta 43、TA-Lib 201、TTR 68。
Wickra 按 FAMILIES 的公开指标计数，不按 513 个文件计数；ta 从五个模块的类定义枚举，不能仅依赖 wrapper（其遗漏 WMA）。
源条目包含同义名、多输出拆分、公式变体及工具函数，不代表 886 个独立待新增指标。
SHA 是文件内容身份，不冒充整个仓库的提交。来源清单只摘录名称与分类，没有复制算法源码。

## 逐项验收

AC-IND-009：每个源条目必须有最终映射或明确的非指标分类。已有同名实现仍需核对种子、公式、周期、单位、输出顺序和时间语义；不得直接标为兼容。
同一公式复用本地算法；有实质差异的公式以独立 formula_variant 表示。需要订单簿、逐笔、横截面、会话或衍生品输入的指标须定义对应数据契约，不能用 OHLCV 伪造。

交付必须包含规格卡、纯 Rust 实现、独立参考值、边界测试、目录注册、MCP 调用和文档证据。
流式能力同时验证重置、批量一致性、分段恢复、旧快照兼容和 as-of 时间隔离。
数学未定义以结构化状态表示；源库填零或填中点的行为必须记录为变体差异。
参数、输入和状态占用须有上限，复杂度逐算法说明，不能沿用源库笼统的 O(1) 宣传。

## 实施跟踪

[逐项映射](reference-indicator-mapping.json) 为所有源条目保留状态：

- `review_pending`：尚未完成公式与本地实现核对；不能推断为缺失或完成。
- `implemented_variant`：已有固定来源的迁移实现、登记入口与证据，但不代表其他库默认行为兼容。
- `mapped_variant`：有可调用的本地公式候选；尚未完成逐项跨库数值兼容验收，不能计为完整AC-IND-009通过。
- `verified_default_cases`：固定TA-Lib默认参数三组独立C参考值通过；不代表所有参数组合兼容。
- `non_indicator`：数据下载、校验等非指标辅助功能，记录理由。

第一批 R1 为 ALMA、SuperTrend、StochRSI、Vortex、Ulcer Index；规格见[公式契约](contracts/reference-r1.md)。
后续已迁入全部514个Wickra导出算法、201个官方TA-Lib Rust函数，补充66个运算，新增114个来源兼容变体，开放895个Reference入口。
886条来源映射现为514个implemented_variant、201个verified_default_cases、167个mapped_variant、4个non_indicator。
368个跨库候选均已执行三组输入对照：[完整审计](evidence/cross-library-parity-audit.json)。其中201项TA-Lib默认配置通过，53项其他库既有选定配置通过，111项来源变体与3项实时对齐变体通过。
原106项差异与8项异常已由114个明确变体闭合，共334组数值和8组异常事务用例通过。全部验收均有参数、输出、时间范围，不能推断整个来源API兼容；另53项保留mapped_variant并附选定用例验收证据。详见[来源契约](contracts/source-parity-v1.md)。
早期证据见[上一轮记录](evidence/2026-09-07-reference-full.md)，新过程见[跨库数值验收记录](evidence/2026-09-07-cross-library-parity.md)。

## 来源入口

- [awesome-quant](https://github.com/wilsonfreitas/awesome-quant)
- [TA-Lib](https://github.com/TA-Lib/ta-lib)
- [talipp](https://github.com/nardew/talipp)
- [ta](https://github.com/bukosabino/ta)
- [TTR](https://github.com/joshuaulrich/TTR)
- [Wickra](https://github.com/wickra-lib/wickra)

来源变体状态：`verified_source_cases` 表示显式配置的三组来源用例通过；`verified_causal_cases` 表示同时具有明确实时对齐约定，不能替换完整历史回填图表。
