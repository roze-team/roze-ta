# E1 分析公式验证记录

2026-09-03；遵循 roze-skills 与项目规范。Windows x86_64-pc-windows-msvc、Rust 1.98.0。
临时验证目录：`C:\Users\xFc\AppData\Local\Temp\roze-ta-eval1-20260903`。
无新增依赖；Cargo.lock、Yata 原始源码和已有指标内核未修改。
当前没有本轮 Git 提交，使用变更文件 SHA-256 清单定位源码版本。

## 本批结果

新增三个批量分析 operation：performance、trade_summary、factor_evaluation。
覆盖收益绩效/回撤、Sharpe/Sortino/IR/TE/Calmar、历史及显式正态 VaR/ES、HAC 均值诊断、
净交易统计、截面 IC/RankIC/ICIR 与方向命中率。契约见 [evaluation-v1](../contracts/evaluation-v1.md)。
仍为 33 类指标/45 Profile；本批能力单独计数，不代表 S2 或全部候选公式完成。

参考用户公式文件 SHA-256：`28e441fd42f7d9ab5418fb045f57f2a5dc9400be2c965cae00ae6aecbaa19606`。
来源路径和范围映射见 [formula-coverage](../formula-coverage.md)；未修改原文。
显式固定 Sortino 的 MAR 分子/分母、ES 部分样本质量与 HAC 固定分母，避免模糊口径直接入库。

## 已执行门禁

| 检查 | 结果 |
| --- | --- |
| `rtk cargo test --workspace --locked` | 253 passed：核心 6、S1 14、engine 12、E1 11、A1 5、MCP 4、Yata 132、Yata 文档 69 |
| `rtk cargo fmt -p roze-ta -p roze-ta-mcp -- --check` | 通过 |
| `rtk cargo clippy -p roze-ta -p roze-ta-mcp --all-targets --no-deps --locked -- -D warnings` | 自有代码通过；保留上游 7 条既有 warning |
| `rtk cargo run -p roze-ta --features schema --example export_schema --locked` | 8 个 Schema 重新导出；更新 analysis request/result，并补齐 batch-result-v2 内嵌的既有领域错误枚举 |
| `rtk cargo run -p roze-ta --example evaluation --locked` | 三类操作运行成功；手算参考值见用法文档 |
| `rtk proxy powershell -NoProfile -File scripts/verify-upstream.ps1` | Yata v0.7.0，100 个文件 SHA-256 一致 |
| `rtk proxy powershell -NoProfile -File scripts/verify-expansion.ps1` | 84 唯一目标、60+12+12、A40/B20、36 上游模块决策均通过 |

完整日志：[tests](eval1-cargo-test.log)、[clippy](eval1-clippy.log)；
示例输出：[JSON](eval1-example-output.json)，output_hash=`1a887e0542876ff3301218f347bcee487a98bb3dc90ae2594c41fab6f54282f4`。
变更文件 SHA-256 见 [source manifest](eval1-source-sha256.json)。

## 独立参考与边界

- 对 [0.1,-0.2,0.25] 直接手算复利、年化、回撤、标准差、Sharpe/Sortino、跟踪误差和 Calmar。
- 历史 ES 采用 1.5 样本质量的小样本，覆盖分位点部分权重、并列损失及负 VaR；
  正态尾部在 c=0.5 用 phi(0)=1/sqrt(2*pi) 的独立闭式值验证。
- HAC 手算 gamma0=0.105/3、gamma1=-0.0625/3，验证 Bartlett 权重及负自相关下 n_eff>n。
- 交易 gross=[110,-40,10]、costs=[10,10,10] 验证净额 50、利润因子 2、胜率 1/3 和平局。
- 两个三资产截面用正/负线性关系验证 IC=+1/-1、汇总标准差 sqrt(2)、覆盖率 3/4，零方向单独计数。
- 数值恢复到峰值的舍入容差、常量/零分母、复利溢出、收益<=-1、重复交易/资产时点、
  内部延迟收益/交易、完整截面标签成熟、真实区间重叠、未来特征拒绝、预算和核心协作取消均有测试。
- 旧 S1 14 项与旧 35 Profile 真实快照恢复一起通过全量回归。
- 官方 rmcp SDK 连接、发现、四个只读工具保持；三类新 operation 同批执行，
  MCP structured_content 与 native 完整结果相等；非法年化参数返回 invalid_parameter。

## 落地与回滚

所有门禁在临时目录完成后，按明确清单复制至 `D:\Alion\roze-ta`。
原文件备份：`C:\Users\xFc\AppData\Local\Temp\roze-ta-eval1-backup-20260903`；
manifest 包含 existed、before/after SHA-256。复制前检查文件未在验证期间被其他工作修改，复制后比较摘要。
回滚只恢复本批原文件和移除本批新增文件；若正式目录已出现后续修改，先核对摘要，不能覆盖后续工作。

## 未验证或未实现

未运行 Linux、跨平台浮点验证、生产负载/协议黑盒超时压力；未发布或接入 roze-quant。
E1 为批量重放，无增量拟合或可恢复统计流状态。金融数据真实可知性、净收益计算、样本池成员和交易日历由调用方提供。
ICIR 未年化，方向命中率不是校准概率；HAC t 无 p 值推断；不实现本轮之外的时序模型、组合优化和交易执行。
