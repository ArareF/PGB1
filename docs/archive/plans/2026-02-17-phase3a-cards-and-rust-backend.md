# Phase 3a: 项目卡片 + 任务卡片 + Rust 文件系统后端

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 主页展示真实项目卡片、项目页展示真实任务卡片，数据由 Rust 后端扫描文件系统获取，替换当前的 mock 数据。

**Architecture:** Rust 后端通过 Tauri commands 扫描用户指定的项目根目录，识别有效项目和任务，自动创建/读取 `.pgb1_project.json` 配置文件。前端通过 composables 调用 Rust 命令，将数据传给独立的卡片组件渲染。

**Tech Stack:** Tauri 2.x Commands (Rust), Vue 3 Composition API, TypeScript, serde/serde_json

**前置条件:** Phase 2 已完成 — 5 个页面骨架、Vue Router、useNavigation、TitleBar 动态化均已就位。

**测试数据:** `D:\work\pgsoft\exp\217_RedDevil`（真实项目，8 个任务）

**设计文档参考:**
- 卡片视觉规范: `design/卡片设计.md`
- 文件目录结构: `design/文件命名与组织规则.md` L8-106
- 自动化识别规则: `design/自动化识别规则.md` L37-250
- 卡片组件 CSS Token: `design/DesignSystem.md` L728-835
- 项目配置文件: `design/自动化识别规则.md` L88-100

---

## 总体路线图（Phase 3a 在全局中的位置）

| 阶段 | 内容 | 状态 |
|------|------|------|
| **Phase 1** | 脚手架 + 设计系统 + 主窗口框架 | ✅ 已完成 |
| **Phase 2** | 三级页面导航 + Vue Router + 页面骨架 | ✅ 已完成 |
| **Phase 3a**（本文档） | 项目卡片 + 任务卡片 + Rust 文件系统后端 | 🔵 当前 |
| **Phase 3b** | 素材卡片 + 普通卡片 + 预览图系统 | ⬜ |
| **Phase 4** | 双视图模式 + 侧边栏详情 | ⬜ |
| **Phase 5** | 文件拖拽 + 任务管理系统 | ⬜ |
| **Phase 6** | 日报打卡 + 翻译 + 转换进度悬浮窗 | ⬜ |

---

## Task 1: Rust 数据模型定义

**Files:**
- Create: `src-tauri/src/models.rs`
- Modify: `src-tauri/src/lib.rs`（添加 mod 声明）

**Step 1: 创建数据模型**

创建 `src-tauri/src/models.rs`：

```rust
use serde::{Deserialize, Serialize};

/// 项目配置文件（.pgb1_project.json）的结构
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ProjectConfig {
    pub project_name: String,
    pub created_at: String,
    #[serde(default)]
    pub imported: bool,
    pub deadline: Option<String>,
    pub enabled_tasks: Vec<String>,
    #[serde(default)]
    pub archived_tasks: Vec<String>,
}

/// 返回给前端的项目信息
#[derive(Debug, Serialize, Clone)]
pub struct ProjectInfo {
    /// 项目名称（目录名），如 "217_RedDevil"
    pub name: String,
    /// 项目完整路径
    pub path: String,
    /// 截止日期，如 "2026-03-15" 或 null
    pub deadline: Option<String>,
    /// 任务名称列表
    pub tasks: Vec<String>,
    /// 任务总数
    pub task_count: usize,
}

/// 返回给前端的任务信息
#[derive(Debug, Serialize, Clone)]
pub struct TaskInfo {
    /// 任务名称（目录名），如 "Ambient"
    pub name: String,
    /// 任务完整路径（Export 下的目录）
    pub path: String,
    /// 文件夹总大小（字节）
    pub size_bytes: u64,
    /// 是否有子任务（Prototype 特例）
    pub has_subtasks: bool,
}
```

**Step 2: 在 lib.rs 添加模块声明**

在 `src-tauri/src/lib.rs` 顶部添加：

```rust
mod models;
```

（暂时只加声明，不改 `run()` 函数）

**Step 3: 验证编译**

```bash
cd D:/work/pgsoft/PGB1/src-tauri && cargo check
```

Expected: 编译通过，无错误（可能有 unused 警告，正常）。

---

## Task 2: Rust 文件系统扫描命令

**Files:**
- Create: `src-tauri/src/commands.rs`
- Modify: `src-tauri/src/lib.rs`（注册命令）

**Step 1: 创建 commands 模块**

创建 `src-tauri/src/commands.rs`：

```rust
use crate::models::{ProjectConfig, ProjectInfo, TaskInfo};
use std::fs;
use std::path::Path;

/// 扫描项目根目录，返回所有有效项目
#[tauri::command]
pub fn scan_projects(root_dir: String) -> Result<Vec<ProjectInfo>, String> {
    let root = Path::new(&root_dir);
    if !root.exists() {
        return Err(format!("项目根目录不存在: {}", root_dir));
    }

    let mut projects = Vec::new();

    let entries = fs::read_dir(root).map_err(|e| format!("无法读取目录: {}", e))?;

    for entry in entries {
        let entry = entry.map_err(|e| format!("读取目录项失败: {}", e))?;
        let path = entry.path();

        // 跳过非目录和隐藏目录
        if !path.is_dir() {
            continue;
        }
        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
            if name.starts_with('.') {
                continue;
            }
        }

        // 检查是否为有效项目：必须存在 03_Render_VFX/VFX/Export/
        let export_path = path.join("03_Render_VFX").join("VFX").join("Export");
        if !export_path.exists() {
            continue;
        }

        // 读取或创建配置文件
        let config = load_or_create_config(&path)?;

        // 扫描 Export 下的任务列表
        let tasks = scan_task_names(&export_path)?;
        let task_count = tasks.len();

        let project_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        projects.push(ProjectInfo {
            name: project_name,
            path: path.to_string_lossy().to_string(),
            deadline: config.deadline,
            tasks,
            task_count,
        });
    }

    // 按项目名排序
    projects.sort_by(|a, b| a.name.cmp(&b.name));

    Ok(projects)
}

/// 扫描指定项目的任务列表
#[tauri::command]
pub fn scan_tasks(project_path: String) -> Result<Vec<TaskInfo>, String> {
    let export_path = Path::new(&project_path)
        .join("03_Render_VFX")
        .join("VFX")
        .join("Export");

    if !export_path.exists() {
        return Err(format!("Export 目录不存在: {}", export_path.display()));
    }

    let mut tasks = Vec::new();

    let entries = fs::read_dir(&export_path).map_err(|e| format!("无法读取 Export 目录: {}", e))?;

    for entry in entries {
        let entry = entry.map_err(|e| format!("读取目录项失败: {}", e))?;
        let path = entry.path();

        // 跳过非目录和隐藏项
        if !path.is_dir() {
            continue;
        }
        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
            if name.starts_with('.') {
                continue;
            }
        }

        let task_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        let size_bytes = calc_dir_size(&path);
        let has_subtasks = task_name.to_lowercase() == "prototype";

        tasks.push(TaskInfo {
            name: task_name,
            path: path.to_string_lossy().to_string(),
            size_bytes,
            has_subtasks,
        });
    }

    // 按任务名排序
    tasks.sort_by(|a, b| a.name.cmp(&b.name));

    Ok(tasks)
}

/// 读取或自动创建 .pgb1_project.json
fn load_or_create_config(project_path: &Path) -> Result<ProjectConfig, String> {
    let config_path = project_path.join(".pgb1_project.json");

    if config_path.exists() {
        let content =
            fs::read_to_string(&config_path).map_err(|e| format!("读取配置文件失败: {}", e))?;
        let config: ProjectConfig =
            serde_json::from_str(&content).map_err(|e| format!("解析配置文件失败: {}", e))?;
        return Ok(config);
    }

    // 自动创建配置文件（旧项目导入）
    let project_name = project_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string();

    let export_path = project_path.join("03_Render_VFX").join("VFX").join("Export");
    let enabled_tasks = if export_path.exists() {
        scan_task_names(&export_path)?
            .into_iter()
            .map(|name| name.to_lowercase())
            .collect()
    } else {
        Vec::new()
    };

    let config = ProjectConfig {
        project_name,
        created_at: chrono::Utc::now().to_rfc3339(),
        imported: true,
        deadline: None,
        enabled_tasks,
        archived_tasks: Vec::new(),
    };

    let json = serde_json::to_string_pretty(&config)
        .map_err(|e| format!("序列化配置失败: {}", e))?;
    fs::write(&config_path, json).map_err(|e| format!("写入配置文件失败: {}", e))?;

    Ok(config)
}

/// 扫描 Export 目录下的任务名称列表
fn scan_task_names(export_path: &Path) -> Result<Vec<String>, String> {
    let mut names = Vec::new();

    let entries =
        fs::read_dir(export_path).map_err(|e| format!("无法读取 Export 目录: {}", e))?;

    for entry in entries {
        let entry = entry.map_err(|e| format!("读取目录项失败: {}", e))?;
        let path = entry.path();

        if !path.is_dir() {
            continue;
        }
        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
            if !name.starts_with('.') {
                names.push(name.to_string());
            }
        }
    }

    names.sort();
    Ok(names)
}

/// 递归计算目录大小（字节）
fn calc_dir_size(path: &Path) -> u64 {
    let mut size = 0u64;

    if let Ok(entries) = fs::read_dir(path) {
        for entry in entries.flatten() {
            let entry_path = entry.path();
            if entry_path.is_file() {
                size += entry_path.metadata().map(|m| m.len()).unwrap_or(0);
            } else if entry_path.is_dir() {
                size += calc_dir_size(&entry_path);
            }
        }
    }

    size
}
```

**Step 2: 添加 chrono 依赖**

在 `src-tauri/Cargo.toml` 的 `[dependencies]` 中添加：

```toml
chrono = "0.4"
```

**Step 3: 在 lib.rs 注册命令**

修改 `src-tauri/src/lib.rs`：

```rust
mod commands;
mod models;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            commands::scan_projects,
            commands::scan_tasks,
        ])
        .setup(|app| {
            let window = app.get_webview_window("main").unwrap();

            // Windows: 应用 Acrylic 半透明毛玻璃效果
            #[cfg(target_os = "windows")]
            {
                use window_vibrancy::apply_acrylic;
                let _ = apply_acrylic(&window, Some((0, 0, 0, 1)));
            }

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

**Step 4: 验证编译**

```bash
cd D:/work/pgsoft/PGB1/src-tauri && cargo check
```

Expected: 编译通过。

---

## Task 3: 前端配置与数据层

**Files:**
- Create: `src/config/app.ts`
- Create: `src/composables/useProjects.ts`
- Create: `src/composables/useTasks.ts`

**Step 1: 创建应用配置**

创建 `src/config/app.ts`：

```typescript
/**
 * 应用配置 — Phase 6 改为从用户设置读取
 * 当前硬编码开发测试路径
 */
export const APP_CONFIG = {
  /** 项目根目录 — 扫描此目录下的所有项目 */
  projectRootDir: 'D:\\work\\pgsoft\\exp',
} as const
```

**Step 2: 创建 useProjects composable**

创建 `src/composables/useProjects.ts`：

```typescript
import { ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { APP_CONFIG } from '../config/app'

export interface ProjectInfo {
  name: string
  path: string
  deadline: string | null
  tasks: string[]
  task_count: number
}

export function useProjects() {
  const projects = ref<ProjectInfo[]>([])
  const loading = ref(false)
  const error = ref<string | null>(null)

  async function loadProjects() {
    loading.value = true
    error.value = null
    try {
      projects.value = await invoke<ProjectInfo[]>('scan_projects', {
        rootDir: APP_CONFIG.projectRootDir,
      })
    } catch (e) {
      error.value = String(e)
      console.error('扫描项目失败:', e)
    } finally {
      loading.value = false
    }
  }

  return {
    projects,
    loading,
    error,
    loadProjects,
  }
}
```

**Step 3: 创建 useTasks composable**

创建 `src/composables/useTasks.ts`：

```typescript
import { ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'

export interface TaskInfo {
  name: string
  path: string
  size_bytes: number
  has_subtasks: boolean
}

export function useTasks() {
  const tasks = ref<TaskInfo[]>([])
  const loading = ref(false)
  const error = ref<string | null>(null)

  async function loadTasks(projectPath: string) {
    loading.value = true
    error.value = null
    try {
      tasks.value = await invoke<TaskInfo[]>('scan_tasks', {
        projectPath,
      })
    } catch (e) {
      error.value = String(e)
      console.error('扫描任务失败:', e)
    } finally {
      loading.value = false
    }
  }

  return {
    tasks,
    loading,
    error,
    loadTasks,
  }
}
```

**Step 4: 验证 TypeScript**

```bash
cd D:/work/pgsoft/PGB1 && npx vue-tsc --noEmit
```

Expected: 零错误。

---

## Task 4: 设计系统卡片尺寸更新

**Files:**
- Modify: `src/styles/design-system.css`

**Step 1: 更新卡片尺寸为 ~2:1 比例**

修改 `src/styles/design-system.css` 中的卡片尺寸 token：

```css
/* 项目卡片 — 2:1 比例 */
--card-project-width:       320px;
--card-project-height:      160px;    /* 原 220px */

/* 任务卡片 — 2:1 比例 */
--card-task-width:          280px;
--card-task-height:         140px;    /* 原 180px */
```

同时更新 `design/DesignSystem.md` 中对应的文档值。

---

## Task 5: ProjectCard 组件

**Files:**
- Create: `src/components/ProjectCard.vue`

**Step 1: 创建项目卡片组件**

创建 `src/components/ProjectCard.vue`：

```vue
<script setup lang="ts">
import type { ProjectInfo } from '../composables/useProjects'

defineProps<{
  project: ProjectInfo
}>()

defineEmits<{
  click: [project: ProjectInfo]
}>()
</script>

<template>
  <button
    class="project-card glass-subtle"
    @click="$emit('click', project)"
  >
    <!-- 左侧 ICON 占位 -->
    <div class="card-icon">
      <svg width="32" height="32" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
        <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z" />
      </svg>
    </div>

    <!-- 右侧信息 -->
    <div class="card-info">
      <span class="card-name">{{ project.name }}</span>
      <span class="card-deadline">
        {{ project.deadline ?? '未设置截止日期' }}
      </span>
    </div>

    <!-- 底部进度条 -->
    <div class="card-progress">
      <div class="progress-bar">
        <div class="progress-fill" :style="{ width: '0%' }" />
      </div>
      <span class="progress-text">{{ project.task_count }} 个任务</span>
    </div>
  </button>
</template>

<style scoped>
.project-card {
  width: var(--card-project-width);
  height: var(--card-project-height);
  display: grid;
  grid-template-columns: auto 1fr;
  grid-template-rows: 1fr auto;
  gap: 0 var(--card-project-gap);
  padding: var(--card-project-padding);
  border-radius: var(--card-border-radius);
  border: none;
  cursor: pointer;
  transition: var(--transition-card-hover);
  text-align: left;
}

.project-card:hover {
  transform: translateY(var(--card-hover-lift));
  box-shadow: var(--card-shadow-hover);
}

.card-icon {
  grid-row: 1;
  grid-column: 1;
  width: var(--card-project-icon-size);
  height: var(--card-project-icon-size);
  display: flex;
  align-items: center;
  justify-content: center;
  border-radius: var(--radius-lg);
  background: var(--bg-hover);
  color: var(--text-secondary);
}

.card-info {
  grid-row: 1;
  grid-column: 2;
  display: flex;
  flex-direction: column;
  justify-content: center;
  gap: var(--spacing-1);
  min-width: 0;
}

.card-name {
  font-size: var(--text-lg);
  font-weight: var(--font-weight-heading);
  color: var(--text-primary);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.card-deadline {
  font-size: var(--text-sm);
  color: var(--text-tertiary);
}

.card-progress {
  grid-row: 2;
  grid-column: 1 / -1;
  display: flex;
  align-items: center;
  gap: var(--spacing-2);
}

.progress-bar {
  flex: 1;
  height: 4px;
  background: var(--bg-hover);
  border-radius: var(--radius-full);
  overflow: hidden;
}

.progress-fill {
  height: 100%;
  background: var(--color-primary-500);
  border-radius: var(--radius-full);
  transition: width var(--duration-normal) var(--ease-out);
}

.progress-text {
  font-size: var(--text-xs);
  color: var(--text-tertiary);
  white-space: nowrap;
}
</style>
```

---

## Task 6: TaskCard 组件

**Files:**
- Create: `src/components/TaskCard.vue`

**Step 1: 创建任务卡片组件**

创建 `src/components/TaskCard.vue`：

```vue
<script setup lang="ts">
import type { TaskInfo } from '../composables/useTasks'

defineProps<{
  task: TaskInfo
}>()

defineEmits<{
  click: [task: TaskInfo]
}>()

/** 文件大小格式化 */
function formatSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(1)} GB`
}
</script>

<template>
  <button
    class="task-card glass-subtle"
    @click="$emit('click', task)"
  >
    <span class="task-name">{{ task.name }}</span>

    <div class="task-bottom">
      <span class="status-tag status-pending">未开始</span>
      <span class="task-size">{{ formatSize(task.size_bytes) }}</span>
    </div>
  </button>
</template>

<style scoped>
.task-card {
  width: var(--card-task-width);
  height: var(--card-task-height);
  display: flex;
  flex-direction: column;
  justify-content: space-between;
  padding: var(--card-task-padding);
  border-radius: var(--card-border-radius);
  border: none;
  cursor: pointer;
  transition: var(--transition-card-hover);
  text-align: left;
}

.task-card:hover {
  transform: translateY(var(--card-hover-lift));
  box-shadow: var(--card-shadow-hover);
}

.task-name {
  font-size: var(--text-lg);
  font-weight: var(--font-weight-heading);
  color: var(--text-primary);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.task-bottom {
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.status-tag {
  display: inline-flex;
  align-items: center;
  height: var(--tag-height);
  padding: 0 var(--tag-padding-x);
  font-size: var(--tag-font-size);
  font-weight: var(--tag-font-weight);
  border-radius: var(--tag-border-radius);
  color: var(--tag-status-text);
}

.status-pending {
  background: var(--tag-status-pending-bg);
}

.task-size {
  font-size: var(--text-sm);
  color: var(--text-tertiary);
}
</style>
```

---

## Task 7: HomePage 改造 — 接入真实数据

**Files:**
- Modify: `src/views/HomePage.vue`

**Step 1: 替换 mock 数据为真实数据**

完整替换 `src/views/HomePage.vue`：

```vue
<script setup lang="ts">
import { onMounted } from 'vue'
import { useRouter } from 'vue-router'
import { useNavigation } from '../composables/useNavigation'
import { useTheme } from '../composables/useTheme'
import { useProjects } from '../composables/useProjects'
import type { ProjectInfo } from '../composables/useProjects'
import ProjectCard from '../components/ProjectCard.vue'

const router = useRouter()
const { setNavigation } = useNavigation()
const { toggleTheme } = useTheme()
const { projects, loading, loadProjects } = useProjects()

/* 注册主页导航配置 */
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

onMounted(() => {
  loadProjects()
})

function openProject(project: ProjectInfo) {
  router.push({ name: 'project', params: { projectId: project.name } })
}
</script>

<template>
  <div class="home-page">
    <p class="page-hint">我的项目</p>

    <p v-if="loading" class="loading-text">扫描中...</p>

    <div v-else class="card-grid">
      <ProjectCard
        v-for="project in projects"
        :key="project.name"
        :project="project"
        @click="openProject"
      />
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
  font-size: var(--text-3xl);
  color: var(--text-primary);
  padding-bottom: var(--spacing-4);
  border-bottom: 1px solid var(--border-medium);
}

.loading-text {
  font-size: var(--text-lg);
  color: var(--text-tertiary);
}

.card-grid {
  display: flex;
  flex-wrap: wrap;
  gap: var(--gap-card);
}
</style>
```

注意：`page-hint` 文字从"主页"改为"我的项目"，更有语义。

---

## Task 8: ProjectPage 改造 — 接入真实数据

**Files:**
- Modify: `src/views/ProjectPage.vue`

**Step 1: 替换 mock 数据为真实数据**

完整替换 `src/views/ProjectPage.vue`：

```vue
<script setup lang="ts">
import { onMounted } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { useNavigation } from '../composables/useNavigation'
import { useTheme } from '../composables/useTheme'
import { useProjects } from '../composables/useProjects'
import { useTasks } from '../composables/useTasks'
import type { TaskInfo } from '../composables/useTasks'
import TaskCard from '../components/TaskCard.vue'

const route = useRoute()
const router = useRouter()
const { setNavigation } = useNavigation()
const { toggleTheme } = useTheme()
const { projects, loadProjects } = useProjects()
const { tasks, loading, loadTasks } = useTasks()

const projectId = route.params.projectId as string

/* 注册项目页导航配置 */
setNavigation({
  title: `项目：${projectId}`,
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

onMounted(async () => {
  /* 先加载项目列表获取项目路径，再用路径加载任务 */
  await loadProjects()
  const project = projects.value.find(p => p.name === projectId)
  if (project) {
    await loadTasks(project.path)
  }
})

function openTask(task: TaskInfo) {
  router.push({ name: 'task', params: { projectId, taskId: task.name } })
}
</script>

<template>
  <div class="project-page">
    <p class="page-hint">制作任务</p>

    <p v-if="loading" class="loading-text">扫描中...</p>

    <div v-else class="card-grid">
      <TaskCard
        v-for="task in tasks"
        :key="task.name"
        :task="task"
        @click="openTask"
      />
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
  font-size: var(--text-3xl);
  color: var(--text-primary);
  padding-bottom: var(--spacing-4);
  border-bottom: 1px solid var(--border-medium);
}

.loading-text {
  font-size: var(--text-lg);
  color: var(--text-tertiary);
}

.card-grid {
  display: flex;
  flex-wrap: wrap;
  gap: var(--gap-card);
}
</style>
```

---

## Task 9: 构建验证与收尾

**Step 1: TypeScript 检查**

```bash
cd D:/work/pgsoft/PGB1 && npx vue-tsc --noEmit
```

Expected: 零错误。

**Step 2: Vite 构建**

```bash
cd D:/work/pgsoft/PGB1 && npm run build
```

Expected: 构建成功。

**Step 3: Cargo 编译**

```bash
cd D:/work/pgsoft/PGB1/src-tauri && cargo build
```

Expected: 编译成功。

**Step 4: Tauri Dev 全流程验证**

```bash
cd D:/work/pgsoft/PGB1 && npm run tauri dev
```

验证清单：
- [ ] 主页显示"我的项目"标题 + `217_RedDevil` 项目卡片
- [ ] 项目卡片显示项目名、截止日期（"未设置截止日期"）、任务数量
- [ ] 点击项目卡片 → 进入项目页
- [ ] 项目页显示"制作任务"标题 + 8 个任务卡片（Ambient, Free Spin, ...）
- [ ] 任务卡片显示任务名、"未开始"标签、文件大小
- [ ] 点击任务卡片 → 进入任务页
- [ ] 返回按钮正常工作
- [ ] 明暗主题切换在新卡片上正常

**Step 5: 确认 .pgb1_project.json 已生成**

检查 `D:\work\pgsoft\exp\217_RedDevil\.pgb1_project.json` 是否存在且内容正确。

**Step 6: 更新设计文档**

- 更新 `design/DesignSystem.md` 中卡片尺寸（项目卡片 320×160，任务卡片 280×140）
- 更新 `design/卡片设计.md` 中截止日期待明确项（已确认：从 `.pgb1_project.json` 读取）

---

## Phase 3a 实施记录

**完成时间:** 2026-02-17

### 已完成

- [x] Task 1: models.rs — ProjectConfig / ProjectInfo / TaskInfo
- [x] Task 2: commands.rs — scan_projects / scan_tasks + chrono 依赖 + lib.rs 注册
- [x] Task 3: app.ts + useProjects.ts + useTasks.ts
- [x] Task 4: 卡片尺寸更新（项目 320x160，任务 280x140）
- [x] Task 5: ProjectCard.vue 组件
- [x] Task 6: TaskCard.vue 组件
- [x] Task 7: HomePage.vue 接入真实数据
- [x] Task 8: ProjectPage.vue 接入真实数据
- [x] Task 9: 构建验证（vue-tsc + vite build + cargo build 全部通过）+ 文档更新

### 遗留问题

- 进度条目前固定 0%，需要 Phase 3b 素材扫描后才有真实进度数据
- "打开项目文件夹"按钮为占位，需 Phase 5 实现 opener 命令
