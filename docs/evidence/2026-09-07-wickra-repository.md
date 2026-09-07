# 2026-09-07 Wickra 整仓交付

固定提交 `7ed1504c805cb45fff7a317a659eb3ad559a80a7`，Git tree未截断。完整原始2639文件已逐项核验blob；此前527文件核心基线及其身份保留不变。

实际编译能力：全部514个核心导出、原始完整属性测试、可选rayon并行、纯Rust逐笔聚合与重采样。Reference总580入口与MCP总10工具保持原有计数。其他语言绑定、数据客户端、基准、golden样本和资料全部原样保存在vendor，未宣称已完成各语言发行构建。

验证命令：

```powershell
rtk proxy cargo test --workspace --all-features --locked --offline --quiet
rtk proxy cargo clippy --workspace --all-targets --all-features --locked --offline -- -D warnings
rtk proxy cargo fmt --all -- --check
rtk proxy python scripts/verify-wickra-repository.py
rtk proxy python scripts/verify-wickra-full.py
rtk proxy powershell -NoProfile -File scripts/verify-reference-indicators.ps1
rtk proxy powershell -NoProfile -File scripts/verify-upstream.ps1
rtk proxy powershell -NoProfile -File scripts/verify-native.ps1
```

全目录属性测试515项包含批量/流式、reset/fresh、非有限输入不污染状态及FAMILIES覆盖断言。它验证Wickra自己的契约，不用于证明与TA-Lib等另外四库数值完全兼容。

## 本机实际结果

Windows、项目固定Rust工具链；全workspace/all-features测试通过：4453个核心单元测试、515个全目录属性测试、566个文档示例及所有集成测试均通过。MCP的11个单元测试和1个真实stdio进程测试通过。
最终单独重跑8项Reference集成测试通过，其中新增导出名集合断言保证FAMILIES与Reference目录逐项相等，不仅核对数量。
fmt和workspace/all-targets/all-features Clippy（`-D warnings`）检查通过；2639文件整仓、527文件原核心、三个补充派生补丁、R1来源映射及Yata基线/原生补丁/45个旧快照全部校验通过。

源码：[完整树](wickra-repository-tree.json)、[补充派生清单](wickra-repository-derived.json)、[迁移说明](../patches/wickra-repository-migration.md)、[纯数据契约](../contracts/wickra-data-v1.md)。原有368条其他库候选映射状态不因本次整仓导入改变。
