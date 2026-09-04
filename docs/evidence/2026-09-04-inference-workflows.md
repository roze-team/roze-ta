# 统计工作流补齐验证记录

日期：2026-09-04。平台：Windows；Rust 1.98.0。
这是上一批工作流实现的验证记录；后续标准校准与最新门禁见[标准校准记录](2026-09-04-standard-calibration.md)。
Git 基线 `61b3ad8cbce8de6e9f505fbc8a98ec8cb504392b` 加首批未提交公式实现。
本次保留已有未提交文件，先将当前 350 个项目文件保存到 `target/continuation-baseline`，
在 `target/paper-implementation` 验证，再检查当前文件仍与基线一致后落地。
每个落地文件的前后 SHA-256 记录在 `target/continuation-changes.json`；
回滚仅恢复本次修改文件的备份或移除本次新增文件，不使用全工作区 reset。

## 对应需求及变化

- 来源章节 8.5–8.7、8.10、19.5、19.7、19.21：ADF/EG 高斯零假设模拟校准、
  VECM 短期系数/截距/残差协方差及多步预测、ARIMA/GARCH/DCC/Hawkes 有界参数拟合。
- FR-MCP-001/004、AC-005/006：四个新任务变体通过原生分析入口与既有 MCP 工具暴露；
  当前共 65 个论文任务变体、68 个完整示例（四个示例分别覆盖拟合任务的四种模型）。
- AC-002/003/004：独立参考、输入边界、时间可知性、迭代上限及协作取消。
- AC-009：原始 Yata 与原生派生映射保持不变，无新增运行时依赖。

## 已执行门禁

| 命令 | 结果 |
|---|---|
| `rtk cargo test --workspace --locked --offline` | 328 项通过，21 个 suite，测试执行 54.46 秒 |
| `rtk cargo clippy -p roze-ta -p roze-ta-mcp --all-targets --locked --offline --no-deps -- -D warnings` | 通过，无警告 |
| `rtk cargo run -p roze-ta --example export_schema --features schema --locked --offline` | 导出成功；仅分析请求/结果 Schema 有语义扩展 |
| `rtk proxy powershell -NoProfile -File scripts/verify-upstream.ps1` | 100 个上游文件通过 |
| `rtk proxy powershell -NoProfile -File scripts/verify-native.ps1` | 90 个源码映射、补丁重放、许可证、45 个快照通过 |

`cargo fmt` 已执行；落地时还执行格式检查、diff 空白检查和文件哈希核对。
测试、clippy 在临时目录执行；主项目接收相同字节，不将未执行的主目录重跑记为已执行。

## 数值依据

- [独立 VAR(2) 参考脚本](vecm-reference.py)：纯 Python 正规方程求解，生产实现为 Johansen+QR；
  比较截距、Gamma、残差协方差、三步均值与三步协方差，绝对容差 1e-9。
- rank=0、无常数、无差分滞后：预测为最后水平，h 步协方差是单步协方差的 h 倍。
- ARIMA(0,0,0)：样本均值 2、残差平方和 6；验证正常停止和迭代上限 false。
- GARCH：用最终参数独立重算递推与高斯负对数似然，验证目标改善和系数和小于 1。
- DCC：初始 R=I 时目标为 sum(z'z)/2，验证观测前的时序语义。
- Hawkes：以所有历史事件的双重求和独立核对强度积分和事件似然。
- Gaussian 校准：种子复现、左尾离散 p 值范围、临界值排序、截距模型平移/缩放不变性、
  EG 与普通 ADF 校准差异、非法初值与过量计算拒绝。
- 中途取消在第 30 次 checkpoint 触发并传播；非法拟合边界拒绝。
- 68 个请求全部经原生/目录/MCP 一致性验证和可知时间边界验证。

Gaussian 校准不是 MacKinnon 响应面或异方差稳健检验；坐标搜索不是全局优化证明；
预测协方差不包含参数估计不确定性。Heston 校准、Johansen 自动 rank 选择等剩余范围见
[规格卡](../contracts/inference-workflows-v1.md)。未验证 Linux、真实市场有效性、生产性能或真实交易。
