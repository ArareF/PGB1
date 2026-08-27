# Sharp Grid 全应用深色 UI 升级 Implementation Plan

> **For Codex:** REQUIRED SUB-SKILL: Use verification-before-completion to verify every delivery claim.

**Goal:** 将当前主页 Sharp Grid 试点收口为覆盖整个应用的统一深色 UI，同时保持功能与布局不变。

**Architecture:** 先把主题 token 提升到 `design-system.css` 的深色变量层，再把 `sharp-grid.css` 改为全局深色组件配方；通过公共类覆盖绝大多数界面，只对独立窗口和特殊页面补充明确选择器。主页移除试点状态与对比开关，深色主题直接渲染精装项目卡。

**Tech Stack:** Vue 3、TypeScript、CSS Variables、Tauri 2、Node 静态检查脚本。

---

### Task 1：建立主题回归检查

**Files:**
- Create: `scripts/check-sharp-grid-coverage.mjs`
- Modify: `package.json`

1. 写入会因当前“主页局部试点”而失败的检查。
2. 校验深色主题 SSOT、全局作用域、试点开关移除、关键页面/窗口覆盖和 reduced-motion。
3. 运行检查，确认失败原因来自尚未完成的全局迁移。

### Task 2：收口 DesignSystem 变量层

**Files:**
- Modify: `src/styles/design-system.css`
- Modify: `src/styles/sharp-grid.css`

1. 将 Sharp Grid token 整组迁入 `:root[data-theme="dark"]`。
2. 深色主题重定向既有语义变量，统一背景、边框、圆角、阴影、输入和按钮。
3. 删除 `sharp-grid.css` 中的局部 token 定义，确保变量只有一个来源。

### Task 3：移除主页试点状态

**Files:**
- Modify: `src/views/HomePage.vue`
- Modify: `src/components/ProjectCard.vue`

1. 删除 `pgb1-home-sharpgrid`、watch 和对比开关。
2. 使用当前主题决定项目卡 DOM 外观：深色为精装，浅色保持既有结构。
3. 保留项目卡事件、菜单操作和数据计算不变。

### Task 4：覆盖共享外壳与公共组件

**Files:**
- Modify: `src/styles/sharp-grid.css`

1. 覆盖 MainLayout、TitleBar、StatusBar、Sidebar、WindowControls。
2. 统一按钮、输入、select、Tab、chip、标签、进度条、滚动条、菜单。
3. 统一四类卡片、侧边栏、笔记、媒体预览和浏览器组件。
4. 统一所有 Dialog/Overlay，保留原有尺寸与位置。

### Task 5：覆盖全部页面与独立窗口

**Files:**
- Modify: `src/styles/sharp-grid.css`

1. 覆盖主流程、工作流、辅助浏览、时光机和设置页面。
2. 覆盖 ReminderPage、TranslatorPage、PinboardPage。
3. 补齐 focus-visible、disabled、loading、danger、drag-over、selected 状态。
4. 增加 `prefers-reduced-motion` 降级。

### Task 6：文档与索引快照

**Files:**
- Modify: `docs/新设计风格.md`
- Modify: `CODE_INDEX.md`
- Modify: `docs/code/styles-system.md`
- Modify: `docs/code/components.md`
- Modify: `docs/code/views.md`
- Modify: `INDEX.md`

1. 将状态从主页试点更新为全应用深色主题。
2. 记录浅色主题暂不升级及全局作用域规则。
3. 更新文件职责、行数和最近计划入口。

### Task 7：验证

1. 运行新增 Sharp Grid 覆盖检查。
2. 运行 `npm run check:all`。
3. 运行 `npm run build`。
4. 启动可预览构建，逐页巡检主应用和独立窗口可达界面。
5. 核对 Git Diff，只保留本任务必要修改并保护既有用户改动。

