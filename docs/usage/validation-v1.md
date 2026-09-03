# 概率评估、Bootstrap 与时间切分用法

```powershell
rtk cargo run -p roze-ta --example validation --locked
```

示例读取 [validation-request-v1.json](validation-request-v1.json)，包含三类批量 operation：

- calibration_evaluation：五个人工二元预测。预期 Brier=0.175、基准 Brier=0.25、
  Brier skill=0.3、ECE=0.2、最大桶偏差=0.375。
- bootstrap_mean：对 [1,3] 做种子 42 的 100 次 IID 均值重采样。
  可用重采样均值只可能为 1、2、3，理论重采样均值为 2。100 次模拟值会有抽样误差。
- temporal_split：训练/校准/验证各保留一个样本；一条标签跨训练边界、另一条校准标签延迟可知，
  均被清除。示例时间单位是毫秒，全部为合成数据。

原生 Rust 使用 `roze_ta::analysis::calculate(&request)`。
MCP 使用现有 `analysis_batch_calculate`，将 JSON 原样作为 arguments；通过 indicator_catalog
发现 eval_calibration、prob_bootstrap_mean、stat_temporal_split。工具数量保持四个。

移动块配置示例（替换 bootstrap_mean 的 spec）：

```json
{
  "scheme": {"scheme":"moving_block","block_length":2,"assume_stationary":true},
  "seed":42,
  "replicates":1000,
  "interval_level":0.95
}
```

块长不得超过当前 cutoff 选择的样本数；此例如果只提供 [1,3]，L=2 将产生退化分布。
实际使用须依据序列依赖选择块长，不能把这个演示参数当作生产默认值。

概率评估须提供独立训练得到的固定基准概率，并记录模型及基准的训练截止时间。
分桶边界预先固定，不能反复依据同一验证集挑选。Log Loss 默认不裁剪，错误的 0/1 确定预测
返回 undefined；若显式指定裁剪，会报告实际裁剪计数。这里输出评估事实，不拟合概率校准器。

参数、公式、时间窗口与剩余边界见 [契约](../contracts/validation-v1.md)，
验收证据见 [S2A 验证记录](../evidence/2026-09-03-validation-s2a.md)。
