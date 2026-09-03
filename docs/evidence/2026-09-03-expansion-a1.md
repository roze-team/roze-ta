# A1 指标增量验证

2026-09-03；依据 roze-skills、项目规范及 V0.2 指标扩展清单。
实现先在 `C:\Users\xFc\AppData\Local\Temp\roze-ta-a1-20260903` 构建测试，再落地独立项目。
Rust 1.98.0，Windows x86_64-pc-windows-msvc；Yata v0.7.0 / 5030e2349cedde60b0e367a9de9400d466ff644f。
本轮没有新增依赖或上游源码修改，没有发布/部署、Git 提交或 roze-quant 接入。

## 结果

- 新增 10 个固定 Profile，10 个不同家族；统一目录由 23/35 增为 33/45。
- 各项规格、初值、单位和预热门槛见 [A1 规格卡](../contracts/expansion-a1.md)。
- 新目录元数据标记 formula_variant、family_id、warmup_rule、verification_status。
- 84 项任务表保持 60 核心 + 12 形态 + 12 结构计数；A=40/B=20；统计能力复用旧 ID。
- Yata 全部 36 个实际 Indicator 模块均有映射/候选/延期理由，示例模块不计入。

## 已执行验证

| 检查 | 结果 |
| --- | --- |
| `rtk cargo test --workspace --locked` | 241 passed：核心 6、S1 14、engine 12、A1 5、MCP 3、Yata 132、Yata 文档 69 |
| `rtk cargo fmt -p roze-ta -p roze-ta-mcp -- --check` | 通过 |
| `rtk cargo clippy -p roze-ta -p roze-ta-mcp --all-targets --no-deps -- -D warnings` | 自有代码通过；上游 7 条既有 warning 未改动 |
| `rtk proxy powershell -NoProfile -File scripts/verify-upstream.ps1` | 100 个原始源码文件摘要一致 |
| `rtk proxy powershell -NoProfile -File scripts/verify-expansion.ps1` | 84 项唯一 ID、分类/批次计数及 36 模块与真实源码对应通过 |

完整日志：[workspace test](a1-cargo-test.log)、[clippy](a1-clippy.log)。
变更文件摘要见 `a1-source-sha256.json`；工程尚无本轮提交，因此不虚构 Git commit。

## 逐项数值与状态证据

`tests/expansion_a1.rs` 的独立公式测试对 90 根不规则价格/成交量序列的每个就绪输出核对：
WMA 直接加权窗口、RMA 单独递推、DEMA/TEMA 三级独立 EMA、VWMA 直接加权求和、
ROC/MTM 按实际 14 期前价格、A/D 独立 CLV 公式、OBV 涨跌平盘累计、Williams 直接极值窗口。
容差为 1e-10×(1+|reference|)，不是以封装自身为独立参考。

共同状态测试自动遍历 45 个 Profile，在切点 0/1/14/257 比较批量/流式/恢复逐行与浮点位模式。
新专项验证精确就绪边界、零量/平盘、未定义恢复、零权重退出窗口、reset、非法输入事务性与未来数据隔离。
MCP 官方 SDK 测试新增 10 项、50 根全序列调用，structured_content 与原生完整结果相等。

## 实际旧版快照兼容

新增前的 S1 临时项目编译执行独立生成器，保存旧 35 个 Profile 各处理 280 根后的 snapshot 和下一根预期结果。
冻结数据：`crates/roze-ta/tests/fixtures/pre-a1-snapshots.json`；
生成器原文：同目录 `pre-a1-generator.rs.txt`，不作为当前示例自动重生成旧预期。
改动前目录/内核/引擎/锁文件摘要：[baseline source](a1-baseline-source.json)。
新版本逐项载入真实旧快照并处理下一根，全部 Measurement 与旧版预期一致。
Kernel 新枚举仅追加；旧参数、实现签名和已有序列行为保持不变。

## 落地与回滚

通过临时目录门禁后复制变更清单；覆盖前文件备份于
`C:\Users\xFc\AppData\Local\Temp\roze-ta-a1-backup-20260903`，manifest 包含路径、原有/新增和前后摘要。
回滚仅恢复本轮原文件、移除本轮新增文件；不得覆盖之后的其他修改。

## 剩余边界

A 批还缺 7 个目录家族，既有复杂 Profile 的独立公式/逐项预热仍待完成；本轮不是 A 批整体交付。
分析模块的批量统计不能算作统计指标流式完成。B/C/S2/S3 维持待办，未实现多周期或订单流。
本轮 volume 沿用调用方单位，尚无强类型 volume_type；VWAP 等需要完整数据合同后另行实现。
Linux、生产级负载/黑盒超时压力与发布验收未执行。
