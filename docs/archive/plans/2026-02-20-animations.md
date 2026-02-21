# 全局动画系统 Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 为 PGB1 补全缺失动画，以 transform 形变为主（滑入/缩放），opacity 只作辅助微变化（0.8→1），不做纯 fade。

**Architecture:** 三批次：① design-system.css 新增统一动画 token + 工具类；② 8 个弹窗包 Transition + dialog-content 动画；③ 路由切换 + 下拉菜单动画。现有侧边栏过渡去掉 opacity 纯化。

**Tech Stack:** Vue 3 `<Transition>`, CSS `@keyframes`, CSS 变量 (`--*`)，无额外依赖

---

## 整体约束

- 所有动画参数必须引用 design-system.css 里的 token，零硬编码数值
- opacity 允许范围：**0.8 → 1.0** 的辅助微变（不做 0→1 纯 fade）
- 弹窗遮罩层（`.dialog-overlay`）不加动画，只有 `.dialog-content` 动画
- 路由切换只包主 `<router-view>` 所在区域，不影响 Sidebar / TitleBar
- 每批改完即可视觉验证，不需要等全部完成

---

## Task 1：design-system.css 新增动画 token

**文件：**
- 修改：`src/styles/design-system.css` L215-L250（动画系统区域）

**目标：** 新增 3 个语义化 token + 2 个 `@keyframes`（放在 `:root` 之外的全局区域）

**Step 1：在 design-system.css 动画区域末尾（L250 后）新增 token**

在现有的 `--transition-card-hover` 后面添加：

```css
  /* 弹窗动画 */
  --transition-dialog-in:
    transform var(--duration-slow) var(--ease-out),
    opacity var(--duration-slow) var(--ease-out);
  --transition-dialog-out:
    transform var(--duration-normal) var(--ease-in),
    opacity var(--duration-normal) var(--ease-in);

  /* 下拉菜单动画 */
  --transition-dropdown:
    transform var(--duration-fast) var(--ease-out),
    opacity var(--duration-fast) var(--ease-out);

  /* 路由切换动画 */
  --transition-route-in:
    transform var(--duration-normal) var(--ease-out),
    opacity var(--duration-normal) var(--ease-out);
  --transition-route-out:
    transform var(--duration-fast) var(--ease-in),
    opacity var(--duration-fast) var(--ease-in);
```

**Step 2：在文件末尾（`:root {}` 块之外）新增 @keyframes**

```css
/* ===== 全局动画关键帧 ===== */

/* 弹窗内容：从下方缩小滑入 */
@keyframes dialog-enter {
  from {
    transform: translateY(16px) scale(0.97);
    opacity: 0.8;
  }
  to {
    transform: translateY(0) scale(1);
    opacity: 1;
  }
}

/* 弹窗内容：退出向下缩小滑出 */
@keyframes dialog-leave {
  from {
    transform: translateY(0) scale(1);
    opacity: 1;
  }
  to {
    transform: translateY(8px) scale(0.97);
    opacity: 0.8;
  }
}

/* 下拉菜单：从上方轻微缩小弹出 */
@keyframes dropdown-enter {
  from {
    transform: translateY(-6px) scale(0.95);
    opacity: 0.8;
  }
  to {
    transform: translateY(0) scale(1);
    opacity: 1;
  }
}
```

**Step 3：验证**

打开浏览器 DevTools，确认 `:root` 变量里能看到新增的 `--transition-dialog-in` 等 token（审查 `<html>` 元素的 CSS 变量）

---

## Task 2：弹窗统一动画（8 个组件）

**文件（均需修改）：**
- `src/components/CreateProjectDialog.vue`
- `src/components/EditProjectDialog.vue`
- `src/components/ShortcutDialog.vue`
- `src/components/UploadConfirmDialog.vue`
- `src/components/AttendanceDialog.vue`
- `src/components/NormalizationDialog.vue`
- `src/components/ScalingDialog.vue`
- `src/components/ConversionDialog.vue`

**原理：** 每个弹窗的模板结构都是：
```
<Teleport to="body">
  <div v-if="show" class="dialog-overlay">    ← 遮罩（不动）
    <div class="dialog-content glass-*">       ← 内容（加动画）
```

动画加在 `dialog-content` 上，不包 `<Transition>` 组件（CSS animation 直接驱动）。

**Step 1：为每个弹窗的 `v-if` 触发添加 CSS animation**

**通用做法（对每个弹窗重复此操作）：**

在各文件的 `<style scoped>` 中，找到 `.dialog-content` 规则，添加一行：

```css
.dialog-content {
  /* ... 已有样式 ... */
  animation: dialog-enter var(--duration-slow) var(--ease-out) both;
}
```

> 注意：`animation` 引用的 `dialog-enter` 是在 design-system.css 里定义的全局 `@keyframes`，scoped 样式可以使用全局 keyframes，无需重复定义。

**离开动画处理：** `v-if` 消失时无法触发 leave 动画（CSS animation 只有 enter）。这是有意设计——弹窗关闭是即时的，用户体验更干净。如果需要 leave 动画，需要用 Vue `<Transition>` 包裹，见 Task 2b。

**Step 1b（可选升级）：用 `<Transition>` 实现完整进出动画**

对有"取消"按钮的弹窗，可用 `<Transition>` 包 `.dialog-overlay` 的 `v-if`：

```html
<Teleport to="body">
  <Transition name="dialog">
    <div v-if="show" class="dialog-overlay">
      <div class="dialog-content glass-strong">
        ...
      </div>
    </div>
  </Transition>
</Teleport>
```

在 `<style scoped>` 添加（注意 scoped 组件 Transition CSS 类需要 `:deep()` 或用非 scoped block）：

```css
/* 弹窗进入/离开动画 — 只作用于 dialog-content */
.dialog-enter-active .dialog-content {
  transition: var(--transition-dialog-in);
}
.dialog-leave-active .dialog-content {
  transition: var(--transition-dialog-out);
}
.dialog-enter-from .dialog-content {
  transform: translateY(16px) scale(0.97);
  opacity: 0.8;
}
.dialog-leave-to .dialog-content {
  transform: translateY(8px) scale(0.97);
  opacity: 0.8;
}
```

**具体操作顺序（逐个弹窗）：**

### 2-A：CreateProjectDialog.vue

- 找到 `<Teleport to="body">` 内的 `<div v-if="..." class="dialog-overlay">`
- 用 `<Transition name="dialog">` 包裹整个 `v-if` 块
- 在 `<style scoped>` 末尾添加上述动画 CSS

### 2-B：EditProjectDialog.vue

- 同上，`v-if` 条件为 `mode !== null` 或控制弹窗显示的变量

### 2-C：ShortcutDialog.vue

- 同上，这个弹窗较大（320px 宽），scale 幅度够用

### 2-D：UploadConfirmDialog.vue

- 同上，这是最简单的弹窗（只有两个按钮）

### 2-E：AttendanceDialog.vue

- 同上

### 2-F：NormalizationDialog.vue

- 同上

### 2-G：ScalingDialog.vue

- 同上

### 2-H：ConversionDialog.vue

- 这个弹窗是全屏级或很大的，scale 从 0.97 开始不用改，translateY 可以从 20px 改为 24px 体现层次感（直接在 scoped 的 `dialog-enter-from` 里覆盖）

**Step 2：逐个视觉测试**

对每个弹窗：触发打开 → 看进入动画 → 触发关闭 → 看离开动画

---

## Task 3：修复现有侧边栏过渡（去 opacity）

**文件：**
- 修改：`src/views/TaskPage.vue` L1494-L1507
- 修改：`src/components/FileDetailSidebar.vue` L420-L430

**目标：** 去掉 `opacity`，纯 `translateX` 滑入滑出

**Step 1：TaskPage.vue（L1494-L1507）**

将：
```css
.sidebar-enter-active {
  transition: transform 300ms ease-out, opacity 300ms ease-out;
}
.sidebar-leave-active {
  transition: transform 200ms ease-in, opacity 200ms ease-in;
}
.sidebar-enter-from {
  transform: translateX(100%);
  opacity: 0;
}
.sidebar-leave-to {
  transform: translateX(100%);
  opacity: 0;
}
```

改为：
```css
.sidebar-enter-active {
  transition: transform var(--duration-normal) var(--ease-slide-in);
}
.sidebar-leave-active {
  transition: transform var(--duration-fast) var(--ease-slide-out);
}
.sidebar-enter-from,
.sidebar-leave-to {
  transform: translateX(100%);
}
```

**Step 2：FileDetailSidebar.vue（L420-L430）**

将：
```css
.file-sidebar-enter-active {
  transition: transform 300ms ease-out, opacity 300ms ease-out;
}
.file-sidebar-leave-active {
  transition: transform 200ms ease-in, opacity 200ms ease-in;
}
.file-sidebar-enter-from,
.file-sidebar-leave-to {
  transform: translateX(100%);
  opacity: 0;
}
```

改为：
```css
.file-sidebar-enter-active {
  transition: transform var(--duration-normal) var(--ease-slide-in);
}
.file-sidebar-leave-active {
  transition: transform var(--duration-fast) var(--ease-slide-out);
}
.file-sidebar-enter-from,
.file-sidebar-leave-to {
  transform: translateX(100%);
}
```

**Step 3：视觉验证**
- 打开任务页 → 点击一个素材 → 侧边栏应从右侧纯滑入（无淡入）
- 点击其他地方关闭 → 侧边栏纯滑出

---

## Task 4：下拉菜单动画（更多菜单）

**文件：**
- 修改：`src/layouts/MainLayout.vue`

**目标：** `v-if="showMoreMenu"` 的 `.more-dropdown` 从右上方缩放弹出

**Step 1：在 MainLayout.vue 模板中包 Transition**

将（L28）：
```html
<div v-if="showMoreMenu" class="more-dropdown glass-strong">
```

改为：
```html
<Transition name="dropdown">
  <div v-if="showMoreMenu" class="more-dropdown glass-strong">
    ...
  </div>
</Transition>
```

**Step 2：在 `<style scoped>` 末尾添加动画 CSS**

```css
/* 下拉菜单进入/离开动画 */
.dropdown-enter-active {
  transition: var(--transition-dropdown);
  transform-origin: top right;
}
.dropdown-leave-active {
  transition: var(--transition-dropdown);
  transform-origin: top right;
}
.dropdown-enter-from {
  transform: translateY(-6px) scale(0.95);
  opacity: 0.8;
}
.dropdown-leave-to {
  transform: translateY(-6px) scale(0.95);
  opacity: 0.8;
}
```

**Step 3：视觉验证**
- 点击 `···` 按钮 → 菜单从右上方弹出（scale + translateY）
- 点击菜单外 → 菜单收回

---

## Task 5：路由页面切换动画

**文件：**
- 修改：`src/layouts/MainLayout.vue`

**目标：** 主内容区页面切换时，新页面从右侧轻微滑入；返回时从左侧滑入

**设计选择：** Vue Router 无法自动检测前进/后退方向，这里用**单向进入**（统一从右 24px 滑入），不区分方向，简单可靠。

**Step 1：修改 MainLayout.vue 的 `<router-view>`（L48）**

将：
```html
<router-view />
```

改为：
```html
<RouterView v-slot="{ Component }">
  <Transition name="page">
    <component :is="Component" :key="$route.path" />
  </Transition>
</RouterView>
```

> `:key="$route.path"` 强制每次路由变化都触发 transition

**Step 2：在 `<style scoped>` 添加动画 CSS**

```css
/* 路由页面进入/离开动画 */
.page-enter-active {
  transition: var(--transition-route-in);
}
.page-leave-active {
  transition: var(--transition-route-out);
  position: absolute;
  width: 100%;
}
.page-enter-from {
  transform: translateX(20px);
  opacity: 0.85;
}
.page-leave-to {
  transform: translateX(-12px);
  opacity: 0.85;
}
```

> `position: absolute` 在 leave-active 阶段防止旧页面占位导致布局跳动

**Step 3：`.main-content` 需要 `position: relative` + `overflow: hidden`**

检查 MainLayout.vue 的 `.main-content` 样式（L179-L185），添加：

```css
.main-content {
  /* ... 已有样式 ... */
  position: relative;
  overflow: hidden;   /* 已有 overflow-y: auto，改为 hidden 防止滑出边界 */
}
```

> **风险警告**：`overflow: hidden` 会让内容区滚动失效。需要把 `overflow-y: auto` 移到各页面自己的根元素上，或者用 `overflow: clip` 仅裁切 X 轴。

**推荐方案：** 用 `overflow-x: clip; overflow-y: auto` 代替 `overflow: hidden`（现代浏览器支持，WebView2 支持）

```css
.main-content {
  overflow-x: clip;
  overflow-y: auto;
  position: relative;
}
```

**Step 4：视觉验证**
- 主页点击项目卡片 → 项目页从右轻滑入
- 项目页点击任务卡片 → 任务页从右轻滑入
- 点返回按钮 → 上一页出现（同方向，可接受）

---

## Task 6：StatusBar 番茄钟配置面板动画（加分项）

**文件：**
- 修改：`src/components/StatusBar.vue`

**目标：** 长按番茄钟弹出的配置面板（Teleport to body）加进入动画

**操作：** 搜索 StatusBar.vue 中控制配置面板显示的 `v-if`，用 `<Transition name="config-panel">` 包裹，加与弹窗相同的 `dialog-enter` 动画（复用 Task 1 的 keyframes）

> 此任务优先级低，可作为锦上添花

---

## 验证清单

| 场景 | 预期效果 |
|------|---------|
| 弹窗打开 | 从下方 16px + scale(0.97) 滑入，duration 500ms |
| 弹窗关闭 | 向下 8px + scale(0.97) 收出，duration 300ms |
| 侧边栏打开 | 纯 translateX(100%)→0，无 opacity 变化 |
| 侧边栏关闭 | 纯 translateX(0)→(100%)，无 opacity 变化 |
| 更多菜单展开 | scale(0.95)+translateY(-6px)→原位，duration 150ms |
| 更多菜单收起 | 反向，duration 150ms |
| 页面前进 | 新页面 translateX(20px)→0，旧页面 translateX(-12px) |
| opacity 范围 | 任何动画 opacity 起点不低于 0.8 |
