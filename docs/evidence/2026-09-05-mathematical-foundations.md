# 数学基础公式补全记录

日期：2026-09-05；平台：Windows / PowerShell。
基线提交：`d0c37d23f00fd1080af5a96b4d3d2ece58c52ef5`；本记录对应其上的文档工作区差异。

## 需求与变更

依据用户“补全缺失公式”及“实施”要求，新增[数学基础](../mathematical-foundations.md)，并在需求总文档添加入口。
保留六类主题，补全注意力、协方差、PCA、贝叶斯、t 检验、MDP、GBM、Black–Scholes、梯度下降及组合优化公式。
修正主成分“不相关/独立”的区别，说明回撤路径依赖、换手率口径及模型适用条件。
相关规格：FR-STAT-006、FR-PROB-004、FR-PROB-011；本文未将背景公式登记为新增可调用能力。

## 验证

- `rtk git diff --check`：通过。
- `rtk proxy powershell -NoProfile -File scripts/verify-upstream.ps1`：通过，100 个上游文件。
- `rtk proxy powershell -NoProfile -File scripts/verify-native.ps1`：通过，90 项源码映射、补丁与许可、45 份迁移前快照。
- 文档复核：注意力矩阵维度、样本协方差分母、PCA 正交投影、贝叶斯归一化、t 检验自由度、折扣条件、期权终端条件及 QP 约束口径已逐项检查。

本次仅修改 Markdown，未改 Rust、依赖、MCP 或上游源码；未运行 Rust 编译、测试和 clippy，也未进行 Markdown 页面渲染或算法数值验收。
回滚可移除本次两份新增文档，并撤销需求总文档中新增的入口行。
