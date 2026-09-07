# 来源兼容变体 v1

对应 FR-IND-009 / AC-IND-009，版本 `source-parity-20260907-v1`。
新增 114 个 `compat.*` 入口，用独立来源变体处理原 106 项差异、8 项参考异常。
原 Wickra、TA-Lib 和 extra 入口保留各自公式；最终绑定见
[逐项映射](../reference-indicator-mapping.json)。原始失败记录保存在
[基线](../evidence/cross-library-parity-baseline.json)，当前结果见
[验收](../evidence/source-compat-parity.json)。

## 数值与参数

参数必须完整、显式提供，名称、范围和示例以 `reference_catalog` 为准。
来源未开放的参数采用本卡固定设置，不能推断为来源整个 API 的替代品。
逐项规格见 [114 项规格表](source-parity-cards.md)。其中每一行绑定输入、参数、实现和独立参考证据。

EMA 区分 pandas 首值种子和 SMA 种子；Wilder 平滑系数为 `1/n`。
talipp STC 保留 EMA 的加权乘加顺序、SMA 的递推顺序，以及窗口内忽略未形成的子指标值的规则。
TTR DVI 的百分位按源规则将绝对差小于 `1e-8` 的值计入同值权重；这是指标定义中的同值判断，并非验收浮点容差。
TTR 的运行和按递推顺序计算。TTR MACD 默认按百分比输出，talipp TRIX 比例因子为 10000，ta/TTR TRIX 为 100。
复合指标保留独立参考中的字段和各字段缺值，不把部分输出伪装成全部就绪。

固定选项包括：FibonacciRetracement 系数 0.618（输出价格乘 1.618）；
PivotsHL 高低周期均为 14；ZigZag/talipp 最短趋势为 1；
StochRSI 的 K/D 平滑周期均为 3；STC 平滑周期为 3；
TTR SMI 信号周期为 9；TRIX 信号周期为 9；TTR VWAP 滚动周期为 10；
TTR CTI 使用秩相关，BBands 使用选定的标量价格，volatility 使用 close 方法；
DVI、GMMA、PBands 的其余设置采用固定来源默认配置。

## 时间与缺值

全部入口只读取当前已接收、已可见的输入，最多 4096 条，使用有界历史重新计算。
这不是常数时间更新承诺；长窗口组合可能需要多轮窗口遍历。
行时间始终是发出时间。输出空值或含空字段时为 `undefined_result`；
来源早期的零值是有效源输出。空值不表示可用于交易的成熟信号。
事件输出可能保留最近一次事件，不能仅凭非空判断发生了新事件。

三个来源具有历史回填或前视批次行为，采用以下明确的实时契约：

| 入口 | 实时约定 | 独立验算 |
|---|---|---|
| compat.ta.KSTIndicator | 每次对当前前缀计算，早期 ROC 缺值的均值只来自已见数据；不追改旧行 | 对每个前缀调用 pinned ta，取最后一行 |
| compat.ttr.DPO | 延迟 `floor(n/2)+1` 个观测发出源位置的值；例如 n=20 延迟 11 条 | 独立 TTR 批次结果按该延迟对齐，未获得未来数据的尾部不提前发出 |
| compat.ttr.ZigZag | 当前前缀最后一行的临时值；最新位置不是极值时可能为空，不回填历史折线 | 每个至少两条数据的前缀调用 pinned TTR，取最后一行；单条输入为空 |

这三项状态为 `verified_causal_cases`，不声称与来源完整历史图表逐时间点相同。
独立对齐值保存在 `tests/fixtures/causal-parity.json`，原完整批次参考文件未覆盖。

## 参考异常

talipp 的 CCI、MassIndex、VTX、VWMA，在原平盘零成交量用例中出现除零；
TTR 的 ADX、EMV、SMI、stoch 在该用例中无法形成后续平滑输入。
本项目返回结构化 `ErrorCode::UndefinedResult`，整个失败更新不改变状态。
这属于明确的错误归一化，不复制 Python/R 异常类型。

## 验收与再现

三个数据集为 mixed、flat、trend。114 项合计 334 个正常数值案例和 8 个参考异常案例；
每条输出和全部字段均比较，绝对加相对容差 `2e-9 * (1 + abs(expected))`。
不跳过预热位置，不只比较最后一个点，不用本地输出生成期望值。
来源提交及逐文件 Git blob 校验见 `cross-library-sources.json`。

执行 `scripts/generate-causal-parity.py` 可重建独立时间对齐参考；
`scripts/check-compat-progress.py` 对全部 342 个案例运行桥接检查；
`scripts/record-compat-acceptance.py` 只接受零失败结果并记录参考文件 SHA-256。
Cargo 回归 `source_compat_parity` 覆盖全部数值、8 类错误事务、真实时间戳和恢复；
`reference_all` 覆盖所有目录入口的构造、恢复、重置和输入边界。

上游原始文件和派生基线不因此修改。来源许可见核心模块的 `SOURCE-NOTICES.md`。
回滚时删除 `compat.*` 绑定和新增模块即可恢复原来源映射，原始失败基线可用于复核。
