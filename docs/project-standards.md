# roze-ta 项目规范

依据 roze-skills 的 Rust engineering 指引与当前 Roze 项目规范，结合本项目
[需求规格](requirements.md) 制定。需求是目标，现有实现和测试是当前能力的事实来源。

## 目录与所有权

- `vendor/yata/`：Yata v0.7.0 原始源码，保留包名、API、README、benches、许可证和构建文件。
- `crates/roze-ta/`：自有 Rust 技术分析 API、目录、批量/流式计算及状态契约。
- `crates/roze-ta-mcp/`：调用核心库的 MCP 适配器。
- `docs/requirements.md`：目标需求；`docs/contracts/`：版本化语义；`docs/evidence/`：验证记录。
- `scripts/`：可重复运行的验证工具；`UPSTREAM.json`：上游来源及完整性记录。

不为目录命名统一而搬迁既有代码。上游 Apache-2.0 与外层 MIT 许可证分别适用，不改标上游版权。
上游修复必须单独记录原始摘要、本地补丁、原因、测试与升级影响。

## Rust 与依赖

固定 Rust 1.98.0，与当前 Roze 验证工具链一致；这不是 MSRV 声明。保留 Cargo.lock。
核心算法采用同步、无隐式 I/O 的 Rust API，存储与调度由调用方决定。
可恢复失败使用结构化 Result，正常缺失使用 Option；调用方需要分类处理时不得使用字符串错误替代错误类型。
生产代码禁止无依据的 unwrap、panic 和吞错；上游既有行为单独核验。
所有权清楚，优先借用，不为绕过借用检查随意 clone 或增加共享锁。
默认 safe Rust；unsafe 需要实际依据、最小范围、SAFETY 说明和测试。
新增依赖前检查标准库、已存在依赖和 Roze 能力，明确许可证与 feature 范围。
批量输入、计算、输出、并发与状态占用均需有边界；性能优化以实际基准为依据。

## 计算契约

公式、价格源、参数范围、输出单位、顺序、最少样本、预热与时间对齐均写入指标规格卡。
批量、流式、切段恢复应使用同一计算路径，按规格中的精度策略验证。
非法输入、预热不足、数学未定义须区分状态，不使用零或上一有效值冒充结果。
状态快照、参数、实现与哈希协议需版本化，拒绝不兼容或损坏的恢复数据。
禁止未来数据泄漏；Pivot 的发生与确认时间分开，图表位移不能修改实际可知时间。
原型限制及 P1/P2 待办保持可见，不以增加指标数量代替公式、单位和时间语义审核。

## Roze 与 MCP 边界

独立核心库不强制依赖 Roze 服务运行时。
引入服务基础设施前，核查当前 Roze checkout，优先复用已有错误、配置、日志、生命周期和治理能力。
如新增 REST/RPC，先定义 .api/.proto，再使用 rozectl 生成边界；业务放 application-owned 模块。
MCP 使用官方 Rust SDK，遵循其 JSON-RPC 合约；适配器映射领域错误，不复制公式或维护另一份参数默认值。
stdio 的 stdout 仅传协议消息，日志写 stderr。工具保持只读，不增加交易、运维或持久化写入能力。
CPU 密集计算避免阻塞异步执行器；超时、取消与资源释放应有对应验收。
HTTP MCP 若实施，按需求另行完成身份验证、租户隔离、配额和审计。

## 验证与证据

自有 crate 的常规门禁：

```powershell
rtk cargo fmt -p roze-ta -p roze-ta-mcp -- --check
rtk cargo test --workspace --locked
rtk cargo clippy -p roze-ta -p roze-ta-mcp --all-targets --no-deps -- -D warnings
rtk proxy powershell -NoProfile -File scripts/verify-upstream.ps1
```

第三方源码不执行自动格式化与批量 lint 修复。其新编译器警告独立记录，不混同自有代码门禁。
参考值不能仅来自同一个封装的自我对照；需独立公式、手工小样本或明示变体的参考实现。
验证记录明确提交、工具链、平台、命令、结果和未验证范围；Windows 测试不能表示 Linux 或生产验收通过。
变更先在临时目录验证再落地，避免覆盖已有工作；回滚只恢复本次修改或移除本次新增文件。
文本采用 UTF-8，文档随公共 API、命令、参数和运行语义同步更新。

