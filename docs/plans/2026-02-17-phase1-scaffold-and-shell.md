# Phase 1: 项目脚手架 + 设计系统 + 主窗口框架

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 从零搭建 PGB1 的 Tauri 2.x + Vue 3 项目，实现设计系统 CSS 变量、无边框毛玻璃主窗口、四区域布局骨架，产出可运行的空壳应用。

**Architecture:** Tauri 2.x 双进程架构 — Rust 后端 (`src-tauri/`) 负责窗口管理和系统API，Vue 3 前端 (`src/`) 负责 UI 渲染。CSS 变量作为设计系统 SSOT，通过 `data-theme` 属性切换明暗主题。主窗口无边框 + `transparent: true`，自定义拖拽区域和窗口控制按钮。

**Tech Stack:** Tauri 2.x, Rust, Vue 3 (Composition API + `<script setup>`), Vite, CSS Custom Properties, 猫啃网糖圆体

**环境确认:** cargo 1.91.1, node v24.11.0, npm 11.6.1, tauri-cli 2.10.0

---

## 总体路线图（所有阶段概览）

| 阶段 | 内容 | 产出 |
|------|------|------|
| **Phase 1**（本文档） | 脚手架 + 设计系统 + 主窗口框架 | 可运行的毛玻璃空壳应用 |
| **Phase 2** | 三级页面导航 + Vue Router + 页面骨架 | 可点击导航的多页面应用 |
| **Phase 3** | 卡片组件 + Rust 文件系统后端 | 显示真实项目数据的卡片列表 |
| **Phase 4** | 双视图模式 + 侧边栏详情 | 完整的素材浏览体验 |
| **Phase 5** | 文件拖拽 + 任务管理系统 | 核心工作流可用 |
| **Phase 6** | 日报打卡 + 翻译 + 转换进度悬浮窗 | 辅助功能完成 |

---

## Phase 1 任务清单

### Task 1: Tauri + Vue 3 项目初始化

**Files:**
- Create: 整个项目脚手架（由 `npm create tauri-app` 生成）
- Modify: `src-tauri/tauri.conf.json`（窗口配置）
- Modify: `src-tauri/Cargo.toml`（项目元数据）

**Step 1: 用 Tauri CLI 创建项目**

```bash
cd D:/work/pgsoft/PGB1
npm create tauri-app@latest . -- --template vue-ts --manager npm
```

选项说明：
- 模板：`vue-ts`（Vue 3 + TypeScript + Vite）
- 包管理器：`npm`
- 项目目录：`.`（当前目录）

**Step 2: 安装依赖**

```bash
cd D:/work/pgsoft/PGB1
npm install
```

**Step 3: 配置 Tauri 窗口（无边框 + 透明）**

修改 `src-tauri/tauri.conf.json` 中的窗口配置：

```json
{
  "app": {
    "windows": [
      {
        "title": "PGB1",
        "width": 1280,
        "height": 720,
        "minWidth": 1280,
        "minHeight": 720,
        "decorations": false,
        "transparent": true,
        "center": true
      }
    ]
  }
}
```

注意：Tauri 2.x 的配置结构和 1.x 不同，`windows` 在 `app` 下而非 `tauri` 下。

**Step 4: 验证项目能启动**

```bash
npm run tauri dev
```

Expected: 打开一个无边框窗口，显示 Vue 默认页面。

**Step 5: 清理 Vue 默认内容**

删除 `src/components/` 下的默认组件，清空 `src/App.vue` 为最小模板，清空 `src/style.css`。

**Step 6: Commit**

```bash
git init
git add -A
git commit -m "chore: 初始化 Tauri 2.x + Vue 3 项目脚手架"
```

---

### Task 2: 设计系统 CSS 变量

**Files:**
- Create: `src/styles/design-system.css`（所有 CSS 变量定义）
- Create: `src/styles/reset.css`（基础重置样式）
- Create: `src/styles/glass.css`（毛玻璃效果工具类）
- Modify: `src/main.ts`（引入样式文件）

**数据来源:** 严格从 `design/DesignSystem.md` 转译，不自行发明任何值。

**Step 1: 创建 reset.css**

```css
/* src/styles/reset.css */
*,
*::before,
*::after {
  margin: 0;
  padding: 0;
  box-sizing: border-box;
}

html, body {
  width: 100%;
  height: 100%;
  overflow: hidden;
  /* 禁止文本选择（桌面应用行为） */
  user-select: none;
  -webkit-user-select: none;
}

#app {
  width: 100%;
  height: 100%;
}

/* 自定义滚动条（毛玻璃主题） */
::-webkit-scrollbar {
  width: 6px;
}
::-webkit-scrollbar-track {
  background: transparent;
}
::-webkit-scrollbar-thumb {
  background: rgba(128, 128, 128, 0.3);
  border-radius: 3px;
}
::-webkit-scrollbar-thumb:hover {
  background: rgba(128, 128, 128, 0.5);
}
```

**Step 2: 创建 design-system.css**

将 DesignSystem.md 中的所有 CSS 变量转译为实际 CSS 文件。结构：

```
:root { /* 通用变量：基础色板、间距、圆角、阴影、动画、组件 */ }
:root[data-theme="light"] { /* 亮色主题语义变量 */ }
:root[data-theme="dark"] { /* 暗色主题语义变量 */ }
```

包含的变量分组（按 DesignSystem.md 章节顺序）：
1. 基础色板（Primary、Neutral、Functional）
2. 透明度层级
3. 间距系统（基础 + 语义化）
4. 圆角系统
5. 阴影系统
6. 动画系统（时长、缓动、组合过渡）
7. 排版系统（字体、字号、行高、字重、字间距）
8. 组件变量（按钮、卡片、标签、输入框、悬浮岛、侧边栏、窗口控制）
9. 响应式断点和 Z-Index
10. 亮色主题（背景色、文字色、边框色、毛玻璃、状态色、功能色）
11. 暗色主题（同上）

**Step 3: 创建 glass.css 工具类**

```css
/* src/styles/glass.css */
/* 毛玻璃效果预设 — 直接对应 DesignSystem.md 四、毛玻璃效果系统 */

.glass-subtle {
  background: var(--glass-subtle-bg);
  backdrop-filter: blur(var(--glass-subtle-blur));
  -webkit-backdrop-filter: blur(var(--glass-subtle-blur));
  border: var(--glass-subtle-border);
  box-shadow: var(--glass-subtle-shadow);
}

.glass-medium {
  background: var(--glass-medium-bg);
  backdrop-filter: blur(var(--glass-medium-blur));
  -webkit-backdrop-filter: blur(var(--glass-medium-blur));
  border: var(--glass-medium-border);
  box-shadow: var(--glass-medium-shadow);
}

.glass-strong {
  background: var(--glass-strong-bg);
  backdrop-filter: blur(var(--glass-strong-blur));
  -webkit-backdrop-filter: blur(var(--glass-strong-blur));
  border: var(--glass-strong-border);
  box-shadow: var(--glass-strong-shadow);
}
```

**Step 4: 加载自定义字体**

将猫啃网糖圆体字体文件放到 `public/fonts/` 目录。在 design-system.css 中通过 `@font-face` 加载（参照 DesignSystem.md L460）。

**注意**：需要确认字体文件的实际路径和文件名。如果字体文件尚未获取，先用 fallback 字体，留 TODO。

**Step 5: 在 main.ts 中引入样式**

```typescript
// src/main.ts
import './styles/reset.css'
import './styles/design-system.css'
import './styles/glass.css'
```

顺序很重要：reset → design-system → glass（后者依赖前者的变量）。

**Step 6: 验证主题切换**

在 App.vue 中临时添加一个主题切换按钮和几个使用 CSS 变量的元素，确认：
- 亮色主题变量生效
- 暗色主题变量生效
- 毛玻璃效果可见
- 字体加载正确

**Step 7: Commit**

```bash
git add src/styles/ src/main.ts public/fonts/
git commit -m "feat: 实现设计系统 CSS 变量（颜色/间距/毛玻璃/排版/组件）"
```

---

### Task 3: 主窗口四区域布局骨架

**Files:**
- Create: `src/layouts/MainLayout.vue`（主布局组件）
- Create: `src/components/TitleBar.vue`（顶部导航栏 — 三段式悬浮岛）
- Create: `src/components/Sidebar.vue`（左侧快捷方式栏骨架）
- Create: `src/components/WindowControls.vue`（窗口控制按钮）
- Modify: `src/App.vue`（挂载 MainLayout）

**参考:** `design/界面设计.md` L8-72, `design/DesignSystem.md` L894-917

**Step 1: 创建 WindowControls.vue**

实现最小化、最大化/还原、关闭三个按钮。

```vue
<script setup lang="ts">
import { getCurrentWindow } from '@tauri-apps/api/window'

const appWindow = getCurrentWindow()

async function minimize() {
  await appWindow.minimize()
}

async function toggleMaximize() {
  await appWindow.toggleMaximize()
}

async function close() {
  await appWindow.close()
}
</script>
```

样式使用 DesignSystem 中的 `--window-control-*` 变量。

**Step 2: 创建 TitleBar.vue**

三段式布局：
- 左侧悬浮岛：返回按钮 + 页面标题
- 中部悬浮岛：快捷功能区（此阶段为空占位）
- 右侧悬浮岛：更多菜单（此阶段为空占位）
- 窗口控制按钮（独立，不在悬浮岛内）

需要设置 `data-tauri-drag-region` 属性实现窗口拖拽。

**Step 3: 创建 Sidebar.vue**

左侧快捷方式栏骨架：
- 固定宽度 80px（`--floating-sidebar-width`）
- 毛玻璃效果（`glass-medium`）
- 顶部 [+] 按钮占位
- 此阶段只有骨架，无功能

**Step 4: 创建 MainLayout.vue**

四区域组合：

```
┌──────────────────────────────────────────────────┐
│  TitleBar（全宽）                   WindowControls│
├──────┬───────────────────────────────────────────┤
│      │                                           │
│ Side │          主功能区 <router-view />          │
│ bar  │                                           │
│      │                                           │
└──────┴───────────────────────────────────────────┘
```

使用 CSS Grid 布局。

**Step 5: 修改 App.vue 挂载 MainLayout**

```vue
<script setup lang="ts">
import MainLayout from './layouts/MainLayout.vue'
</script>

<template>
  <MainLayout />
</template>
```

**Step 6: 设置应用背景**

`<html>` 和 `<body>` 设置为透明（Tauri transparent 窗口需要），应用背景色 `--bg-app` 设在 `#app` 或 MainLayout 的根元素上。

**Step 7: 验证**

```bash
npm run tauri dev
```

Expected:
- 无边框窗口，四区域布局可见
- 毛玻璃效果在各区域正常显示
- 窗口控制按钮（最小化/最大化/关闭）可用
- 标题栏区域可拖拽移动窗口
- 亮色主题默认显示

**Step 8: Commit**

```bash
git add src/layouts/ src/components/ src/App.vue
git commit -m "feat: 实现主窗口四区域布局（标题栏/侧边栏/主功能区/窗口控制）"
```

---

### Task 4: 主题切换功能

**Files:**
- Create: `src/composables/useTheme.ts`（主题管理 composable）
- Modify: `src/components/TitleBar.vue`（添加主题切换入口）

**Step 1: 创建 useTheme composable**

```typescript
// src/composables/useTheme.ts
import { ref, watchEffect } from 'vue'

type Theme = 'light' | 'dark'

const theme = ref<Theme>('light')

export function useTheme() {
  function initTheme() {
    const saved = localStorage.getItem('pgb1-theme') as Theme | null
    theme.value = saved || 'light'
    applyTheme(theme.value)
  }

  function toggleTheme() {
    theme.value = theme.value === 'light' ? 'dark' : 'light'
    applyTheme(theme.value)
    localStorage.setItem('pgb1-theme', theme.value)
  }

  function applyTheme(t: Theme) {
    document.documentElement.setAttribute('data-theme', t)
  }

  return { theme, initTheme, toggleTheme }
}
```

**Step 2: 在 App.vue 中初始化主题**

```vue
<script setup lang="ts">
import { onMounted } from 'vue'
import { useTheme } from './composables/useTheme'
import MainLayout from './layouts/MainLayout.vue'

const { initTheme } = useTheme()
onMounted(initTheme)
</script>
```

**Step 3: 在 TitleBar 或临时位置添加切换按钮**

暂时在"更多菜单"区域放一个主题切换图标按钮。

**Step 4: 验证**

- 点击切换按钮，明暗主题切换
- 刷新后主题保持（localStorage）
- 毛玻璃效果在两个主题下都正常

**Step 5: Commit**

```bash
git add src/composables/ src/components/TitleBar.vue src/App.vue
git commit -m "feat: 实现明暗主题切换（localStorage 持久化）"
```

---

### Task 5: Tauri 权限和安全配置

**Files:**
- Modify: `src-tauri/capabilities/default.json`（Tauri 2.x 权限）

**Step 1: 配置 Tauri 2.x capabilities**

Tauri 2.x 使用 capabilities 系统替代了 1.x 的 allowlist。需要在 `src-tauri/capabilities/default.json` 中声明应用需要的权限：

```json
{
  "$schema": "../gen/schemas/desktop-schema.json",
  "identifier": "default",
  "description": "PGB1 默认权限",
  "windows": ["main"],
  "permissions": [
    "core:default",
    "core:window:default",
    "core:window:allow-close",
    "core:window:allow-minimize",
    "core:window:allow-toggle-maximize",
    "core:window:allow-start-dragging",
    "core:window:allow-set-focus"
  ]
}
```

后续阶段会逐步添加文件系统、shell、dialog 等权限。

**Step 2: 验证权限生效**

```bash
npm run tauri dev
```

确认窗口控制按钮和拖拽功能正常工作。

**Step 3: Commit**

```bash
git add src-tauri/capabilities/
git commit -m "chore: 配置 Tauri 2.x 权限（窗口控制 + 拖拽）"
```

---

### Task 6: Phase 1 收尾验证

**检查清单:**

- [ ] `npm run tauri dev` 正常启动
- [ ] 无边框窗口，1280×720 最小尺寸
- [ ] 四区域布局正确（左侧栏 80px + 顶部导航 + 主功能区 + 窗口控制）
- [ ] 毛玻璃效果在所有区域正常（subtle/medium/strong）
- [ ] 明暗主题切换正常，刷新后保持
- [ ] 窗口控制：最小化、最大化/还原、关闭
- [ ] 标题栏拖拽移动窗口
- [ ] CSS 变量全部来自 design-system.css，零硬编码
- [ ] 无 console 报错

**Step 1: 运行完整验证**

```bash
npm run tauri dev
```

逐项确认检查清单。

**Step 2: 最终 Commit**

如有修复，合并提交。

---

## 设计文档参考速查

| 需要查什么 | 去哪里看 |
|-----------|---------|
| CSS 变量具体值 | `design/DesignSystem.md` |
| 窗口布局 ASCII 图 | `design/界面设计.md` L8-72 |
| 悬浮岛尺寸 | `design/DesignSystem.md` L894-917 |
| 窗口控制按钮 | `design/DesignSystem.md` L942-958 |
| Tauri 窗口配置 | `design/DesignSystem.md` L1255-1271 |
| 主题切换实现 | `design/DesignSystem.md` L1152-1214 |
