# 排序 & 优先度 Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 为「我的项目」页添加截止日期/优先度排序；为「制作任务」页添加优先度排序；两页卡片均显示带颜色的优先度标签；TaskCard 新增右上角 ··· 菜单（内含优先度设置）。

**Architecture:** 优先度数据存入 `.pgb1_project.json`（项目优先度 = `priority` 字段，任务优先度 = `task_priorities` Map）。Rust 后端新增 2 个命令（set_project_priority / set_task_priority），并在 scan_projects / scan_tasks 中把优先度下发给前端 DTO。前端卡片菜单顶部内联优先度选择器（行内 pill 按钮，click 即写入无需弹窗），页面用 computed 对列表排序。

**Tech Stack:** Tauri 2 (Rust) + Vue 3 + vue-i18n@10 + CSS 变量 SSOT

---

## 数据约定

| 优先度值 | 中文标签 | 颜色语义 |
|----------|---------|---------|
| `"high"` | 高 | danger（红） |
| `"medium"` | 中 | warning（橙） |
| `"low"` | 低 | success（绿） |
| `null` / 不存在 | 无标签 | — |

---

## Task 1: Rust 数据模型扩展

**Files:**
- Modify: `src-tauri/src/models.rs`

### Step 1: 在 ProjectConfig 加两个字段

在 `default_ae_file` 字段之后插入：

```rust
/// 项目优先度（"high" / "medium" / "low"），null 表示无
#[serde(default)]
pub priority: Option<String>,
/// 任务优先度 Map（task_name_lower → priority）
#[serde(default)]
pub task_priorities: std::collections::HashMap<String, String>,
```

### Step 2: 在 ProjectInfo 加优先度字段

在 `app_icon` 字段之后插入：

```rust
/// 项目优先度（来自 ProjectConfig.priority）
pub priority: Option<String>,
```

### Step 3: 在 TaskInfo 加优先度字段

在 `video_uploaded` 字段之后插入：

```rust
/// 任务优先度（来自项目 .pgb1_project.json 的 task_priorities）
pub priority: Option<String>,
```

### Step 4: 验证编译

```bash
cd "C:\work\PG Butler\PGB1\src-tauri"
cargo check 2>&1
```

期望：无 error（可能有 unused import warning，忽略）

### Step 5: Commit

```bash
git add src-tauri/src/models.rs
git commit -m "feat(models): 新增 priority / task_priorities 字段到 ProjectConfig/ProjectInfo/TaskInfo"
```

---

## Task 2: Rust 扫描命令更新

**Files:**
- Modify: `src-tauri/src/commands.rs`（scan_projects 约 L85 + scan_tasks 约 L159）

### Step 1: 在 scan_projects 的 ProjectInfo 构造中传 priority

找到约 L85 的 `projects.push(ProjectInfo {` 块，在 `completed_tasks,` 行之后插入：

```rust
priority: config.priority.clone(),
```

### Step 2: 在 scan_tasks 中读取 task_priorities

在 `scan_tasks` 函数体内，`let mut tasks = Vec::new();` 之后、`let entries = fs::read_dir(&export_path)` 之前插入：

```rust
// 读取项目配置，获取任务优先度 Map
let task_priorities: std::collections::HashMap<String, String> = {
    let config_path = project_dir.join(".pgb1_project.json");
    fs::read_to_string(&config_path)
        .ok()
        .and_then(|s| {
            #[derive(serde::Deserialize, Default)]
            struct PriorityOnly {
                #[serde(default)]
                task_priorities: std::collections::HashMap<String, String>,
            }
            serde_json::from_str::<PriorityOnly>(&s).ok()
        })
        .map(|c| c.task_priorities)
        .unwrap_or_default()
};
```

### Step 3: 在 TaskInfo 构造中传 priority

找到约 L159 的 `tasks.push(TaskInfo {` 块，在 `video_uploaded,` 行之后插入：

```rust
priority: task_priorities.get(&task_name.to_lowercase()).cloned(),
```

> **注意**：key 用 `task_name.to_lowercase()`，与 enabled_tasks 命名约定对齐。

### Step 4: 验证编译

```bash
cd "C:\work\PG Butler\PGB1\src-tauri"
cargo check 2>&1
```

期望：no errors

### Step 5: Commit

```bash
git add src-tauri/src/commands.rs
git commit -m "feat(commands): scan_projects/scan_tasks 携带 priority 字段"
```

---

## Task 3: Rust 新增两个写入命令

**Files:**
- Modify: `src-tauri/src/commands.rs`（在文件末尾 `send_ctrl_end` 辅助函数之前追加）

### Step 1: 添加 set_project_priority 命令

```rust
/// 设置项目优先度（"high"/"medium"/"low" 或 null 清除）
#[tauri::command]
pub fn set_project_priority(project_path: String, priority: Option<String>) -> Result<(), String> {
    use std::fs;
    let config_path = std::path::Path::new(&project_path).join(".pgb1_project.json");
    let content = fs::read_to_string(&config_path)
        .map_err(|e| format!("读取配置失败: {}", e))?;
    let mut config: crate::models::ProjectConfig = serde_json::from_str(&content)
        .map_err(|e| format!("解析配置失败: {}", e))?;
    config.priority = priority;
    let json = serde_json::to_string_pretty(&config)
        .map_err(|e| format!("序列化失败: {}", e))?;
    fs::write(&config_path, json).map_err(|e| format!("写入失败: {}", e))?;
    Ok(())
}
```

### Step 2: 添加 set_task_priority 命令

```rust
/// 设置任务优先度（"high"/"medium"/"low" 或 null 清除）
#[tauri::command]
pub fn set_task_priority(project_path: String, task_name: String, priority: Option<String>) -> Result<(), String> {
    use std::fs;
    let config_path = std::path::Path::new(&project_path).join(".pgb1_project.json");
    let content = fs::read_to_string(&config_path)
        .map_err(|e| format!("读取配置失败: {}", e))?;
    let mut config: crate::models::ProjectConfig = serde_json::from_str(&content)
        .map_err(|e| format!("解析配置失败: {}", e))?;
    let key = task_name.to_lowercase();
    match priority {
        Some(p) => { config.task_priorities.insert(key, p); }
        None    => { config.task_priorities.remove(&key); }
    }
    let json = serde_json::to_string_pretty(&config)
        .map_err(|e| format!("序列化失败: {}", e))?;
    fs::write(&config_path, json).map_err(|e| format!("写入失败: {}", e))?;
    Ok(())
}
```

### Step 3: 在 lib.rs 注册两个新命令

在 `commands::get_file_mtime,` 之前插入：

```rust
commands::set_project_priority,
commands::set_task_priority,
```

### Step 4: 验证编译

```bash
cd "C:\work\PG Butler\PGB1\src-tauri"
cargo check 2>&1
```

期望：no errors

### Step 5: Commit

```bash
git add src-tauri/src/commands.rs src-tauri/src/lib.rs
git commit -m "feat(commands): 新增 set_project_priority / set_task_priority 命令"
```

---

## Task 4: CSS 优先度颜色 Token

**Files:**
- Modify: `src/styles/design-system.css`

### Step 1: 添加 Priority 语义色变量

在 design-system.css 的暗色主题块内，找到 `--progress-outdated` 附近的进度/标签语义变量区，在其后追加（暗色主题块 `.dark` 内也同步）：

> **注意**：当前项目只有暗色主题（`.dark` 类），在 `:root` 添加即可（无明色主题需同步）。

在 `:root` 的标签相关变量区（`--tag-status-*` 附近）之后插入：

```css
/* Priority 优先度标签 */
--priority-high-bg:     color-mix(in srgb, var(--color-danger) 18%, transparent);
--priority-high-text:   var(--color-danger-light);
--priority-high-active: color-mix(in srgb, var(--color-danger) 35%, transparent);

--priority-medium-bg:   color-mix(in srgb, var(--color-warning) 18%, transparent);
--priority-medium-text: var(--color-warning-light);
--priority-medium-active: color-mix(in srgb, var(--color-warning) 35%, transparent);

--priority-low-bg:      color-mix(in srgb, var(--color-success) 18%, transparent);
--priority-low-text:    var(--color-success-light);
--priority-low-active:  color-mix(in srgb, var(--color-success) 35%, transparent);
```

### Step 2: Commit

```bash
git add src/styles/design-system.css
git commit -m "feat(design-system): 新增 priority 标签颜色语义变量"
```

---

## Task 5: i18n 新增 priority 相关 key

**Files:**
- Modify: `src/locales/zh-CN.ts`
- Modify: `src/locales/en.ts`

### Step 1: zh-CN.ts — 在 home 对象后追加 priority namespace

```typescript
priority: {
  high: '高',
  medium: '中',
  low: '低',
  clearPriority: '清除优先度',
  setPriority: '设置优先度',
},
```

同时在 `home` namespace 中追加排序相关 key：

```typescript
sortDefault: '默认',
sortDeadline: '截止日期',
sortPriority: '优先度',
```

在 `project` namespace 中追加：

```typescript
sortDefault: '默认',
sortPriority: '优先度',
```

### Step 2: en.ts — 对齐相同结构

```typescript
priority: {
  high: 'High',
  medium: 'Med',
  low: 'Low',
  clearPriority: 'Clear',
  setPriority: 'Priority',
},
```

home 下：
```typescript
sortDefault: 'Default',
sortDeadline: 'Deadline',
sortPriority: 'Priority',
```

project 下：
```typescript
sortDefault: 'Default',
sortPriority: 'Priority',
```

### Step 3: 验证 TypeScript

```bash
cd "C:\work\PG Butler\PGB1"
npx vue-tsc --noEmit 2>&1
```

期望：0 errors

### Step 4: Commit

```bash
git add src/locales/zh-CN.ts src/locales/en.ts
git commit -m "feat(i18n): 新增 priority / sort 相关国际化 key"
```

---

## Task 6: 前端 TS 接口更新

**Files:**
- Modify: `src/composables/useProjects.ts`
- Modify: `src/composables/useTasks.ts`

### Step 1: useProjects.ts — 在 ProjectInfo 末尾加字段

在 `app_icon: string | null` 之后加：

```typescript
priority: string | null
```

### Step 2: useTasks.ts — 在 TaskInfo 末尾加字段

在 `video_uploaded: number` 之后加：

```typescript
priority: string | null
```

### Step 3: 验证 TypeScript

```bash
npx vue-tsc --noEmit 2>&1
```

期望：0 errors（如有 ProjectCard/TaskCard 的 priority 相关 TS 错误，是 Task 7/8 的工作，属正常）

### Step 4: Commit

```bash
git add src/composables/useProjects.ts src/composables/useTasks.ts
git commit -m "feat(composables): ProjectInfo/TaskInfo 接口新增 priority 字段"
```

---

## Task 7: ProjectCard — 优先度标签 + 菜单优先度选择器

**Files:**
- Modify: `src/components/ProjectCard.vue`

### Step 1: 在 `<script setup>` 顶部添加 invoke import 和 priority 工具

在现有 import 末尾追加：

```typescript
import { invoke } from '@tauri-apps/api/core'

// 优先度设置（直接调用，完成后 emit refresh）
async function setPriority(value: string | null) {
  showMenu.value = false
  await invoke('set_project_priority', { projectPath: props.project.path, priority: value })
  emit('refresh')
}
```

### Step 2: 修改 defineEmits 类型，增加 refresh 事件

将原：
```typescript
defineEmits<{
  click: [project: ProjectInfo]
  action: [project: ProjectInfo, action: 'rename' | 'deadline' | 'delete']
}>()
```
改为：
```typescript
const emit = defineEmits<{
  click: [project: ProjectInfo]
  action: [project: ProjectInfo, action: 'rename' | 'deadline' | 'delete']
  refresh: []
}>()
```

> **注意**：同时删除原 `defineEmits<{...}>()` 前没有 const emit = 的写法，确保 emit 有变量名。

### Step 3: 在 template 中，card-info 里加优先度标签

在 `<div class="card-info">` 内，`<span class="card-name">` 之前插入：

```html
<div class="card-name-row">
  <span
    v-if="project.priority"
    class="priority-tag"
    :class="`priority-tag--${project.priority}`"
  >{{ $t(`priority.${project.priority}`) }}</span>
  <span class="card-name">{{ project.name }}</span>
</div>
```

同时将原 `<span class="card-name">{{ project.name }}</span>` 删除（已移入 card-name-row）。

### Step 4: 在卡片菜单顶部加优先度选择器行

在 `<div v-if="showMenu" class="card-menu glass-medium" ...>` 内，第一个 `<button class="menu-item">` 之前插入：

```html
<!-- 优先度选择器 -->
<div class="menu-priority-section">
  <span class="menu-priority-label">{{ $t('priority.setPriority') }}</span>
  <div class="menu-priority-pills">
    <button
      v-for="p in ['high', 'medium', 'low']"
      :key="p"
      class="priority-pill"
      :class="[`priority-pill--${p}`, { 'is-active': project.priority === p }]"
      @mousedown.prevent="setPriority(p)"
    >{{ $t(`priority.${p}`) }}</button>
    <button
      v-if="project.priority"
      class="priority-pill priority-pill--clear"
      @mousedown.prevent="setPriority(null)"
    >✕</button>
  </div>
</div>
<div class="menu-divider" />
```

### Step 5: 在 scoped `<style>` 中添加优先度标签 + 菜单样式

```css
/* 名称行（标签 + 名字横排） */
.card-name-row {
  display: flex;
  align-items: center;
  gap: var(--spacing-1);
  min-width: 0;
}

.card-name-row .card-name {
  /* 继承原 card-name 样式，无需重写 */
}

/* Priority 标签（小胶囊） */
.priority-tag {
  flex-shrink: 0;
  display: inline-flex;
  align-items: center;
  height: 18px;
  padding: 0 6px;
  font-size: 11px;
  font-weight: var(--font-semibold);
  border-radius: var(--radius-tag);
  letter-spacing: 0.02em;
}

.priority-tag--high   { background: var(--priority-high-bg);   color: var(--priority-high-text); }
.priority-tag--medium { background: var(--priority-medium-bg); color: var(--priority-medium-text); }
.priority-tag--low    { background: var(--priority-low-bg);    color: var(--priority-low-text); }
```

在全局 `<style>` 块（非 scoped，菜单 Teleport 到 body）追加：

```css
/* 优先度选择器区域 */
.menu-priority-section {
  display: flex;
  flex-direction: column;
  gap: var(--spacing-1);
  padding: var(--spacing-2) var(--spacing-3) var(--spacing-2);
}

.menu-priority-label {
  font-size: var(--text-xs);
  color: var(--text-tertiary);
}

.menu-priority-pills {
  display: flex;
  gap: var(--spacing-1);
}

.priority-pill {
  flex: 1;
  height: 24px;
  border: none;
  border-radius: var(--radius-tag);
  font-size: 11px;
  font-family: inherit;
  font-weight: var(--font-semibold);
  cursor: pointer;
  transition: all var(--duration-fast) var(--ease-out);
}

.priority-pill--high   { background: var(--priority-high-bg);   color: var(--priority-high-text); }
.priority-pill--medium { background: var(--priority-medium-bg); color: var(--priority-medium-text); }
.priority-pill--low    { background: var(--priority-low-bg);    color: var(--priority-low-text); }

.priority-pill--high.is-active   { background: var(--priority-high-active); }
.priority-pill--medium.is-active { background: var(--priority-medium-active); }
.priority-pill--low.is-active    { background: var(--priority-low-active); }

.priority-pill--clear {
  flex: 0 0 24px;
  background: var(--bg-hover);
  color: var(--text-secondary);
}
.priority-pill--clear:hover { background: var(--bg-active); }

.menu-divider {
  height: 1px;
  background: var(--border-light);
  margin: 0 var(--spacing-2);
}
```

### Step 6: 验证 TypeScript

```bash
npx vue-tsc --noEmit 2>&1
```

### Step 7: Commit

```bash
git add src/components/ProjectCard.vue
git commit -m "feat(ProjectCard): 优先度标签展示 + 菜单内联优先度选择器"
```

---

## Task 8: TaskCard — 优先度标签 + 右上角 ··· 菜单

**Files:**
- Modify: `src/components/TaskCard.vue`

**概述：** TaskCard 需要完整实现 ProjectCard 同款的 ··· 菜单机制（Teleport to body，position:fixed + 动态坐标）。TaskCard 自己调用 Tauri 命令需要 project_path，但 TaskInfo 中没有该字段，所以改为 emit action 让 ProjectPage 处理。

### Step 1: 重写 script setup

将原有 script 替换为：

```typescript
<script setup lang="ts">
import { computed, ref, nextTick } from 'vue'
import { useI18n } from 'vue-i18n'
import type { TaskInfo } from '../composables/useTasks'

const { t } = useI18n()

interface SubtaskProgress {
  completed: number
  total: number
}

const props = defineProps<{
  task: TaskInfo
  subtaskProgress?: SubtaskProgress
}>()

const emit = defineEmits<{
  click: [task: TaskInfo]
  action: [task: TaskInfo, action: 'priority', value: string | null]
}>()

// 菜单控制
const showMenu = ref(false)
const menuBtnRef = ref<HTMLElement | null>(null)
const menuStyle = ref({ top: '0px', right: '0px' })

async function toggleMenu() {
  showMenu.value = !showMenu.value
  if (showMenu.value) {
    await nextTick()
    if (menuBtnRef.value) {
      const rect = menuBtnRef.value.getBoundingClientRect()
      menuStyle.value = {
        top: `${rect.bottom + 4}px`,
        right: `${window.innerWidth - rect.right}px`,
      }
    }
  }
}

function setPriority(value: string | null) {
  showMenu.value = false
  emit('action', props.task, 'priority', value)
}

// —— 原有 statusInfo / filesAllUploaded / formatSize 逻辑保持不变 ——

/** 文件上传是否全部完成 */
function filesAllUploaded(): boolean {
  const { material_total: mTotal, material_uploaded: mUploaded, video_total: vTotal, video_uploaded: vUploaded } = props.task
  const materialsOk = mTotal === 0 || mUploaded >= mTotal
  const videosOk = vTotal === 0 || vUploaded >= vTotal
  return materialsOk && videosOk
}

const statusInfo = computed(() => {
  const p = props.subtaskProgress
  if (p && p.total > 0) {
    if (p.completed >= p.total && filesAllUploaded()) {
      return { label: t('taskCard.completed'), cls: 'status-completed' }
    }
    if (p.completed > 0) {
      return { label: `${t('taskCard.inProgress')} ${p.completed}/${p.total}`, cls: 'status-wip' }
    }
    return { label: `${t('taskCard.notStarted')} 0/${p.total}`, cls: 'status-pending' }
  }
  const { material_total: total, material_uploaded: uploaded } = props.task
  if (total > 0 && uploaded >= total && filesAllUploaded()) {
    return { label: t('taskCard.completed'), cls: 'status-completed' }
  }
  if (total > 0) {
    return { label: t('taskCard.inProgress'), cls: 'status-wip' }
  }
  return { label: t('taskCard.notStarted'), cls: 'status-pending' }
})

function formatSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(1)} GB`
}
</script>
```

### Step 2: 重写 template

```html
<template>
  <button
    class="task-card glass-subtle"
    @click="$emit('click', task)"
  >
    <!-- 优先度标签 + 名称 -->
    <div class="task-name-row">
      <span
        v-if="task.priority"
        class="priority-tag"
        :class="`priority-tag--${task.priority}`"
      >{{ $t(`priority.${task.priority}`) }}</span>
      <span class="task-name">{{ task.name }}</span>
    </div>

    <div class="task-bottom">
      <span class="status-tag" :class="statusInfo.cls">{{ statusInfo.label }}</span>
      <span class="task-size">{{ formatSize(task.size_bytes) }}</span>
    </div>

    <!-- ··· 菜单按钮 -->
    <button
      ref="menuBtnRef"
      class="card-menu-btn"
      :class="{ visible: showMenu }"
      @click.stop="toggleMenu"
      @blur="showMenu = false"
    >
      ···
    </button>
  </button>

  <!-- 下拉菜单 — Teleport to body -->
  <Teleport to="body">
    <Transition name="card-menu">
      <div v-if="showMenu" class="card-menu glass-medium" :style="menuStyle" @click.stop>
        <!-- 优先度选择器 -->
        <div class="menu-priority-section">
          <span class="menu-priority-label">{{ $t('priority.setPriority') }}</span>
          <div class="menu-priority-pills">
            <button
              v-for="p in ['high', 'medium', 'low']"
              :key="p"
              class="priority-pill"
              :class="[`priority-pill--${p}`, { 'is-active': task.priority === p }]"
              @mousedown.prevent="setPriority(p)"
            >{{ $t(`priority.${p}`) }}</button>
            <button
              v-if="task.priority"
              class="priority-pill priority-pill--clear"
              @mousedown.prevent="setPriority(null)"
            >✕</button>
          </div>
        </div>
      </div>
    </Transition>
  </Teleport>
</template>
```

### Step 3: 更新 scoped style

在原有 style 基础上添加（不删除原有样式）：

```css
/* 名称行 */
.task-name-row {
  display: flex;
  align-items: center;
  gap: var(--spacing-1);
  min-width: 0;
  overflow: hidden;
}

/* 优先度标签（与 ProjectCard 共享同名类但 scoped） */
.priority-tag {
  flex-shrink: 0;
  display: inline-flex;
  align-items: center;
  height: 18px;
  padding: 0 6px;
  font-size: 11px;
  font-weight: var(--font-semibold);
  border-radius: var(--radius-tag);
}

.priority-tag--high   { background: var(--priority-high-bg);   color: var(--priority-high-text); }
.priority-tag--medium { background: var(--priority-medium-bg); color: var(--priority-medium-text); }
.priority-tag--low    { background: var(--priority-low-bg);    color: var(--priority-low-text); }

/* ··· 菜单按钮（从 ProjectCard 复制） */
.task-card {
  position: relative;
}

.card-menu-btn {
  position: absolute;
  top: var(--spacing-2);
  right: var(--spacing-2);
  width: 28px;
  height: 28px;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: var(--text-xl);
  font-weight: bold;
  letter-spacing: 1px;
  color: var(--text-secondary);
  background: transparent;
  border: none;
  border-radius: var(--radius-sm);
  cursor: pointer;
  opacity: 0;
  transform: scale(0.85);
  transition: opacity var(--duration-fast) var(--ease-out),
              transform var(--duration-fast) var(--ease-out),
              background var(--duration-fast) var(--ease-out);
  line-height: 1;
  padding-bottom: 4px;
}

.task-card:hover .card-menu-btn,
.card-menu-btn.visible {
  opacity: 1;
  transform: scale(1);
}

.card-menu-btn:hover {
  background: var(--bg-hover);
  color: var(--text-primary);
}
```

> **注意**：菜单本身（`.card-menu`、`.priority-pill` 等）已在 ProjectCard 的全局 `<style>` 中定义，此处无需重复。

### Step 4: 验证 TypeScript

```bash
npx vue-tsc --noEmit 2>&1
```

### Step 5: Commit

```bash
git add src/components/TaskCard.vue
git commit -m "feat(TaskCard): 新增优先度标签展示 + 右上角 ··· 菜单（内含优先度选择器）"
```

---

## Task 9: HomePage — 排序控制 + 逻辑

**Files:**
- Modify: `src/views/HomePage.vue`

### Step 1: 在 script setup 中添加排序状态和逻辑

在 `const showGuide = ref(false)` 之后插入：

```typescript
import { computed } from 'vue'
import { invoke } from '@tauri-apps/api/core'

// 排序模式
const sortMode = ref<'default' | 'deadline' | 'priority'>('default')

// 项目是否已完成
function isProjectComplete(p: typeof projects.value[0]): boolean {
  const enabled = p.enabled_tasks
  const parentTasks = enabled.filter(t => !t.includes('/'))
  let total = 0, done = 0
  const completedSubs = new Set(p.completed_subtasks)
  const completedT = new Set(p.completed_tasks)
  for (const parent of parentTasks) {
    const children = enabled.filter(t => t.startsWith(parent + '/'))
    if (children.length === 0) {
      total++
      if (completedT.has(parent)) done++
    } else {
      total += children.length
      done += children.filter(c => completedSubs.has(c)).length
    }
  }
  return total > 0 && done >= total
}

const PRIORITY_ORDER: Record<string, number> = { high: 0, medium: 1, low: 2 }

const sortedProjects = computed(() => {
  const list = [...projects.value]
  if (sortMode.value === 'default') return list

  if (sortMode.value === 'priority') {
    return list.sort((a, b) => {
      const ao = a.priority ? (PRIORITY_ORDER[a.priority] ?? 3) : 3
      const bo = b.priority ? (PRIORITY_ORDER[b.priority] ?? 3) : 3
      if (ao !== bo) return ao - bo
      return a.name.localeCompare(b.name)
    })
  }

  // deadline 排序
  const today = new Date()
  today.setHours(0, 0, 0, 0)
  return list.sort((a, b) => {
    const aComplete = isProjectComplete(a)
    const bComplete = isProjectComplete(b)
    // 已完成的排最后
    if (aComplete !== bComplete) return aComplete ? 1 : -1
    if (aComplete && bComplete) return a.name.localeCompare(b.name)

    // 都未完成：处理截止日期
    const aDate = a.deadline ? new Date(a.deadline) : null
    const bDate = b.deadline ? new Date(b.deadline) : null
    if (!aDate && !bDate) return a.name.localeCompare(b.name)
    if (!aDate) return 1
    if (!bDate) return -1
    // 超时且未完成 vs 未超时：超时的靠前
    const aOverdue = aDate < today
    const bOverdue = bDate < today
    if (aOverdue !== bOverdue) return aOverdue ? -1 : 1
    // 同类型按截止日期从近到远
    return aDate.getTime() - bDate.getTime()
  })
})
```

> **`computed` import**：确认 `vue` import 行包含 `computed`。

### Step 2: 在 onProjectAction 中处理 refresh 事件

修改 `onProjectAction` 函数，接受 refresh 情况：

在 `function onProjectAction(project: ProjectInfo, action: 'rename' | 'deadline' | 'delete')` 之后添加：

```typescript
function onProjectRefresh() {
  loadProjects()
}
```

### Step 3: 在 template 的 page-header 中加排序按钮

将原 `<div class="page-header">` 内容改为：

```html
<div class="page-header">
  <p class="page-hint">{{ $t('home.myProjects') }}</p>
  <div class="sort-tabs">
    <button
      v-for="mode in (['default', 'deadline', 'priority'] as const)"
      :key="mode"
      class="sort-tab"
      :class="{ 'is-active': sortMode === mode }"
      @click="sortMode = mode"
    >{{ $t(`home.sort${mode.charAt(0).toUpperCase() + mode.slice(1)}`) }}</button>
  </div>
  <button class="add-btn" :title="$t('home.createProject')" @click="showCreateDialog = true">+</button>
</div>
```

### Step 4: 将 `v-for="project in projects"` 改为 `sortedProjects`

```html
<ProjectCard
  v-for="(project, i) in sortedProjects"
  ...
  @refresh="onProjectRefresh"
/>
```

### Step 5: 在 scoped style 中添加排序按钮样式

```css
.sort-tabs {
  display: flex;
  gap: var(--spacing-1);
  margin-left: auto;
}

.sort-tab {
  height: 28px;
  padding: 0 var(--spacing-3);
  font-size: var(--text-xs);
  font-family: inherit;
  color: var(--text-secondary);
  background: transparent;
  border: 1px solid var(--border-light);
  border-radius: var(--radius-md);
  cursor: pointer;
  transition: all var(--duration-fast) var(--ease-out);
}

.sort-tab:hover {
  color: var(--text-primary);
  background: var(--bg-hover);
}

.sort-tab.is-active {
  color: var(--color-primary-300);
  background: color-mix(in srgb, var(--color-primary-500) 12%, transparent);
  border-color: var(--color-primary-700);
}

/* add-btn 的 margin-left: auto 已不再需要，由 sort-tabs 推向右侧 */
```

> **注意**：原 page-header 只有 hint + add-btn，现在加了 sort-tabs 在中间。add-btn 靠右改由 sort-tabs 的 `margin-left: auto` 实现，add-btn 自身不再需要 margin-left。

### Step 6: 验证 TypeScript

```bash
npx vue-tsc --noEmit 2>&1
```

### Step 7: Commit

```bash
git add src/views/HomePage.vue
git commit -m "feat(HomePage): 新增截止日期/优先度排序选项 + 处理 ProjectCard refresh 事件"
```

---

## Task 10: ProjectPage — 排序控制 + 任务优先度处理

**Files:**
- Modify: `src/views/ProjectPage.vue`

### Step 1: 在 script setup 中添加排序状态 + 任务优先度处理

在 `const showGuide = ref(false)` 之后插入（确保 `computed`、`invoke` 已 import）：

```typescript
import { computed } from 'vue'  // 如果还没有
import { invoke } from '@tauri-apps/api/core'  // 已有，确认即可

// 排序模式
const sortMode = ref<'default' | 'priority'>('default')

const PRIORITY_ORDER: Record<string, number> = { high: 0, medium: 1, low: 2 }

const sortedTasks = computed(() => {
  const list = [...tasks.value]
  if (sortMode.value === 'default') return list
  return list.sort((a, b) => {
    const ao = a.priority ? (PRIORITY_ORDER[a.priority] ?? 3) : 3
    const bo = b.priority ? (PRIORITY_ORDER[b.priority] ?? 3) : 3
    if (ao !== bo) return ao - bo
    return a.name.localeCompare(b.name)
  })
})

// 处理 TaskCard 的优先度 action
async function onTaskAction(task: TaskInfo, _action: 'priority', value: string | null) {
  if (!projectPath) return
  await invoke('set_task_priority', {
    projectPath,
    taskName: task.name,
    priority: value,
  })
  await loadTasks(projectPath)
}
```

### Step 2: 在 template 的 sub-title-bar 中加排序按钮

将原 `<div class="sub-title-bar">` 改为：

```html
<div class="sub-title-bar">
  <span class="sub-title">{{ $t('project.tasks') }}</span>
  <div class="sort-tabs">
    <button
      v-for="mode in (['default', 'priority'] as const)"
      :key="mode"
      class="sort-tab"
      :class="{ 'is-active': sortMode === mode }"
      @click="sortMode = mode"
    >{{ $t(`project.sort${mode.charAt(0).toUpperCase() + mode.slice(1)}`) }}</button>
  </div>
</div>
```

### Step 3: 将 `v-for="task in tasks"` 改为 `sortedTasks`，并绑定 action

```html
<TaskCard
  v-for="(task, i) in sortedTasks"
  :key="task.name"
  :style="{ '--delay': i * 40 + 'ms' }"
  :task="task"
  :subtask-progress="taskSubtaskProgress[task.name.toLowerCase()]"
  @click="openTask"
  @action="onTaskAction"
/>
```

### Step 4: 在 scoped style 中添加排序按钮样式

复用与 HomePage 一致的 `.sort-tabs` / `.sort-tab` 样式（复制粘贴）：

```css
/* sub-title-bar 扩展为 flex row */
.sub-title-bar {
  display: flex;
  align-items: center;
  gap: var(--spacing-3);
}

.sort-tabs {
  display: flex;
  gap: var(--spacing-1);
  margin-left: auto;
}

.sort-tab {
  height: 26px;
  padding: 0 var(--spacing-3);
  font-size: var(--text-xs);
  font-family: inherit;
  color: var(--text-secondary);
  background: transparent;
  border: 1px solid var(--border-light);
  border-radius: var(--radius-md);
  cursor: pointer;
  transition: all var(--duration-fast) var(--ease-out);
}

.sort-tab:hover {
  color: var(--text-primary);
  background: var(--bg-hover);
}

.sort-tab.is-active {
  color: var(--color-primary-300);
  background: color-mix(in srgb, var(--color-primary-500) 12%, transparent);
  border-color: var(--color-primary-700);
}
```

> **注意**：`.sub-title-bar` 已在 design-system.css 中定义为公共类，检查是否 `display: flex` 已有。如果已有则只追加 gap 即可，不要重复定义 display。

### Step 5: 验证 TypeScript

```bash
npx vue-tsc --noEmit 2>&1
```

期望：0 errors

### Step 6: Commit

```bash
git add src/views/ProjectPage.vue
git commit -m "feat(ProjectPage): 新增优先度排序选项 + 处理 TaskCard 优先度 action"
```

---

## Task 11: 手动功能验证

```bash
cd "C:\work\PG Butler\PGB1"
npm run tauri dev
```

逐一验证：

1. **HomePage 排序按钮**：Header 右侧出现「默认 / 截止日期 / 优先度」三个 Tab
2. **ProjectCard 优先度标签**：右上角 ··· 菜单顶部出现优先度选择器，选择后卡片名称前显示红/橙/绿标签
3. **截止日期排序**：切到「截止日期」模式，未完成项目按截止日期升序排列，超时未完成的在最前，已完成的在最后
4. **优先度排序（项目）**：切到「优先度」模式，高 → 中 → 低 → 无排序正确
5. **TaskCard ··· 菜单**：ProjectPage 任务卡片右上角悬停出现 ··· 按钮，点击弹出含优先度选择器的菜单
6. **任务优先度**：选择后卡片名称前显示标签，切「优先度」排序后任务按优先度排列
7. **清除优先度**：点 ✕ 后标签消失
8. **持久化**：重启 dev 后优先度数据仍在（存入 .pgb1_project.json 验证）

---

## Task 12: 更新文档

**Files:**
- Modify: `CODE_INDEX.md`

### Step 1: 更新 CODE_INDEX.md

1. 更新文件统计行数
2. `ProjectCard.vue` 行数 ~380 → ~450，职责末尾追加：`**优先度标签**：名称前 priority-tag（高/中/低颜色胶囊）；菜单顶部内联 priority-pill 行直接调用 set_project_priority + emit refresh`
3. `TaskCard.vue` 行数 ~135 → ~230，职责更新为含 ··· 菜单 + 优先度 emit
4. `HomePage.vue` 行数 ~170 → ~220，职责追加排序逻辑
5. `ProjectPage.vue` 行数 ~360 → ~420，职责追加排序逻辑 + TaskCard action 处理
6. `commands.rs` 命令列表新增 `set_project_priority` / `set_task_priority`
7. `ProjectConfig` 数据模型描述追加 `priority` / `task_priorities` 字段
8. `ProjectInfo` / `TaskInfo` 追加 `priority` 字段说明

### Step 2: 更新 MEMORY.md

在版本状态行更新版本号（如已到 v2.4.2 → 这次不涉及版本号，记录功能记忆即可）。

### Step 3: Commit

```bash
git add CODE_INDEX.md
git commit -m "docs: 更新 CODE_INDEX 反映排序/优先度功能的代码变更"
```

---

## 快速参考：Priority 值约定

| 值 | 标签 | CSS 类 | 颜色 |
|----|------|--------|------|
| `"high"` | 高 / High | `priority-tag--high` | danger 红 |
| `"medium"` | 中 / Med | `priority-tag--medium` | warning 橙 |
| `"low"` | 低 / Low | `priority-tag--low` | success 绿 |
| `null` | （不显示） | — | — |

## 爆炸半径预警

- `ProjectConfig` 字段新增对旧 JSON 向后兼容（`#[serde(default)]` 保障）
- `ProjectInfo` / `TaskInfo` 前端接口新增字段，现有模板用 `v-if="project.priority"` 保护，不会破坏无优先度的卡片
- `TaskCard` 从 `<button>` 改为同时包含 `<Teleport>` — Teleport 不在 button 内部，而是 template 根级并列，Vue 3 支持多根节点，无问题
- `scan_tasks` 新增读文件 I/O — 使用 `.ok().and_then()` 链式，读取失败时静默返回空 Map，不影响正常扫描
