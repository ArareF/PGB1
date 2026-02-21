# Phase 2: 三级页面导航 + Vue Router + 页面骨架

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 实现 Vue Router 路由系统，5 个页面的空壳骨架，标题栏/快捷功能/更多菜单根据当前页面动态变化，返回按钮逐级导航。

**Architecture:** Vue Router 4 嵌套路由，所有页面共享 MainLayout（标题栏+侧边栏+主功能区）。通过 `useNavigation` composable 管理页面级元数据（标题、返回路径、快捷功能按钮列表、更多菜单项列表），TitleBar 和更多菜单从该 composable 读取数据并响应式渲染。路由参数传递项目 ID 和任务 ID。

**Tech Stack:** Vue Router 4, Vue 3 Composition API (`<script setup>`), TypeScript

**前置条件:** Phase 1 已完成 — MainLayout、TitleBar、Sidebar、WindowControls、设计系统、主题切换均已就位。

**设计文档参考:**
- 页面架构: `design/界面设计.md` L83-102
- 各页面导航栏配置: `design/界面设计.md` L108-313, L652-757
- 交互设计-页面导航: `design/界面设计.md` L1807-1810
- UI 开发理念: `开发规范.md` — 「UI 开发理念」章节

---

## 总体路线图（Phase 2 在全局中的位置）

| 阶段 | 内容 | 状态 |
|------|------|------|
| **Phase 1** | 脚手架 + 设计系统 + 主窗口框架 | ✅ 已完成 |
| **Phase 2**（本文档） | 三级页面导航 + Vue Router + 页面骨架 | ✅ 已完成 |
| **Phase 3** | 卡片组件 + Rust 文件系统后端 | ⬜ |
| **Phase 4** | 双视图模式 + 侧边栏详情 | ⬜ |
| **Phase 5** | 文件拖拽 + 任务管理系统 | ⬜ |
| **Phase 6** | 日报打卡 + 翻译 + 转换进度悬浮窗 | ⬜ |

---

## 页面导航配置速查（来自设计文档）

后续 Task 实现会频繁查这个表，统一放在这里避免重复。

| 页面 | 标题 | 返回按钮 | 快捷功能区 | 更多菜单 |
|------|------|---------|-----------|---------|
| 主页 | "PGB1" | 无 | （无） | 日报打卡、程序设置 |
| 项目页 | "← 项目名称" | 返回主页 | 游戏介绍、项目素材、AE工程、任务列表 | 打开项目文件夹 |
| 任务页 | "← 任务名称" | 返回项目页 | 子任务 X/Y、规范化、缩放、转换、上传 | 打开任务文件夹、打开 nextcloud 文件夹 |
| 游戏介绍页 | "← 游戏介绍" | 返回项目页 | 打开文件夹 | 刷新 |
| 项目素材页 | "← 项目素材" | 返回项目页 | （无） | 刷新 |

---

## Phase 2 任务清单

### Task 1: 安装 Vue Router + 路由配置

**Files:**
- Modify: `package.json`（新增依赖）
- Create: `src/router/index.ts`（路由配置）
- Modify: `src/main.ts`（注册路由）

**Step 1: 安装 Vue Router 4**

```bash
cd D:/work/pgsoft/PGB1
npm install vue-router@4
```

**Step 2: 创建路由配置**

创建 `src/router/index.ts`：

```typescript
import { createRouter, createWebHistory } from 'vue-router'

const router = createRouter({
  history: createWebHistory(),
  routes: [
    {
      path: '/',
      name: 'home',
      component: () => import('../views/HomePage.vue'),
    },
    {
      path: '/project/:projectId',
      name: 'project',
      component: () => import('../views/ProjectPage.vue'),
    },
    {
      path: '/project/:projectId/task/:taskId',
      name: 'task',
      component: () => import('../views/TaskPage.vue'),
    },
    {
      path: '/project/:projectId/game-intro',
      name: 'gameIntro',
      component: () => import('../views/GameIntroPage.vue'),
    },
    {
      path: '/project/:projectId/materials',
      name: 'materials',
      component: () => import('../views/MaterialsPage.vue'),
    },
  ],
})

export default router
```

路由设计说明：
- 使用 `createWebHistory`（Tauri WebView 支持）
- 项目页和任务页带动态参数（`:projectId`, `:taskId`），后续 Phase 3 连接真实数据
- 辅助页面挂在项目路径下，返回时能取到 `projectId`
- 懒加载所有页面组件

**Step 3: 在 main.ts 中注册路由**

修改 `src/main.ts`：

```typescript
import { createApp } from 'vue'
import App from './App.vue'
import router from './router'

/* 样式引入顺序: reset → 设计系统 → 工具类 */
import './styles/reset.css'
import './styles/design-system.css'
import './styles/glass.css'

createApp(App).use(router).mount('#app')
```

**Step 4: 验证安装成功**

```bash
npm run tauri dev
```

Expected: 无报错启动（页面视图组件尚未创建，但路由本身不会报错因为是懒加载）。

---

### Task 2: 5 个页面骨架组件

**Files:**
- Create: `src/views/HomePage.vue`
- Create: `src/views/ProjectPage.vue`
- Create: `src/views/TaskPage.vue`
- Create: `src/views/GameIntroPage.vue`
- Create: `src/views/MaterialsPage.vue`

每个页面此阶段只需要占位内容，后续 Phase 逐步填充真实功能。

**Step 1: 创建 HomePage.vue**

```vue
<script setup lang="ts">
import { useRouter } from 'vue-router'

const router = useRouter()

/* 临时：模拟项目列表，Phase 3 替换为真实数据 */
const mockProjects = [
  { id: 'project-1', name: '217_RedDevil' },
  { id: 'project-2', name: '218_GoldenDragon' },
  { id: 'project-3', name: '219_CyberNinja' },
]

function openProject(projectId: string) {
  router.push({ name: 'project', params: { projectId } })
}
</script>

<template>
  <div class="home-page">
    <p class="page-hint">点击项目进入项目页</p>
    <div class="mock-card-grid">
      <button
        v-for="project in mockProjects"
        :key="project.id"
        class="mock-card glass-subtle"
        @click="openProject(project.id)"
      >
        {{ project.name }}
      </button>
    </div>
  </div>
</template>

<style scoped>
.home-page {
  display: flex;
  flex-direction: column;
  gap: var(--spacing-6);
}

.page-hint {
  font-size: var(--text-sm);
  color: var(--text-tertiary);
}

.mock-card-grid {
  display: flex;
  flex-wrap: wrap;
  gap: var(--gap-card);
}

.mock-card {
  width: var(--card-project-width);
  height: var(--card-project-height);
  display: flex;
  align-items: center;
  justify-content: center;
  border-radius: var(--card-border-radius);
  border: none;
  color: var(--text-primary);
  font-size: var(--text-xl);
  cursor: pointer;
  transition: var(--transition-card-hover);
}

.mock-card:hover {
  transform: translateY(var(--card-hover-lift));
  box-shadow: var(--card-shadow-hover);
}
</style>
```

**Step 2: 创建 ProjectPage.vue**

```vue
<script setup lang="ts">
import { useRoute, useRouter } from 'vue-router'

const route = useRoute()
const router = useRouter()

const projectId = route.params.projectId as string

/* 临时：模拟任务列表 */
const mockTasks = [
  { id: 'task-ambient', name: 'Ambient' },
  { id: 'task-freespin', name: 'Free Spin' },
  { id: 'task-prototype', name: 'Prototype' },
]

function openTask(taskId: string) {
  router.push({ name: 'task', params: { projectId, taskId } })
}
</script>

<template>
  <div class="project-page">
    <p class="page-hint">项目: {{ projectId }} — 点击任务进入任务页</p>
    <div class="mock-card-grid">
      <button
        v-for="task in mockTasks"
        :key="task.id"
        class="mock-card glass-subtle"
        @click="openTask(task.id)"
      >
        {{ task.name }}
      </button>
    </div>
  </div>
</template>

<style scoped>
.project-page {
  display: flex;
  flex-direction: column;
  gap: var(--spacing-6);
}

.page-hint {
  font-size: var(--text-sm);
  color: var(--text-tertiary);
}

.mock-card-grid {
  display: flex;
  flex-wrap: wrap;
  gap: var(--gap-card);
}

.mock-card {
  width: var(--card-task-width);
  height: var(--card-task-height);
  display: flex;
  align-items: center;
  justify-content: center;
  border-radius: var(--card-border-radius);
  border: none;
  color: var(--text-primary);
  font-size: var(--text-lg);
  cursor: pointer;
  transition: var(--transition-card-hover);
}

.mock-card:hover {
  transform: translateY(var(--card-hover-lift));
  box-shadow: var(--card-shadow-hover);
}
</style>
```

**Step 3: 创建 TaskPage.vue**

```vue
<script setup lang="ts">
import { useRoute } from 'vue-router'

const route = useRoute()
const projectId = route.params.projectId as string
const taskId = route.params.taskId as string
</script>

<template>
  <div class="task-page">
    <p class="page-hint">项目: {{ projectId }} / 任务: {{ taskId }}</p>
    <p class="page-placeholder">素材双视图区域（Phase 4 实现）</p>
  </div>
</template>

<style scoped>
.task-page {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  height: 100%;
  gap: var(--spacing-4);
}

.page-hint {
  font-size: var(--text-sm);
  color: var(--text-tertiary);
}

.page-placeholder {
  font-size: var(--text-xl);
  color: var(--text-tertiary);
}
</style>
```

**Step 4: 创建 GameIntroPage.vue**

```vue
<script setup lang="ts">
import { useRoute } from 'vue-router'

const route = useRoute()
const projectId = route.params.projectId as string
</script>

<template>
  <div class="game-intro-page">
    <p class="page-hint">项目: {{ projectId }}</p>
    <p class="page-placeholder">游戏介绍内容区（Phase 3 实现）</p>
  </div>
</template>

<style scoped>
.game-intro-page {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  height: 100%;
  gap: var(--spacing-4);
}

.page-hint {
  font-size: var(--text-sm);
  color: var(--text-tertiary);
}

.page-placeholder {
  font-size: var(--text-xl);
  color: var(--text-tertiary);
}
</style>
```

**Step 5: 创建 MaterialsPage.vue**

```vue
<script setup lang="ts">
import { useRoute } from 'vue-router'

const route = useRoute()
const projectId = route.params.projectId as string
</script>

<template>
  <div class="materials-page">
    <p class="page-hint">项目: {{ projectId }}</p>
    <p class="page-placeholder">项目素材内容区（Phase 3 实现）</p>
  </div>
</template>

<style scoped>
.materials-page {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  height: 100%;
  gap: var(--spacing-4);
}

.page-hint {
  font-size: var(--text-sm);
  color: var(--text-tertiary);
}

.page-placeholder {
  font-size: var(--text-xl);
  color: var(--text-tertiary);
}
</style>
```

---

### Task 3: useNavigation composable — 页面级元数据

**Files:**
- Create: `src/composables/useNavigation.ts`

这个 composable 是 TitleBar 和更多菜单的数据源。每个页面在 `setup` 中调用它来注册自己的导航配置，TitleBar 响应式读取。

**Step 1: 创建 useNavigation.ts**

```typescript
import { ref, readonly } from 'vue'
import type { Ref } from 'vue'

/* 快捷功能按钮定义 */
export interface NavAction {
  id: string
  label: string
  icon?: string      /* 预留：SVG 图标名，Phase 3+ 实现图标系统 */
  handler: () => void
  disabled?: boolean
}

/* 更多菜单项定义 */
export interface MoreMenuItem {
  id: string
  label: string
  handler: () => void
}

/* 导航状态 */
const title = ref('PGB1')
const showBackButton = ref(false)
const backHandler = ref<(() => void) | null>(null)
const actions = ref<NavAction[]>([])
const moreMenuItems = ref<MoreMenuItem[]>([])

export function useNavigation() {
  /**
   * 页面调用此方法注册导航配置。
   * 每次路由切换时由新页面重新调用，覆盖上一个页面的配置。
   */
  function setNavigation(config: {
    title: string
    showBackButton?: boolean
    onBack?: () => void
    actions?: NavAction[]
    moreMenuItems?: MoreMenuItem[]
  }) {
    title.value = config.title
    showBackButton.value = config.showBackButton ?? false
    backHandler.value = config.onBack ?? null
    actions.value = config.actions ?? []
    moreMenuItems.value = config.moreMenuItems ?? []
  }

  function goBack() {
    if (backHandler.value) {
      backHandler.value()
    }
  }

  return {
    /* 只读状态 — TitleBar 和更多菜单消费 */
    title: readonly(title) as Readonly<Ref<string>>,
    showBackButton: readonly(showBackButton) as Readonly<Ref<boolean>>,
    actions: readonly(actions) as Readonly<Ref<readonly NavAction[]>>,
    moreMenuItems: readonly(moreMenuItems) as Readonly<Ref<readonly MoreMenuItem[]>>,

    /* 写入方法 — 页面组件调用 */
    setNavigation,
    goBack,
  }
}
```

设计说明：
- 全局单例模式（模块级 ref），和 `useTheme` 一致
- 页面在 `onMounted` 或 `setup` 中调用 `setNavigation` 注册配置
- TitleBar 和更多菜单通过 `readonly` ref 消费数据
- `NavAction.handler` 由页面自己定义，TitleBar 只负责触发

---

### Task 4: 各页面注册导航配置

**Files:**
- Modify: `src/views/HomePage.vue`
- Modify: `src/views/ProjectPage.vue`
- Modify: `src/views/TaskPage.vue`
- Modify: `src/views/GameIntroPage.vue`
- Modify: `src/views/MaterialsPage.vue`

**参考:** 上方「页面导航配置速查」表。

**Step 1: HomePage 注册导航**

在 `HomePage.vue` 的 `<script setup>` 中添加：

```typescript
import { useNavigation } from '../composables/useNavigation'
import { useTheme } from '../composables/useTheme'

const { setNavigation } = useNavigation()
const { toggleTheme } = useTheme()

setNavigation({
  title: 'PGB1',
  showBackButton: false,
  actions: [],
  moreMenuItems: [
    { id: 'attendance', label: '日报打卡', handler: () => { /* Phase 6 */ } },
    { id: 'settings', label: '程序设置', handler: () => { /* Phase 6 */ } },
    { id: 'theme', label: '切换主题', handler: toggleTheme },
  ],
})
```

**Step 2: ProjectPage 注册导航**

```typescript
import { useNavigation } from '../composables/useNavigation'
import { useTheme } from '../composables/useTheme'

const { setNavigation } = useNavigation()
const { toggleTheme } = useTheme()

setNavigation({
  title: projectId,
  showBackButton: true,
  onBack: () => router.push({ name: 'home' }),
  actions: [
    { id: 'game-intro', label: '游戏介绍', handler: () => router.push({ name: 'gameIntro', params: { projectId } }) },
    { id: 'materials', label: '项目素材', handler: () => router.push({ name: 'materials', params: { projectId } }) },
    { id: 'ae-project', label: 'AE工程', handler: () => { /* Phase 5: 启动 AE */ } },
    { id: 'task-list', label: '任务列表', handler: () => { /* Phase 5: 弹出任务管理窗口 */ } },
  ],
  moreMenuItems: [
    { id: 'open-folder', label: '打开项目文件夹', handler: () => { /* Phase 3: Rust 命令 */ } },
    { id: 'theme', label: '切换主题', handler: toggleTheme },
  ],
})
```

**Step 3: TaskPage 注册导航**

```typescript
import { useNavigation } from '../composables/useNavigation'
import { useTheme } from '../composables/useTheme'

const { setNavigation } = useNavigation()
const { toggleTheme } = useTheme()

setNavigation({
  title: taskId,
  showBackButton: true,
  onBack: () => router.push({ name: 'project', params: { projectId } }),
  actions: [
    { id: 'subtasks', label: '子任务 0/0', handler: () => { /* Phase 5 */ }, disabled: true },
    { id: 'normalize', label: '规范化', handler: () => { /* Phase 5 */ } },
    { id: 'scale', label: '缩放', handler: () => { /* Phase 5 */ } },
    { id: 'convert', label: '转换', handler: () => { /* Phase 5 */ } },
    { id: 'upload', label: '上传', handler: () => { /* Phase 5 */ } },
  ],
  moreMenuItems: [
    { id: 'open-task-folder', label: '打开任务文件夹', handler: () => { /* Phase 3 */ } },
    { id: 'open-nextcloud', label: '打开 nextcloud 文件夹', handler: () => { /* Phase 3 */ } },
    { id: 'theme', label: '切换主题', handler: toggleTheme },
  ],
})
```

注意：TaskPage 需要 `useRouter` 来实现 `onBack`。

**Step 4: GameIntroPage 注册导航**

```typescript
import { useNavigation } from '../composables/useNavigation'
import { useTheme } from '../composables/useTheme'

const { setNavigation } = useNavigation()
const { toggleTheme } = useTheme()

setNavigation({
  title: '游戏介绍',
  showBackButton: true,
  onBack: () => router.push({ name: 'project', params: { projectId } }),
  actions: [
    { id: 'open-folder', label: '打开文件夹', handler: () => { /* Phase 3 */ } },
  ],
  moreMenuItems: [
    { id: 'refresh', label: '刷新', handler: () => { /* Phase 3 */ } },
    { id: 'theme', label: '切换主题', handler: toggleTheme },
  ],
})
```

**Step 5: MaterialsPage 注册导航**

```typescript
import { useNavigation } from '../composables/useNavigation'
import { useTheme } from '../composables/useTheme'

const { setNavigation } = useNavigation()
const { toggleTheme } = useTheme()

setNavigation({
  title: '项目素材',
  showBackButton: true,
  onBack: () => router.push({ name: 'project', params: { projectId } }),
  actions: [],
  moreMenuItems: [
    { id: 'refresh', label: '刷新', handler: () => { /* Phase 3 */ } },
    { id: 'theme', label: '切换主题', handler: toggleTheme },
  ],
})
```

---

### Task 5: TitleBar 改造 — 消费 useNavigation

**Files:**
- Modify: `src/components/TitleBar.vue`

TitleBar 从硬编码的静态内容改为从 `useNavigation` 读取数据并响应式渲染。

**Step 1: 改造 TitleBar.vue**

```vue
<script setup lang="ts">
import { useNavigation } from '../composables/useNavigation'

const { title, showBackButton, actions, goBack } = useNavigation()
</script>

<template>
  <header class="title-bar" data-tauri-drag-region>
    <!-- 左侧悬浮岛：返回按钮 + 标题 -->
    <div class="title-bar-left glass-medium">
      <button
        v-if="showBackButton"
        class="back-btn"
        title="返回"
        @click="goBack"
      >
        <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <polyline points="15 18 9 12 15 6" />
        </svg>
      </button>
      <span class="title-text">{{ title }}</span>
    </div>

    <!-- 中部悬浮岛：快捷功能区 -->
    <div v-if="actions.length > 0" class="title-bar-center glass-medium">
      <button
        v-for="action in actions"
        :key="action.id"
        class="action-btn"
        :title="action.label"
        :disabled="action.disabled"
        @click="action.handler"
      >
        {{ action.label }}
      </button>
    </div>
    <!-- 无快捷功能时：空占位保持布局 -->
    <div v-else class="title-bar-center glass-medium">
      <span class="placeholder-text">快捷功能区</span>
    </div>
  </header>
</template>

<style scoped>
.title-bar {
  display: flex;
  align-items: stretch;
  gap: var(--spacing-8);
  padding: 0;
  height: 100%;
  -webkit-app-region: drag;
}

.title-bar-left,
.title-bar-center {
  -webkit-app-region: no-drag;
}

.title-bar-left {
  display: flex;
  align-items: center;
  gap: var(--spacing-2);
  padding: var(--spacing-2) var(--spacing-8);
  border-radius: var(--floating-navbar-radius);
  flex-shrink: 0;
}

.title-text {
  font-size: var(--text-3xl);
  font-weight: var(--font-weight-body);
  color: var(--text-primary);
}

.back-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  width: var(--button-sm-height);
  height: var(--button-sm-height);
  border: none;
  background: transparent;
  color: var(--text-secondary);
  border-radius: var(--radius-md);
  cursor: pointer;
  transition: var(--transition-all);
  flex-shrink: 0;
}

.back-btn:hover {
  background: var(--bg-hover);
  color: var(--text-primary);
}

.title-bar-center {
  display: flex;
  align-items: center;
  gap: var(--spacing-3);
  padding: var(--spacing-2) var(--spacing-4);
  border-radius: var(--floating-navbar-radius);
  flex: 1;
  min-width: 0;
  margin-right: var(--spacing-5);
  overflow-x: auto;
}

/* 隐藏快捷功能区的滚动条（保持简洁） */
.title-bar-center::-webkit-scrollbar {
  display: none;
}

.action-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  padding: var(--spacing-1) var(--spacing-3);
  border: none;
  background: transparent;
  color: var(--text-secondary);
  font-size: var(--text-sm);
  border-radius: var(--radius-md);
  cursor: pointer;
  transition: var(--transition-all);
  white-space: nowrap;
  flex-shrink: 0;
}

.action-btn:hover:not(:disabled) {
  background: var(--bg-hover);
  color: var(--text-primary);
}

.action-btn:disabled {
  opacity: var(--button-disabled-opacity);
  cursor: not-allowed;
}

.placeholder-text {
  font-size: var(--text-xl);
  color: var(--text-tertiary);
}
</style>
```

---

### Task 6: 更多菜单改造 — 消费 useNavigation

**Files:**
- Modify: `src/layouts/MainLayout.vue`

更多菜单从硬编码的主题切换改为从 `useNavigation.moreMenuItems` 读取。

**Step 1: 改造 MainLayout.vue**

`<script setup>` 中：
- 移除 `useTheme` import（主题切换现在由各页面通过 moreMenuItems 注入）
- 从 `useNavigation` 读取 `moreMenuItems`

```typescript
import { ref } from 'vue'
import TitleBar from '../components/TitleBar.vue'
import Sidebar from '../components/Sidebar.vue'
import WindowControls from '../components/WindowControls.vue'
import { useNavigation } from '../composables/useNavigation'

const { moreMenuItems } = useNavigation()
const showMoreMenu = ref(false)
```

`<template>` 中更多菜单下拉部分改为：

```html
<!-- 下拉菜单 -->
<div v-if="showMoreMenu" class="more-dropdown glass-strong" @click="showMoreMenu = false">
  <button
    v-for="item in moreMenuItems"
    :key="item.id"
    class="dropdown-item"
    @click="item.handler"
  >
    {{ item.label }}
  </button>
</div>
```

**Step 2: 接入 router-view**

主功能区从占位内容替换为路由出口：

```html
<main class="main-content glass-medium">
  <router-view />
</main>
```

删除 `<template>` 中的占位 `<div class="placeholder">` 及其 `<style>` 中对应的 `.placeholder` 样式。

---

### Task 7: Phase 2 收尾验证

**检查清单:**

- [ ] `npm run tauri dev` 正常启动，无报错
- [ ] 主页显示 3 个模拟项目卡片
- [ ] 点击项目卡片 → 进入项目页，标题变为项目名，返回按钮出现
- [ ] 项目页快捷功能区显示 4 个按钮（游戏介绍/项目素材/AE工程/任务列表）
- [ ] 点击"游戏介绍" → 进入游戏介绍页，标题变为"游戏介绍"
- [ ] 点击"项目素材" → 进入项目素材页，标题变为"项目素材"
- [ ] 点击任务卡片 → 进入任务页，标题变为任务名，快捷功能区显示 5 个按钮
- [ ] 所有页面的返回按钮 → 正确返回上一级
- [ ] 更多菜单内容随页面变化（主页有日报打卡/程序设置，项目页有打开项目文件夹，等等）
- [ ] 所有页面的更多菜单都有"切换主题"选项，点击正常切换
- [ ] 明暗主题在所有页面正常显示
- [ ] CSS 零硬编码，所有样式值引用 design-system token

**Step 1: 全流程点击测试**

```bash
npm run tauri dev
```

逐项确认检查清单。

**Step 2: 修复问题后验证通过**

如有问题，定位修复后重新验证。

---

## 设计文档参考速查

| 需要查什么 | 去哪里看 |
|-----------|---------|
| 页面架构和导航逻辑 | `design/界面设计.md` L83-102 |
| 主页导航栏配置 | `design/界面设计.md` L108-127 |
| 项目页导航栏配置 | `design/界面设计.md` L274-301 |
| 任务页导航栏配置 | `design/界面设计.md` L304-313 |
| 游戏介绍页导航栏 | `design/界面设计.md` L652-666 |
| 项目素材页导航栏 | `design/界面设计.md` L695-709 |
| 交互设计-页面导航 | `design/界面设计.md` L1807-1810 |
| UI 开发理念 | `开发规范.md` — 「UI 开发理念」章节 |
| CSS Token 速查 | `开发规范.md` — 「当前 Token 速查」表 |

---

## Phase 2 实施记录

**完成时间:** 2026-02-17

### 已完成

- ✅ Task 1-7 全部完成，TypeScript 零报错，Vite 构建通过
- ✅ 5 个页面骨架 + useNavigation composable + 动态 TitleBar/更多菜单
- ✅ 全流程页面导航可用（主页→项目→任务→辅助页，返回按钮正常）

### UI 调整（计划外，产品总监现场审核）

- 快捷功能按钮：颜色加深（text-primary）、字号 text-2xl、背景 bg-hover、圆角 radius-button、右对齐、容器按内容收缩
- 标题栏高度：去掉 height:100%，底边对齐右侧控制区，顶部自然留白
- 返回按钮：撑满容器高度（aspect-ratio:1），左圆角=容器圆角，右圆角=0，常显深色背景
- 控制按钮与更多菜单间距调整（spacing-3），更多菜单 margin-top:auto 推底
- 项目页标题加"项目："前缀，page-hint 字号放大到 text-2xl

### UI 二次调整（同日后续会话）

- 标题浮动岛高度固定为 `--floating-navbar-height`（100px），所有页面统一
- 快捷功能区比标题矮（`margin-top: --spacing-3` 出血），底边对齐
- 快捷功能区按钮 padding 调优（上下 spacing-2，左右 spacing-4）
- 标题字号从 `--text-3xl` 放大到 `--text-4xl`
- 项目页功能区标题从"项目: xxx"改为"制作任务"，白色 + `--text-3xl` + 下划线分隔
- 主页功能区标题样式与项目页统一
- 窗口控制区 `align-self: stretch` 使按钮贴近顶部边缘
- 设计系统 `--floating-navbar-height` 从 60px 更新为 100px
- 界面设计文档同步更新

### 遗留问题

无
