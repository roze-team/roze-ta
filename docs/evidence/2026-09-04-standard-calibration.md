# 标准校准扩展验证记录

日期：2026-09-04；Windows / Rust 1.98.0。
基线为 Git `61b3ad8cbce8de6e9f505fbc8a98ec8cb504392b` 加此前公式及工作流未提交实现。
保留当前 357 个文件的快照于 `target/finalmodels-baseline`，在 `target/paper-implementation`
完成实现与测试后，以逐文件前后 SHA-256 检查落地；记录在 `target/finalmodels-changes.json`。
回滚仅恢复本次涉及文件的基线或移除本次新增文件，保留前几批未提交改动。

## 对应需求与实现

- 原文 8.6/8.7：ADF N=1 与含截距 EG N=2 的 MacKinnon tau p 值及有限样本临界值。
- 原文 19.7：trace/max-eigen 检验的逐阶 rank 选择、所选 rank 对应 alpha/beta 与可选 VECM 预测。
- 原文 19.22：1..5 活动 Heston 参数、加权多报价目标、有界搜索及价格积分收敛检查。
- AC-002–006：独立数值参考、非法值/预算/取消、原生与 MCP 的72个请求一致性。
- AC-009：statsmodels 系数保留 BSD-3-Clause；Yata 原始及派生基线不变。

共增加四个任务变体，论文扩展累计 69 个任务变体；72 个完整示例。
未新增运行时依赖；组合 crate 的许可表达式增加 BSD-3-Clause。

## 已执行验证

| 命令 | 结果 |
|---|---|
| `rtk cargo test --workspace --locked --offline` | **335 项通过，22 个 suite，测试执行47.48秒** |
| `rtk cargo clippy -p roze-ta -p roze-ta-mcp --all-targets --locked --offline --no-deps -- -D warnings` | 通过，无警告 |
| `rtk cargo run -p roze-ta --example export_schema --features schema --locked --offline` | 八个 Schema 导出，只有分析请求/结果 Schema 语义变化 |
| `rtk proxy powershell -NoProfile -File scripts/verify-upstream.ps1` | 100 个原始文件通过 |
| `rtk proxy powershell -NoProfile -File scripts/verify-native.ps1` | 90 个映射、补丁重放、许可证、45 个快照通过 |

最后一次目录兼容性调整保留 `fits` 的 boolean 类型，另加 fitting_tasks 明确任务；
对该改动追加运行 MCP 全分析入口覆盖测试。落地主目录再执行 fmt/diff 检查及源码完整性验证。
不将临时目录测试误记为主目录重新运行，也不代表 Linux 或生产环境已验收。

## 独立参考与边界

- 24 个 MacKinnon case/statistic 组合：来自固定来源表格，Python 直接幂展开和 erfc，
  Rust Horner 和 statrs；p 容差1e-10、临界值1e-12。包含左右尾截断。
- ADF 临界样本数 n−lags−1、EG 临界样本数 n−1，避免把两种校准混用。
- 已有独立 Johansen 参考数据在5% trace临界值15.4943下选rank=0，
  最大特征根临界值14.2639下选rank=1；另验证10%选择和预测衔接。
- Heston 确定方差极限恢复 v0=.04，Black–Scholes call=10.450583572185565。
- 非零 xi 校准恢复独立 Riccati ODE 参考的 xi=.3，call=10.394218552313326。
- 五参数/五报价路径核对加权目标、活动参数和迭代未收敛状态；不冒称这些报价可唯一识别全部参数。
- 非法边界、超预算和第15次 checkpoint 的中途取消均拒绝/传播。

系数原始哈希见[来源清单](statistical-table-sources.json)，重建脚本
[build-calibration-tables.py](build-calibration-tables.py) 只解析 AST 字面量且先核对来源哈希，
不会执行下载的 Python 源码。数值系数及完整 BSD 许可证保留在 crate 中。

模型适用范围、渐近校准限制、Heston 预算及未收敛语义见[契约](../contracts/standard-calibration-v1.md)。
不宣称完整研究平台、概率校准器训练、自动模型选阶、跨平台生产性能或真实交易验收已完成。
