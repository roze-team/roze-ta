# Reference API V1：外部算法变体

对应 FR-IND-009 / AC-IND-009。核心入口为 `roze_ta::reference_all`，迁入的低层类型为 `roze_ta::wickra_all` 和 `roze_ta::talib`；原 50 个严格 Profile、R1 变体及旧快照保持原有版本。

## 目录与参数

`reference_catalog` 列出显式构造参数、输入类型、输出类型、公式说明和源文件。514 个 Wickra 导出名来自固定提交的 FAMILIES；不把辅助输出结构或不同周期重复计数。补充运算与组合见 [extras 规格](reference-extras.md)。
参数必须按目录完整提供，不接受未知字段，不静默截断整数；`example_params` 是经构造测试的示例，不冒充源库默认值。Wickra 的 usize 参数限制 0..512，u32 限制 0..10080，i32 限制 -1440..1440，浮点参数限制绝对值 1e6；随后还要通过各构造器的关系与取值校验。

`wickra.*` 公式以 `wickra-7ed1504-v1` 命名，直接使用保留的 Rust 算法。目录 `documentation` 是各类型的公式规格卡；完整算法、初始化、warmup_period、reset、独立样本和边界测试位于目录指向的源文件及对应维护副本。这不是 TA-Lib / talipp / ta / TTR 的默认参数或逐位兼容声明。

另有201个`talib.*`入口，变体为`ta-lib-rust-2f0426d-v1`，参数、预热、独立C验收范围见[TA-Lib契约](talib-reference-v1.md)。新增114个`compat.*`入口，见[来源契约](source-parity-v1.md)。目录合计895项；TA-Lib结果使用`talib_formula_variant`标志，保留源默认中性值约定。

## 输入契约

所有输入外包 `Sample { at_ms, available_at_ms, value }`。时间为 UTC 毫秒；at_ms 严格增加，available_at_ms 不倒退，必须满足 `0 < at_ms <= available_at_ms <= as_of_ms`。

| 输入类型 | value | 校验 |
| --- | --- | --- |
| f64 | 数字 | finite，绝对值不超过 1e50；对数价格算法另外要求 >0 |
| (f64,f64) | 长度2的数组 | 两个序列在同一时点对齐；不从其他资产隐式补齐 |
| Candle | open/high/low/close/volume/timestamp | 正价格、合法 OHLC、非负量；timestamp=at_ms，调用方保证已收盘 |
| OrderBook | bids/asks，元素 price/size | 双边非空、正价格、非负量、正确排序、不交叉；每边至多128档 |
| Trade | price/size/side/timestamp | 正价格、非负量、Buy/Sell、timestamp=at_ms |
| TradeQuote | trade/mid | 已校验成交及正的中间价；成交时间与 at_ms 相同 |
| DerivativesTick | 资金费率、标记/指数/期货价格、持仓/多空/成交/强平量及 timestamp | 三个价格为正，其余金额非负，费率有限，timestamp=at_ms |
| CrossSection | members/timestamp | 1..128个成员；change有限、volume非负；极值/均线/买入状态由调用方明确提供 |

所有 JSON 数字有限且有界；深度至多8、单对象至多32字段、嵌套数组至多128项、字符串至多128字节。反序列化后重新走丰富类型的校验构造器，不能用 JSON 绕过其不变量。会话算法沿用源代码的固定 UTC 偏移，未引入交易日历或夏令时数据库。

## 输出和状态

每根输入对应一个 Row：at_ms、available_at_ms、samples_seen、status、value。多输出保留具名结构；事件/替代图表在本次确认时间输出，历史 Row 不会被回填。

- ready：本次输出可序列化为完整有限结果。
- warming_up：无输出且还没达到源算法声明的最少输入数。
- pending：达到该下界后仍无输出，包括等待会话、成交量阈值或已确认枢轴；不能把 pending 当成有效零。
- undefined_result：输出含非有限数或缺失分量，JSON保留null；wire层不额外前向填充。个别源算法在零分母时保持上次输出，其原生行为仍作为来源变体保留，不能据ready状态推断分母一定非零。

原始 Wickra 的显式中点/零值分段约定予以保留，结果带 `source_neutral_value_conventions`。例如 flat RSI/StochRSI 的50与 Vortex 的0是该来源变体的约定，不能声称已满足严格 Profile 的数学未定义语义。需要 R1 严格语义时继续使用既有 `stoch_rsi.14_14` / `vortex.14`。低层原生 API 不提供 wire 层的全部限额和时间校验。

## 流式、恢复与资源

Rust Stream 支持 new、update、latest、reset、snapshot、restore；MCP 增加 reference_catalog、reference_batch_calculate、reference_stream（create/advance/inspect/reset）。所有公式只在核心库运行。

在线计算调用相同原生 update。为确保错误、取消或意外 panic 后状态不变，wire 更新使用私有候选副本；更新成本等于原生算法成本加状态复制成本，不能笼统标为 O(1)。原生的各算法成本可从其有界窗口、回归或频谱循环核查。

`talib.*`适配器每次调用原生批量算法重放完整已观察前缀；成本高于增量更新，界限和限制见其独立契约。

快照保存完整受限历史，不反序列化未经语义检查的内部 Rust 状态。恢复核验版本、身份、配置、校验和，再按相同路径重放。历史上限4096条/4 MiB，单行输出上限64 KiB、结果上限4 MiB；达到历史上限时明确拒绝继续扩展，不丢弃种子历史伪造连续性。恢复成本为完整历史重放成本；这不是无限长度流的压缩状态格式。

Renko/Point-and-Figure/Kagi 调用前检查价格移动与构造阈值之比；单次完成图表最多128根，过大跳空明确失败。构造器的输入范围约束与候选副本上限一起控制资源；MCP 复用现有2并发、10秒协作检查及8 MiB协议响应限制。

## 可复现性与验收

原始源码、MIT许可证、文件blob、派生摘要及补丁分开保留。原始基线不参与编译；所有迁入源码参与fmt、测试和Clippy。整仓补充后已恢复7个模块内原始随机属性测试，另运行515项全目录属性测试；原始文件在vendor中不变。

验证覆盖每个目录项的构造与输入种类、批量调用、序列化恢复、下一次输出、重置、MCP 与核心一致性，以及旧45个 Profile 快照。独立公式与边界测试在迁入源码和 `tests/reference_all.rs` 中；完整命令和结果见验收记录。
