# roze-ta 工程约定

- 使用 `roze-skills`，先阅读 `docs/project-standards.md` 和 `docs/requirements.md`，再读取技能中与改动相关的 reference。
- `docs/requirements.md` 及其扩展文档是目标规格，不能将需求表述为已实现能力。变更应标注对应需求与验收证据。
- 项目独立于 `roze-quant`；不自动修改其依赖、发布项目或执行真实交易。
- 所有 shell 命令以 `rtk` 开头，未支持的命令使用 `rtk proxy`。

## Hermes MCP 运维规则
- 涉及服务器、容器、文件、数据库或服务状态时，优先使用 Hermes MCP 工具读取真实状态。
- 严格遵守 scoped Token 的服务器范围与 Hermes AI 策略，不得绕过审批、黑名单或技能危险命令规则。
- 修改前说明目标、影响范围和回滚方式；执行后读取状态验证结果。

## 源码与实现
- `vendor/yata/` 保留 Yata 原始名称、作者、许可证和代码。不要批量格式化或修复上游 lint。
- 上游补丁必须记录原因、最小差异、测试和升级影响；校验清单需区分原始基线与补丁，不能更新哈希掩盖变化。
- 自有扩展在 `crates/roze-ta/`，MCP 在 `crates/roze-ta-mcp/`；沿用既有结构。
- 核心库保持纯 Rust、无隐式 I/O；MCP 使用官方 Rust SDK 并调用核心库，不复制公式。
- 可恢复错误采用结构化 Result；生产逻辑不使用无依据的 unwrap 或 panic，默认 safe Rust。
- 指标新增遵循规格卡、实现、独立参考值、边界测试、目录注册、文档六个步骤。
- 自有代码及 crates/roze-ta 内的派生算法执行 fmt、针对性测试与 clippy；vendor/yata 仅作不参与构建的原始基线。
- 派生模块保留 Apache-2.0 文件头；同时运行 verify-upstream.ps1 与 verify-native.ps1，文档与实际行为同步。
