# R1 Rust 源码迁移验收记录

日期：2026-09-07；环境：Windows，本地 locked/offline Cargo 工作区。
需求：FR-IND-009 / AC-IND-009。本记录仅覆盖首批五项，不代表全部外部指标覆盖完成。

## 变更范围

- 迁入 Wickra 的 ALMA、SuperTrend、StochRSI、Vortex、Ulcer Index，以及 ATR、RSI、RollingSum、OHLCV 辅助代码；原始九个 Rust 文件单独保存在 vendor/wickra-r1，不参与构建。
- 新增五个固定 Profile；核心与 MCP 目录共 50 个 Profile、38 个指标族。低层构造器支持参数，统一引擎仍使用固定配置。
- 保留 MIT 许可和作者信息；原始 Git blob、派生 SHA256、补丁 SHA256 分开记录。补丁可从原始文件重放得到当前派生源码。
- 严格引擎将退化 StochRSI/Vortex 映射为 undefined_result；旧批量入口将其标记为单项失败并保留其余结果，取消错误仍向上传播。
- 历史快照测试保留全部 45 个原始 fixture，对目录原始前缀验证顺序；没有重生成历史快照。

## 验证结果

以下命令均退出 0：

| 命令 | 结果与证据 |
| --- | --- |
| `rtk proxy cargo fmt --all` | 自有工作区格式化完成；vendor 基线不参与 |
| `rtk proxy cargo test --workspace --locked --offline` | 全量通过，含七个 R1 集成测试、MCP 目录全配置批量/流式/恢复/重置、stdio 与 76 个文档测试 |
| `rtk proxy cargo clippy -p roze-ta -p roze-ta-mcp --all-targets --no-deps --locked --offline -- -D warnings` | 无警告 |
| `rtk proxy powershell -NoProfile -File scripts/verify-upstream.ps1` | 100 个 Yata 原始文件通过 |
| `rtk proxy powershell -NoProfile -File scripts/verify-native.ps1` | 90 个原生映射、补丁、许可证及 45 个旧快照通过 |
| `rtk proxy powershell -NoProfile -File scripts/verify-reference-indicators.ps1` | 九个原始 Rust blob、许可证、派生哈希、补丁重放及 886 个来源映射条目通过 |

R1 独立参考测试覆盖手算结果、历史切片公式、全部恢复切点、重置、常量/零范围窗口过期恢复、未来与无效输入不改变状态、构造参数边界及旧批量接口的单项失败隔离。

## 限制

五个实现标记为 implemented_variant，不宣称与全部外部库逐位一致。其余 881 个来源条目为 review_pending；同名已有实现仍需核对参数、初始化、预热和退化定义。
低层 Wickra API 保留部分上游中性值语义，统一引擎提供更严格的状态和输入检查，差异见规格卡。
本次未执行跨平台验证或发布，未修改 roze-quant，也未执行交易。
