# Phase 3b: 普通卡片 + 辅助页面文件扫描 + 打开文件夹

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 游戏介绍页和项目素材页展示真实文件（普通卡片），所有"打开文件夹"功能可用，刷新功能可用。

**Scope 界定:**
- **本 Phase 做的**：普通卡片组件、Rust 文件扫描命令（辅助目录）、GameIntroPage/MaterialsPage 改造、所有"打开文件夹"按钮、刷新功能
- **不做的**：素材卡片（任务页双视图）、预览图系统（Canvas 动画）、进度链条 → 这些归 Phase 4

**Architecture:** Rust 后端新增 scan_directory 通用命令扫描任意目录文件列表，open_in_explorer 命令打开系统文件管理器。前端新增 NormalCard 组件 + useDirectoryFiles composable。

**Tech Stack:** Tauri 2.x Commands (Rust), tauri-plugin-opener, Vue 3 Composition API, TypeScript

**前置条件:** Phase 3a 已完成 — 项目卡片 + 任务卡片 + Rust 后端扫描均已就位。

**测试数据:** `D:\work\pgsoft\exp\217_RedDevil`

**设计文档参考:**
- 普通卡片视觉规范: `design/卡片设计.md` L182-214
- 普通卡片 CSS Token: `design/DesignSystem.md` L787-800
- 游戏介绍页: `design/界面设计.md` L643-696
- 项目素材页: `design/界面设计.md` L686-761
- 项目目录结构: `design/文件命名与组织规则.md` L8-106

---

## 总体路线图（Phase 3b 在全局中的位置）

| 阶段 | 内容 | 状态 |
|------|------|------|
| **Phase 1** | 脚手架 + 设计系统 + 主窗口框架 | ✅ 已完成 |
| **Phase 2** | 三级页面导航 + Vue Router + 页面骨架 | ✅ 已完成 |
| **Phase 3a** | 项目卡片 + 任务卡片 + Rust 文件系统后端 | ✅ 已完成 |
| **Phase 3b**（本文档） | 普通卡片 + 辅助页面文件扫描 + 打开文件夹 | 🔵 当前 |
| **Phase 4** | 素材卡片 + 双视图模式 + 预览图系统 + 侧边栏详情 | ⬜ |
| **Phase 5** | 文件拖拽 + 任务管理系统 + 工作流功能 | ⬜ |
| **Phase 6** | 日报打卡 + 翻译 + 转换进度悬浮窗 + 设置页 | ⬜ |

---

## Task 1: Rust 新增数据模型 + 扫描命令

**Files:**
- Modify: `src-tauri/src/models.rs`（新增 FileEntry）
- Modify: `src-tauri/src/commands.rs`（新增 scan_directory, open_in_explorer）
- Modify: `src-tauri/src/lib.rs`（注册新命令）

**Step 1: 新增 FileEntry 数据模型**

在 `src-tauri/src/models.rs` 末尾添加：

```rust
/// 通用文件/目录条目（普通卡片用）
#[derive(Debug, Serialize, Clone)]
pub struct FileEntry {
    /// 文件/目录名
    pub name: String,
    /// 完整路径
    pub path: String,
    /// 是否为目录
    pub is_dir: bool,
    /// 文件大小（字节），目录为 0
    pub size_bytes: u64,
    /// 文件扩展名（小写，无点号），如 "png"、"mp4"
    pub extension: String,
}
```

**Step 2: 新增 scan_directory 命令**

在 `src-tauri/src/commands.rs` 添加：

```rust
/// 扫描指定目录，返回文件和子目录列表（非递归，只扫一层）
#[tauri::command]
pub fn scan_directory(dir_path: String) -> Result<Vec<FileEntry>, String> {
    let dir = Path::new(&dir_path);
    if !dir.exists() {
        return Ok(Vec::new()); // 目录不存在返回空列表，不报错
    }

    let mut entries = Vec::new();
    let dir_entries = fs::read_dir(dir).map_err(|e| format!("无法读取目录: {}", e))?;

    for entry in dir_entries {
        let entry = entry.map_err(|e| format!("读取目录项失败: {}", e))?;
        let path = entry.path();

        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        // 跳过隐藏文件/目录
        if name.starts_with('.') {
            continue;
        }

        let is_dir = path.is_dir();
        let size_bytes = if is_dir { 0 } else { path.metadata().map(|m| m.len()).unwrap_or(0) };
        let extension = if is_dir {
            String::new()
        } else {
            path.extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_lowercase()
        };

        entries.push(FileEntry {
            name,
            path: path.to_string_lossy().to_string(),
            is_dir,
            size_bytes,
            extension,
        });
    }

    // 目录在前，文件在后；各自按名称排序
    entries.sort_by(|a, b| {
        b.is_dir.cmp(&a.is_dir).then_with(|| a.name.cmp(&b.name))
    });

    Ok(entries)
}

/// 在系统文件管理器中打开指定路径
#[tauri::command]
pub fn open_in_explorer(path: String) -> Result<(), String> {
    let target = Path::new(&path);
    if !target.exists() {
        return Err(format!("路径不存在: {}", path));
    }

    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer")
            .arg(target)
            .spawn()
            .map_err(|e| format!("打开文件管理器失败: {}", e))?;
    }

    Ok(())
}
```

**Step 3: 在 lib.rs 注册新命令**

```rust
.invoke_handler(tauri::generate_handler![
    commands::scan_projects,
    commands::scan_tasks,
    commands::scan_directory,
    commands::open_in_explorer,
])
```

**Step 4: 验证编译**

```bash
cd D:/work/pgsoft/PGB1/src-tauri && cargo check
```

---

## Task 2: 前端数据层

**Files:**
- Create: `src/composables/useDirectoryFiles.ts`

**Step 1: 创建 useDirectoryFiles composable**

```typescript
import { ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'

export interface FileEntry {
  name: string
  path: string
  is_dir: boolean
  size_bytes: number
  extension: string
}

export function useDirectoryFiles() {
  const files = ref<FileEntry[]>([])
  const loading = ref(false)
  const error = ref<string | null>(null)

  async function loadFiles(dirPath: string) {
    loading.value = true
    error.value = null
    try {
      files.value = await invoke<FileEntry[]>('scan_directory', { dirPath })
    } catch (e) {
      error.value = String(e)
      console.error('扫描目录失败:', e)
    } finally {
      loading.value = false
    }
  }

  async function openInExplorer(path: string) {
    try {
      await invoke('open_in_explorer', { path })
    } catch (e) {
      console.error('打开文件管理器失败:', e)
    }
  }

  return { files, loading, error, loadFiles, openInExplorer }
}
```

**Step 2: 验证 TypeScript**

```bash
cd D:/work/pgsoft/PGB1 && npx vue-tsc --noEmit
```

---

## Task 3: NormalCard 组件

**Files:**
- Create: `src/components/NormalCard.vue`

**Step 1: 创建普通卡片组件**

设计规范（`design/卡片设计.md` L182-214）：
- 200×240px
- 上方：预览区域 150px（4:3），右上角格式标签
- 下方：文件名
- 预览区：Phase 3b 先用文件类型图标占位，Phase 4 接入预览图系统

```vue
<script setup lang="ts">
import type { FileEntry } from '../composables/useDirectoryFiles'

defineProps<{
  file: FileEntry
}>()

defineEmits<{
  click: [file: FileEntry]
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
    class="normal-card glass-subtle"
    @click="$emit('click', file)"
  >
    <!-- 预览区域 -->
    <div class="card-preview">
      <!-- 格式标签（右上角） -->
      <span v-if="file.extension" class="format-tag">
        {{ file.extension.toUpperCase() }}
      </span>
      <span v-else class="format-tag">DIR</span>

      <!-- 类型图标占位 -->
      <svg v-if="file.is_dir" class="type-icon" width="40" height="40" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
        <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z" />
      </svg>
      <svg v-else class="type-icon" width="40" height="40" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
        <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z" />
        <polyline points="14 2 14 8 20 8" />
      </svg>
    </div>

    <!-- 文件信息 -->
    <div class="card-info">
      <span class="card-name">{{ file.name }}</span>
      <span v-if="!file.is_dir" class="card-size">{{ formatSize(file.size_bytes) }}</span>
    </div>
  </button>
</template>

<style scoped>
.normal-card {
  width: var(--card-normal-width);
  height: var(--card-normal-height);
  display: flex;
  flex-direction: column;
  padding: var(--card-normal-padding);
  border-radius: var(--card-border-radius);
  border: none;
  cursor: pointer;
  transition: var(--transition-card-hover);
  text-align: left;
  overflow: hidden;
}

.normal-card:hover {
  transform: translateY(var(--card-hover-lift));
  box-shadow: var(--card-shadow-hover);
}

.card-preview {
  position: relative;
  width: 100%;
  height: var(--card-normal-preview-height);
  display: flex;
  align-items: center;
  justify-content: center;
  background: var(--bg-hover);
  border-radius: var(--radius-md);
  color: var(--text-tertiary);
}

.format-tag {
  position: absolute;
  top: var(--spacing-2);
  right: var(--spacing-2);
  display: inline-flex;
  align-items: center;
  height: var(--tag-height);
  padding: 0 var(--tag-padding-x);
  font-size: var(--tag-font-size);
  font-weight: var(--tag-font-weight);
  border-radius: var(--tag-border-radius);
  background: var(--tag-format-bg);
  color: var(--tag-format-text);
  border: 1px solid var(--tag-format-border);
  backdrop-filter: blur(8px);
}

.type-icon {
  opacity: 0.5;
}

.card-info {
  flex: 1;
  display: flex;
  flex-direction: column;
  justify-content: center;
  gap: var(--spacing-1);
  padding-top: var(--card-normal-gap);
  min-width: 0;
}

.card-name {
  font-size: var(--text-base);
  font-weight: var(--font-weight-heading);
  color: var(--text-primary);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.card-size {
  font-size: var(--text-xs);
  color: var(--text-tertiary);
}
</style>
```

---

## Task 4: GameIntroPage 改造

**Files:**
- Modify: `src/views/GameIntroPage.vue`

**Step 1: 接入真实数据**

游戏介绍页对应 `00_Game Design & Doc/` 目录。

需要：
1. 通过 projectId 找到项目路径（复用 useProjects）
2. 拼接 `{projectPath}/00_Game Design & Doc` 路径
3. 用 useDirectoryFiles 扫描
4. 用 NormalCard 渲染
5. "打开文件夹"按钮调用 openInExplorer
6. "刷新"按钮重新调用 loadFiles

---

## Task 5: MaterialsPage 改造

**Files:**
- Modify: `src/views/MaterialsPage.vue`

**Step 1: 接入真实数据**

项目素材页对应多个目录：
- `01_Preproduction/`
- `02_Production/`
- `03_Render_VFX/VFX/PSD/`（显示为 "PSD 源文件"）
- `05_Outside/`

需要：
1. 通过 projectId 找到项目路径
2. 分组扫描四个目录
3. 每组显示标题 + NormalCard 网格
4. 空目录隐藏
5. 每组标题旁有文件夹图标按钮（打开对应目录）

---

## Task 6: 补全所有"打开文件夹"占位符

**Files:**
- Modify: `src/views/TaskPage.vue`（打开任务文件夹 + 打开 nextcloud）
- Modify: `src/views/ProjectPage.vue`（打开项目文件夹）

**Step 1: TaskPage — 打开任务文件夹 + nextcloud**

需要通过 projectId + taskId 拼接路径：
- 任务文件夹: `{projectPath}/03_Render_VFX/VFX/Export/{taskId}`
- nextcloud: `{projectPath}/03_Render_VFX/VFX/nextcloud/{taskId}`

**Step 2: ProjectPage — 打开项目文件夹**

直接用项目路径调用 openInExplorer。

---

## Task 7: 构建验证与收尾

**Step 1: TypeScript 检查**
```bash
cd D:/work/pgsoft/PGB1 && npx vue-tsc --noEmit
```

**Step 2: Vite 构建**
```bash
cd D:/work/pgsoft/PGB1 && npm run build
```

**Step 3: Cargo 编译**
```bash
cd D:/work/pgsoft/PGB1/src-tauri && cargo build
```

**Step 4: 验证清单**
- [ ] 游戏介绍页显示 `00_Game Design & Doc/` 下的文件卡片（PDF、MP4、ODOC 等）
- [ ] 普通卡片显示文件名 + 格式标签 + 大小
- [ ] 项目素材页分组展示四个目录的文件
- [ ] 空目录自动隐藏
- [ ] 所有"打开文件夹"按钮能打开系统文件管理器
- [ ] 刷新按钮能重新加载文件列表
- [ ] 明暗主题切换正常

**Step 5: 更新设计文档**
- 更新 `design/DesignSystem.md` 普通卡片尺寸确认
- 更新计划文档实施记录

---

## Phase 3b 实施记录

**完成时间:** 2026-02-17

### 已完成

- [x] Task 1: Rust FileEntry 模型 + scan_directory / open_in_explorer 命令
- [x] Task 2: useDirectoryFiles composable
- [x] Task 3: NormalCard.vue 普通卡片组件
- [x] Task 4: GameIntroPage 改造（展示 00_Game Design & Doc 文件）
- [x] Task 5: MaterialsPage 改造（四组分组展示 + 空目录隐藏）
- [x] Task 6: 补全所有"打开文件夹"占位符（TaskPage×2 + ProjectPage×1）
- [x] Task 7: 构建验证（vue-tsc + vite build + cargo build 全部通过）

### 遗留问题

- 普通卡片预览区目前用类型图标占位，Phase 4 接入真实预览图
- 点击普通卡片目前直接打开文件管理器，Phase 4 改为打开侧边栏详情
