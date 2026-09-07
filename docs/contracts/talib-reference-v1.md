# TA-Lib reference v1

对应跨库指标迁移和逐项数值验收需求。201 个规格卡由 `reference-catalog.json`
中 `talib.*` 条目组成：参数名、默认值、范围、输入类型和全部输出字段逐项列出。
公式来源固定为 TA-Lib `2f0426d4a3e7d5b153c83ce7e6c4b9b05d8ca9c7` 的官方 Rust 实现。

- 算法保持 TA-Lib 默认兼容模式、默认 candle settings 和 unstable period=0。
- scalar 输入为调用方指定序列；Price 输入使用 OHLCV；OBV/VWMA 的 Real 输入固定使用 candle.close。
- 输出字段顺序及单位遵循源 API，包括 candle patterns 的整数强度；多字段输出使用源字段名。
- lookback+1 个样本前为 warming_up；非有限结果按 Reference 契约编码为 null/undefined。
- 输入必须有限、时间有序；计算窗口最多 4096 条，参数受目录范围限制。并非完整开放原生参数范围。
- 每次更新重放截至当前的全部历史，保持批量初始化。整个批次存在二次或更高时间成本，不能用于承诺常数时间的实时接口。
- 快照保存输入历史并用同一算法恢复；没有未来输入、图表回填或隐式 I/O。

## 验收范围

`scripts/check-talib-parity.py` 调用独立编译的相同提交 C DLL，为每个函数生成
256 条 mixed、flat、trend 输入下的全部预热和有效输出。容差为
`abs(actual-expected) <= 2e-9 * (1 + abs(expected))`。
`tests/talib_c_parity.rs` 使用提交的独立参考夹具复验 201×3 案例。
目录构造、输入校验与切段恢复还由 `tests/reference_all.rs` 覆盖。

映射状态 `verified_default_cases` 仅表示上述案例数值验收，不表示所有参数、
所有 candle setting、极端浮点输入、跨平台逐位一致或完整性能验收。
ta、talipp、TTR 的同名函数不因 TA-Lib 通过而自动获得兼容状态。
