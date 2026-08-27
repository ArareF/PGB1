# Spine 原件直传 Implementation Plan

> **For Codex:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 为静帧和序列帧提供统一的「Spine」原件直传入口，并使复制、状态扫描、重命名、更新与删除形成闭环。

**Architecture:** 前端继续调用现有 `copy_to_nextcloud`。后端在无 `02_done` 产物时按素材类型选择文件复制或目录递归复制；扫描器以 nextcloud `original/` 中的同名条目作为 Spine 上传事实来源。

**Tech Stack:** Vue 3、TypeScript、Tauri 2、Rust、文件系统单元测试

---

### Task 1: 序列帧 Spine 复制

**Files:**
- Modify: `src-tauri/src/commands/conversion.rs`

**Step 1:** 添加失败测试，期望序列目录递归复制到 `original/{素材名}/` 并返回帧文件数量。

**Step 2:** 运行 `cargo test copy_spine_sequence_original --lib -- --nocapture`，确认因功能缺失失败。

**Step 3:** 实现最小复制 helper，并接入普通与 Prototype 的 `copy_to_nextcloud` 兜底。

**Step 4:** 重跑目标测试并确认通过。

### Task 2: 序列帧 Spine 状态判定

**Files:**
- Modify: `src-tauri/src/commands/scanning.rs`

**Step 1:** 添加失败测试，期望 `original/{素材名}/` 命中时序列帧为 `Uploaded`。

**Step 2:** 运行 `cargo test spine_original_marks_sequence_uploaded --lib -- --nocapture`，确认当前返回 `Original`。

**Step 3:** 在序列帧判定中加入 `has_file_in_original` 合法分支，根层交付判定保持不变。

**Step 4:** 重跑目标测试并确认通过。

### Task 3: 生命周期闭环

**Files:**
- Modify: `src-tauri/src/commands/files.rs`

**Step 1:** 让 `rename_material` 扫描 nextcloud 的 `original/` 子目录。

**Step 2:** 让更新/删除对 `original/` 中的匹配目录使用递归删除，对文件保持单文件删除。

### Task 4: 侧边栏统一入口

**Files:**
- Modify: `src/views/TaskPage.vue`
- Modify: `src/locales/zh-CN.ts`
- Modify: `src/locales/en.ts`

**Step 1:** 将函数和日志语义统一为 Spine 标记。

**Step 2:** 按钮条件扩展到 `image | sequence` 且仅 `original` 状态显示。

**Step 3:** 中文和英文按钮统一显示 `Spine`，复用现有样式与忙碌态。

### Task 5: 文档与验证

**Files:**
- Modify: `design/文件命名与组织规则.md`
- Modify: `docs/code/views.md`
- Modify: `docs/code/rust-backend.md`
- Modify: `CODE_INDEX.md`

**Step 1:** 同步业务规则、数据流、文件职责与行数快照。

**Step 2:** 运行目标 Rust 测试与 `cargo test --lib`。

**Step 3:** 运行 `npm run build`、`cargo check`、`git diff --check`。
