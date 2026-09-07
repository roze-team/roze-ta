# 2026-09-07 外部 Rust 指标全量迁移与补充

对应 FR-IND-009 / AC-IND-009 的实现增量；不将可调用覆盖等同于全部跨库兼容验收。

## 实际交付

- 固定 Wickra 提交 `7ed1504c805cb45fff7a317a659eb3ad559a80a7`，保留527个原始文件、Git blob和MIT许可；524个Rust维护文件全部参与构建。
- 514个Wickra导出算法与66个补充运算，共580个Reference入口。实际目录由 `export_reference_catalog` 导出至 [reference-catalog.json](reference-catalog.json)。计数包含同族变体及基础运算，不是580种独立交易信号。
- 核心统一批量/流式/重置/恢复，以及3个Reference MCP工具；既有50个Profile、原生接口和旧快照继续保留。MCP总计10个工具，各类数量有重叠，不相加。
- 886个外部来源条目全部登记本地候选或非指标分类：514个直接迁移、368个跨库公式映射、4个TTR辅助功能（下载、符号发现、缺失校验、复权因子预处理）。后者不作为技术指标复制。

## 验证

- `rtk proxy cargo test --workspace --locked --offline --quiet`：通过，核心4423个单元测试及563个文档测试，所有集成/MCP测试通过。
- `rtk proxy cargo test -p roze-ta --test reference_all --locked --offline --quiet`：新增最后两项后重跑，8项全部通过。覆盖每个目录项构造、16条类型化输入、序列化恢复、下一条输出、重置；独立手算/闭式公式、零分母、非法丰富输入、取消与时间隔离。
- MCP集成对所有580入口逐一与核心对照；真实stdio进程测试批量与跨进程恢复。最终重跑11个MCP单元测试和1个stdio测试均通过。全目录对照证明接线一致，不冒充独立公式oracle。
- 迁入4278个源算法测试及7个保留独立参考体的确定性属性网格；源算法测试不能单独证明与另外四库默认参数完全兼容。
- `cargo fmt --all -- --check`、workspace/all-targets Clippy `-D warnings`：通过。
- `verify-upstream.ps1`：100个Yata原始文件；`verify-native.ps1`：90个派生映射、补丁重放、许可与45个旧快照。
- `verify-wickra-full.py`：527个原始blob、524个维护文件、MIT许可与完整补丁逐字节重放。原始SHA不随派生修改更新。
- `verify-reference-indicators.ps1`：R1来源/补丁及886条映射的完整性、实际入口、契约与证据链接。

## 明确未验收项

368个跨库来源条目使用 `mapped_variant`，表示已有可调用的本地公式候选，**尚未完成逐项跨库数值兼容验收**。同名不足以认定等价，特别是TA-Lib形态阈值/信号单位、均线种子、默认参数、复合输出和TTR可选公式。
因此FR-IND-009实现覆盖取得上述增量，完整AC-IND-009仍未关闭；不得把候选映射自动改成兼容通过。

Reference快照保存受限历史：4096条/4 MiB，不是无限历史压缩状态。保留Wickra中性值/种子约定；严格数学未定义Profile另有版本。丰富输入由调用方提供，不使用OHLCV伪造订单簿或横截面。

规格：[Reference V1](../contracts/reference-all-v1.md)、[66个补充公式](../contracts/reference-extras.md)。来源维护：[迁移说明](../patches/wickra-full-migration.md)。
