# Wickra R1 最小源码迁移

日期：2026-09-07；依据 FR-IND-009 / AC-IND-009 及用户优先迁移已有 Rust 实现的指示。

## 来源与许可

来源：[wickra-lib/wickra](https://github.com/wickra-lib/wickra)。选用其 MIT 许可：
Copyright (c) 2026 kingchenc and the Wickra contributors。
许可全文保留于 `vendor/wickra-r1/LICENSE-MIT` 和 `crates/roze-ta/src/wickra/LICENSE-MIT`。
原件为 9 个 Rust 文件的选定子集，不是整个源码仓库；各文件 Git blob SHA 见[来源清单](../evidence/reference-r1-source.json)。
派生 SHA-256 单列于[派生清单](../evidence/reference-r1-derived.json)，不能修改原始摘要来掩盖派生变更。

## 迁入范围与修改原因

- 迁入 ALMA、SuperTrend、StochRSI、Vortex、UlcerIndex，以及 ATR、RSI、OHLCV、RollingSum 辅助算法。
- 原始文件完整保留，包括测试；参与编译的副本提取运行时代码，RollingSum 只提取所需结构和实现。独立本地测试不依赖上游 approx/proptest。
- 模块与文档导入路径改为 `roze_ta::wickra`；添加 serde 与 crate 内可见字段供有界快照验证，保留原作者、许可证和修改声明。
- 使用本地结构化 Error 和最小 Indicator/BatchExt 接口，不引入 thiserror/rayon 等新依赖。构造器周期上限4096，补齐 StochRSI 随机窗口上限检查。
- ALMA 增加极端 sigma 下归一化权重和不可表示的错误检查；避免“exp 始终大于零”的浮点假设。
- ATR/RSI 去除未检查切片长度的优化批量捷径，批量通过同一 update 路径执行。
- Vortex 的有界窗口求和改为重算，Ulcer 的平方和每次重算，确保非零样本全部过期后没有残留；两者更新复杂度如实标为 O(n)。
- 统一引擎适配器对 StochRSI、Vortex 的数学未定义情况返回结构化状态；低层原始中点/零值约定保留。适配器不复制指标公式。
- 原有 Kernel 变体后追加 Reference，未改旧枚举序号、旧参数和旧快照布局。

逐文件差异保留在本目录的 `wickra-r1/*.patch`；校验工具会将补丁重放到临时目录后比较派生文件。
新旧来源核验独立执行：`verify-upstream.ps1`、`verify-native.ps1`、`verify-reference-indicators.ps1`。
本次未修改原 Yata 迁移清单、来源摘要或其已固定的第三方声明；Wickra 许可说明在本文件、README 入口及派生目录中单独保留。

## 升级影响与验证

升级先固定新的原始文件身份，再逐项复核补丁；不能将上游相同指标名当作行为兼容证据。
序列化字段、枚举、公式或默认值发生变化时必须提供新版本和恢复策略。
低层模块仍有上游 Option 返回和调用方校验约定；面向 MCP 的正式接口为统一引擎的五个固定 Profile，非任意上游参数兼容声明。
独立手工/历史切片参考、各预热阶段恢复、无效输入与前视隔离见 `crates/roze-ta/tests/reference_r1.rs`；
所有50个 Profile 的 MCP 批量、流式、恢复、重置一致性由现有覆盖测试枚举执行。
