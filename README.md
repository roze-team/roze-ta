# roze-ta

独立的纯 Rust 技术分析项目。基于固定版本 Yata，提供指标目录、可复现批量计算与只读 MCP。

**项目需求与验收标准：[docs/requirements.md](docs/requirements.md)。**

工程约定见 [docs/project-standards.md](docs/project-standards.md)，依据 `roze-skills` 制定；
工具链固定为 Rust 1.98.0。上游下载与完整性验证见
[来源验证记录](docs/evidence/2026-09-03-yata-import.md)。

## 当前原型

新增 **S2A 分析**：冻结预测的 Brier/Log Loss 与可靠性分桶、均值 IID/移动块 Bootstrap、
带真实标签时间清除的训练/校准/验证切分。[用法](docs/usage/validation-v1.md)；概率校准模型拟合仍待实现。

新增 **E1 分析**：绩效与回撤、Sharpe/Sortino、VaR/ES、HAC、交易统计、因子 IC/RankIC/ICIR。
使用同一个只读分析 MCP；[用法与示例](docs/usage/evaluation-v1.md)、[公式覆盖范围](docs/formula-coverage.md)。

- Yata v0.7.0 完整源码位于 `vendor/yata`，保留 Apache-2.0 许可证。
- `crates/roze-ta`：统一计算封装，33 类指标、45 个固定参数 Profile；A1 新增 10 类，A 批尚未整体交付。
- `crates/roze-ta-mcp`：stdio 服务，提供 `indicator_catalog` 和 `indicator_batch_calculate`。
- `roze_ta::indicators` / `roze_ta::methods` 可访问未纳入统一目录的上游能力。
- 已登记的全部 45 个 Profile 支持统一流式更新、版本化无损状态快照及恢复；[A1 规格卡](docs/contracts/expansion-a1.md)明确新指标的公式、种子和逐项预热。
- V2 提供 latest/series、可知时间校验、结构化状态及规范哈希；新增 MCP 工具 `indicator_batch_calculate_v2`。
- 当前仍为 P1 首批实现；复杂公式独立参考、预热审核、多周期与跨平台验收见 [实施状态](docs/implementation-status.md)。
- S1 首批统计/概率：描述与稳健统计、收益变换、滚动 Z-score、成对相关/OLS、五种基础分布、显式种子采样、成熟事件 Wilson 与 Beta-Binomial。
- 新增只读 `analysis_batch_calculate`；参数、模型隔离和未完成范围见 [分析契约](docs/contracts/analysis-v1.md)，示例见 [使用说明](docs/usage/analysis-v1.md)。

```powershell
rtk cargo test --workspace --locked
rtk cargo build -p roze-ta-mcp --locked
rtk proxy powershell -NoProfile -File scripts/verify-upstream.ps1
```

MCP 客户端直接启动 `D:\Alion\roze-ta\target\debug\roze-ta-mcp.exe`，参数为空；
不要在协议 stdout 前套命令日志包装器。输入使用已收盘 OHLCV，最多 4096 根、64 个 Profile。
V2 使用方式见 [使用说明](docs/usage/engine-v2.md)，版本、快照与哈希见 [引擎契约](docs/contracts/engine-v2.md)。
运行示例：`rtk cargo run -p roze-ta --example stream --locked`。
数据由调用方提供，结果哈希不证明行情来源真实。

所有复杂默认 Profile 暂用保守的 256 根预热限制。上游 Yata 的单位和初始化可能不同于其他平台；
例如 ADX/MFI 使用 0～1，TRIX 是绝对差分变体，不能直接套用其他平台的阈值。
版本戳包含上游 commit。上游原始代码在当前编译器有风格和生命周期提示，暂保留以便核对来源。

项目仓库：[roze-team/roze-ta](https://github.com/roze-team/roze-ta)。
本项目未发布到 crates.io、未自动替换 roze-quant 的依赖。
独立库不依赖 Roze 服务运行时；后续服务层应复用 Roze 原生能力。

## 来源

上游：<https://github.com/amv-dev/yata>，tag `v0.7.0`，
commit `5030e2349cedde60b0e367a9de9400d466ff644f`。
原始代码由上游 `git archive` 导出；`UPSTREAM.json` 保存文件 SHA-256 清单。
`vendor/yata/LICENSE` 适用于上游代码；外层 roze-ta 扩展使用根目录 MIT 许可证。

## 后续顺序

P0 独立项目和计算/MCP 原型 → P1 统一可靠引擎与指标审核 → P2 特征分析 → P3 业务接入。
完整需求是目标规格，不是全部功能已完成的声明。
