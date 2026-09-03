# S2A 概率评估、时间切分与重采样验证

2026-09-03；依据 roze-skills、项目规范与 FR-PROB-006/012/013、需求第 7 节。
平台：Windows x86_64-pc-windows-msvc；Rust 1.98.0。临时目录
`C:\Users\xFc\AppData\Local\Temp\roze-ta-s2a-20260903` 验证后写入正式项目。
没有本轮 Git 提交；源码版本由 [SHA-256 清单](s2a-source-sha256.json) 定位。
没有新增依赖；继续使用锁定的 rand 0.9.5、rand_chacha 0.9.0。

## 范围

- 冻结二元预测：Brier、自然对数 Log Loss、固定基准、Brier skill、可靠性分桶、ECE/MCE。
- 均值 Bootstrap：显式 IID/非环绕移动块、固定 seed、percentile 区间、标准误、模拟均值误差。
- 时间切分：显式 rolling/expanding 的训练/校准/验证分区，按真实标签结束/可知时间清除跨界样本。
- 现有 analysis_batch_calculate 统一调用，MCP 工具仍为四个；指标数量仍为 33 类/45 Profile。

语义、参数、最少样本与限制见 [validation-v1](../contracts/validation-v1.md)。

## 执行证据

| 命令 | 结果 |
| --- | --- |
| `rtk cargo test --workspace --locked` | 266 passed：核心 6、S1 14、engine 12、E1 11、A1 5、S2A 12、MCP 5、Yata 132、Yata 文档 69 |
| `rtk cargo fmt -p roze-ta -p roze-ta-mcp -- --check` | 通过 |
| `rtk cargo clippy -p roze-ta -p roze-ta-mcp --all-targets --no-deps --locked -- -D warnings` | 自有代码通过；Yata 保留既有 7 条 warning |
| `rtk cargo run -p roze-ta --features schema --example export_schema --locked` | 导出 8 个 Schema；analysis request/result 增加三类操作及类型 |
| `rtk cargo run -p roze-ta --example validation --locked` | 三类操作运行成功、输出有效 JSON |
| `rtk proxy powershell -NoProfile -File scripts/verify-upstream.ps1` | 100 个 Yata 原始文件摘要一致 |
| `rtk proxy powershell -NoProfile -File scripts/verify-expansion.ps1` | 84 唯一目标、A40/B20、60+12+12、全部 36 上游模块决策通过 |

完整日志：[tests](s2a-cargo-test.log)、[clippy](s2a-clippy.log)。
示例产物：[JSON](s2a-example-output.json)，
output_hash=`1bd815c4e0b05c745b26a1d78a6f16d52d553847345eaac38a397bcf17993994`。

## 数值与时间证据

- 五个手算预测得到 Brier 0.175、固定基准 Brier 0.25、skill 0.3、ECE 0.2、MCE 0.375；
  Log Loss 与直接对数公式一致，容差 1e-10*(1+abs(reference))。
- 错误确定性概率返回无限 Log Loss 的 undefined 状态，裁剪显式且只影响 Log Loss；
  精确分桶边界、空桶、基准零损失有独立断言。
- 标签缺失、未来可知、真实 horizon 重叠、训练截止泄漏、重复 ID、非法概率/边界/裁剪均测试。
  固定 cutoff 的未来未选中概率/标签改变不改变选样哈希和评分；与截断数据重放一致。
- IID [1,3] 的完整两次抽样分布为均值 [1,2,2,3]，理论均值 2、方差 0.5。
  固定 4096 次模拟与理论均值/标准差偏差均 <0.06（确定 seed 的统计容差）；
  95% percentile 区间 [1,3]，模拟均值标准误等于 bootstrap SE/64。
- 三点移动块穷举验证块内次序和最终截断；L=1 与 IID 在同 seed 下相等，L=n 为退化分布。
  相同 seed 完整 replicate 序列一致，改变 seed 改变序列；短样本、内部未知值和假设未声明拒绝。
- 时间切分测试真实跨界标签、延迟可知、边界同毫秒、样本不足、输入顺序无关、滚动/扩展窗口、
  特征未来可知和重复 ID。分区少样本报告 usable=false，不虚构成功。
- n*B 超过 2,000,000 的重采样请求被拒绝；已进入随机采样循环后取消返回 cancelled，
  不返回部分产物伪装为全部完成。
- 官方 rmcp SDK 同批调用三类操作，structured_content 与原生结果全字段相等；
  评估期内拟合基准返回 invalid_time。完整回归保留旧 S1/E1 与旧指标快照测试。

## 落地与回滚

变更通过临时目录门禁后，按明确文件清单复制到 `D:\Alion\roze-ta`。
复制前检查期间没有并发修改；原文件备份于
`C:\Users\xFc\AppData\Local\Temp\roze-ta-s2a-backup-20260903`，manifest 含 existed/before/after SHA-256。
复制后读取文件并核对摘要。回滚只恢复本轮原文件、移除本轮新增文件，先核对是否已有后续修改。

## 剩余边界

S2A 是部分需求交付，不是整个 S2：没有拟合概率校准器、通用统计量/BCa/平稳 Bootstrap、
完整 Purged CV 执行器、Monte Carlo 路径模型或统计检验。时间切分仅生成分区计划。
概率评分没有校准置信区间；Bootstrap 区间不保证有限样本覆盖率；基本结果均需调用方的真实数据与时间证明。
没有执行 Linux、跨平台浮点、长期负载或协议黑盒超时压力；没有发布、部署或修改 roze-quant。
