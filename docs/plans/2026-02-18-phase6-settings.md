# Phase 6: 应用设置与持久化 实施计划

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 实现完整的应用设置功能，包括预览参数、外部工具路径 (Imagine/TexturePacker)、翻译 API 及项目根目录的持久化管理。

**Architecture:** 
- **后端 (Rust)**：在 `app_config_dir` 下管理 `app_settings.json`。实现 `load_settings` (含首次启动路径自动检测) 和 `save_settings` 命令。
- **前端 (Vue)**：新增 `SettingsPage.vue`。采用侧边 Tab 切换分类（预览/工作流/翻译/通用）。
- **Composables**：新增 `useSettings.ts` 用于全局配置共享。

**Tech Stack:** Tauri 2.x, Rust, Vue 3.

---

## Task 1: Rust 后端 — 实现设置持久化与自动检测

**Files:**
- Modify: `src-tauri/src/models.rs`
- Modify: `src-tauri/src/commands.rs`
- Modify: `src-tauri/src/lib.rs`

**Step 1: 在 models.rs 中定义 AppSettings 结构**

涵盖预览、工作流、翻译及外观设置。

**Step 2: 实现 load_settings 命令**

逻辑：
1. 检查配置文件是否存在。
2. 若不存在，执行自动检测逻辑（在 `C:\Program Files` 等路径下查找工具），生成初始配置。
3. 若存在，读取并解析。

**Step 3: 实现 save_settings 命令**

将前端修改后的配置写回 JSON。

---

## Task 2: 前端 — 创建 useSettings Composable

**Files:**
- Create: `src/composables/useSettings.ts`

**功能说明：** 封装 `loadSettings` 和 `saveSettings` 的 invoke 调用，提供全局响应式的 `settings` 状态。

---

## Task 3: 前端 — 开发 SettingsPage 页面

**Files:**
- Create: `src/views/SettingsPage.vue`
- Modify: `src/router/index.ts`

**功能说明：** 
- 分类 Tab：预览、工作流、翻译、通用。
- 工作流分类：展示 Imagine, TexturePacker 路径，提供 [浏览] 按钮（使用 `@tauri-apps/plugin-dialog`）。
- 翻译分类：设置 API Key, 快捷键。
- 导航：支持返回主页。

---

## Task 4: 前端 — 集成设置入口与动态路径注入

**Files:**
- Modify: `src/views/HomePage.vue` (更多菜单)
- Modify: `src/views/TaskPage.vue` (转换逻辑)

**Step 1: 激活设置入口**

在主页更多菜单中，将“程序设置”关联至路由跳转。

**Step 2: 替换硬编码路径**

在 `TaskPage.vue` 的 `handleStartConversion` 中，从全局 settings 中读取工具路径。

---

## Task 5: 联调与测试

**验证点：**
1. 首次启动能自动检测到已安装的 TexturePacker。
2. 修改设置后，`app_settings.json` 内容同步更新。
3. 转换功能在路径修改后依然能正确启动。
4. 页面刷新后设置不丢失。
