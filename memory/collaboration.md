# 协作记忆

本文档承接 `CLAUDE.md` 中的 Skill 使用规则，避免行为指南引用断链。

## Skill 使用规范

| 场景 | 要求 |
|------|------|
| 创建新功能前 | 先做需求澄清与方案路演；如环境提供 brainstorming 类 skill，优先使用 |
| 多步骤任务 | 先给出可执行计划；如环境提供 writing-plans 类 skill，优先使用 |
| 完成前 | 做验证清单；如环境提供 verification-before-completion 类 skill，优先使用 |
| 当前环境没有对应 skill | 不阻塞任务，改用普通计划、实现、验证流程，并在最终说明 |

## 当前项目入口

1. 新会话先读 `INDEX.md`。
2. 编程定位按 `INDEX.md` → `CODE_INDEX.md` → 目标文件。
3. 版本、源码规模、文档入口变化后，优先更新 `INDEX.md` 和 `CODE_INDEX.md`。

**最后更新**：2026-06-16
