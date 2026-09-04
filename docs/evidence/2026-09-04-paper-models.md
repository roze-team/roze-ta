# 论文公式计算层验证记录

日期：2026-09-04。基线：`61b3ad8cbce8de6e9f505fbc8a98ec8cb504392b`。
验证平台：Windows，rustc 1.98.0 (88d9e12ae 2026-08-18)。
本记录对应首批公式计算层补丁，不代表已经提交或发布的新版本。
后续拟合、校准与 VECM 扩展及最新门禁见 [工作流验证记录](2026-09-04-inference-workflows.md)。

## 范围与需求

- FR-STAT-006～013、016～018：回归、PCA、推断、显式时序模型、研究检验和组合数学。
- FR-PROB-001、005～008、013：新增四类分布、闭式估计、联合块 bootstrap、种子模拟及不确定性。
- FR-MCP-001～004、006 / AC-006：七类新操作内 61 个请求的原生、目录和 MCP 一致性。
- AC-009：Yata 基线与迁移快照不变。

新增 61 个任务变体，七类新操作；总计 21 类分析操作，MCP 工具仍为七个。
新建 [131 条章节映射](../paper-formula-coverage.csv)，不将重复公式章节计为新增算法。
此首批验证时尚未提供模型参数拟合、ADF/协整临界值和完整 VECM 预测；后续补齐情况见上述工作流记录。

## 验证方式与结果

先从基线 archive 创建 `target/paper-implementation`，在其中编辑和执行门禁，
再生成受路径约束、保留原文件备份的补丁落地，避免覆盖其他任务修改。

| 命令 | 结果 |
|---|---|
| `rtk cargo fmt -p roze-ta -p roze-ta-mcp -- --check` | 通过 |
| `rtk cargo test --workspace --locked --offline` | **320 项通过，20 个 suite** |
| `rtk cargo clippy -p roze-ta -p roze-ta-mcp --all-targets --locked --offline --no-deps -- -D warnings` | 通过 |
| `rtk cargo run -p roze-ta --example export_schema --features schema --locked --offline` | 八个 Schema 导出成功，仅分析请求/结果发生语义扩展 |
| `rtk proxy powershell -NoProfile -File scripts/verify-upstream.ps1` | 100 个原始文件通过 |
| `rtk proxy powershell -NoProfile -File scripts/verify-native.ps1` | 90 个派生映射、补丁重放、许可证、45 个迁移前快照通过 |

测试还覆盖时间 cutoff、前缀选择、未知字段、计算限额、取消、常量/秩不足状态、
种子复现、迭代收敛标志、既有批量/流式/恢复及实际 stdio 进程边界。

## 独立数值证据

- Lo/PSR：手工三点样本，单周期 Sharpe=1，聚合 sqrt(2)，PSR=Phi(4/3)。
- PBO：两个候选在两半样本中反向排名，两个互补 split 的 PBO=1、logit=−ln(2)。
- RC/SPA：完整环形块保持均值的退化重采样场景，plus-one p=0.01；常量列 SPA 明确 undefined。
- OLS/Ridge/Lasso：手工直线与正交设计；PCA 对角协方差；BL 单变量正态共轭更新。
- 风险平价：diag(1,4) 的权重为 (2/3,1/3)；LW 四点样本收缩强度 17/18。
- GARCH、DCC、Kalman Joseph 更新、OU 和 Hawkes：独立手工小样本。
- Black–Scholes：S=K=100、r=.05、sigma=.2、T=1 的 call=10.450583572185565。
- Heston：[独立 RK4 Riccati ODE + Simpson 参考](heston-reference.py)，call=10.394218552313326；
  Rust 使用不同的闭式特征函数路径，另验证 xi=0 的确定性方差极限。
- Johansen：[独立 2×2 行列式参考](johansen-reference.py)，根 0.3096880026203923、0.012741373431172598，trace(0)=14.95395931961356；
  Rust 使用白化与对称 Jacobi 特征分解。

参考 Python 脚本仅用于离线核对，核心与 MCP 运行不依赖 Python。未新增 Cargo 依赖。
没有验证 Linux、生产性能、远程 MCP 身份治理、外部数据真实性或真实交易；本记录不作这些承诺。
