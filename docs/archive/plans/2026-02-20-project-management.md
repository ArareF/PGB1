# Project Management Actions Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 在 ProjectCard hover 菜单中添加重命名项目、修改截止日期、删除项目三个管理操作。

**Architecture:** 后端新增 3 个 Tauri 命令（rename_project / update_project_deadline / delete_project）；前端 ProjectCard 增加 hover 时 ··· 菜单，点击打开新建的 EditProjectDialog 组件处理三种操作；HomePage 监听事件后刷新列表。

**Tech Stack:** Rust/Tauri 2.x（后端命令），Vue 3 Composition API（前端组件），现有 design-system CSS 变量（样式）

---

## 背景

- 项目配置文件：`{project_path}/.pgb1_project.json`，结构见 `src-tauri/src/models.rs` 的 `ProjectConfig`
- 重命名 = 磁盘目录改名 + config 的 `project_name` 字段更新
- 截止日期字段已在 `ProjectConfig.deadline: Option<String>` 中存在
- 删除 = `fs::remove_dir_all`，不可逆，需二次确认
- `normalizeDeadline` 日期格式化逻辑已在 `CreateProjectDialog.vue` 中实现，直接复用

---

### Task 1: 后端 — 新增 `update_project_deadline` 命令

**Files:**
- Modify: `src-tauri/src/commands.rs`（在文件末尾添加）
- Modify: `src-tauri/src/lib.rs`（注册命令）

**Step 1: 在 commands.rs 末尾添加命令**

找到文件末尾，添加：

```rust
/// 更新项目截止日期
#[tauri::command]
pub fn update_project_deadline(
    project_path: String,
    deadline: Option<String>,
) -> Result<(), String> {
    let config_path = Path::new(&project_path).join(".pgb1_project.json");
    let mut config = load_config_from_path(&config_path)?;
    config.deadline = deadline;
    save_config_to_path(&config_path, &config)
}
```

> 注意：`load_config_from_path` 和 `save_config_to_path` 是已有的辅助函数（grep 确认名称）。
> 若无此辅助函数，则手动读写：
> ```rust
> let raw = fs::read_to_string(&config_path).map_err(|e| format!("读取配置失败: {}", e))?;
> let mut config: ProjectConfig = serde_json::from_str(&raw).map_err(|e| format!("解析配置失败: {}", e))?;
> config.deadline = deadline;
> let json = serde_json::to_string_pretty(&config).map_err(|e| format!("序列化失败: {}", e))?;
> fs::write(&config_path, json).map_err(|e| format!("写入配置失败: {}", e))?;
> Ok(())
> ```

**Step 2: 在 lib.rs 的 invoke_handler 列表中注册**

在 `commands::set_default_ae_file,` 附近添加：
```rust
commands::update_project_deadline,
```

**Step 3: cargo check 验证编译**

```bash
cd src-tauri && cargo check 2>&1
```
预期：无 error，只有可能的 warning。

---

### Task 2: 后端 — 新增 `delete_project` 命令

**Files:**
- Modify: `src-tauri/src/commands.rs`（接 Task 1 末尾继续添加）
- Modify: `src-tauri/src/lib.rs`

**Step 1: 添加命令**

```rust
/// 删除整个项目目录（不可逆）
#[tauri::command]
pub fn delete_project(project_path: String) -> Result<(), String> {
    let path = Path::new(&project_path);
    if !path.exists() {
        return Err(format!("项目目录不存在: {}", project_path));
    }
    // 安全检查：必须包含 .pgb1_project.json，防止误删非项目目录
    if !path.join(".pgb1_project.json").exists() {
        return Err("目标目录不是有效的 PGB1 项目（缺少 .pgb1_project.json）".to_string());
    }
    fs::remove_dir_all(path).map_err(|e| format!("删除项目失败: {}", e))
}
```

**Step 2: 在 lib.rs 注册**

```rust
commands::delete_project,
```

**Step 3: cargo check**

---

### Task 3: 后端 — 新增 `rename_project` 命令

**Files:**
- Modify: `src-tauri/src/commands.rs`
- Modify: `src-tauri/src/lib.rs`

**Step 1: 添加命令**

```rust
/// 重命名项目（改目录名 + 更新 config 中的 project_name）
#[tauri::command]
pub fn rename_project(project_path: String, new_name: String) -> Result<ProjectInfo, String> {
    let trimmed = new_name.trim();
    if trimmed.is_empty() {
        return Err("项目名称不能为空".to_string());
    }

    // 校验非法字符（与 create_project 一致）
    const ILLEGAL_CHARS: &[char] = &['<', '>', ':', '"', '/', '\\', '|', '?', '*'];
    if trimmed.chars().any(|c| ILLEGAL_CHARS.contains(&c)) {
        return Err(format!(
            "项目名称包含非法字符，不能使用: {}",
            ILLEGAL_CHARS.iter().collect::<String>()
        ));
    }

    let old_path = Path::new(&project_path);
    if !old_path.exists() {
        return Err(format!("项目目录不存在: {}", project_path));
    }

    let parent = old_path
        .parent()
        .ok_or("无法获取父目录")?;
    let new_path = parent.join(trimmed);

    if new_path.exists() {
        return Err(format!("同名项目已存在: {}", trimmed));
    }

    // 重命名目录
    fs::rename(old_path, &new_path)
        .map_err(|e| format!("重命名目录失败: {}", e))?;

    // 更新 .pgb1_project.json 中的 project_name
    let config_path = new_path.join(".pgb1_project.json");
    let raw = fs::read_to_string(&config_path)
        .map_err(|e| format!("读取配置失败: {}", e))?;
    let mut config: ProjectConfig = serde_json::from_str(&raw)
        .map_err(|e| format!("解析配置失败: {}", e))?;
    config.project_name = trimmed.to_string();
    let json = serde_json::to_string_pretty(&config)
        .map_err(|e| format!("序列化配置失败: {}", e))?;
    fs::write(&config_path, json)
        .map_err(|e| format!("写入配置失败: {}", e))?;

    // 返回新的 ProjectInfo（重新扫描单个项目）
    // 复用 load_or_create_config 和 scan_task_names
    let config = load_or_create_config(&new_path)?;
    let export_path = new_path.join("03_Render_VFX").join("VFX").join("Export");
    let tasks = if export_path.exists() {
        scan_task_names(&export_path)?
    } else {
        Vec::new()
    };
    let task_count = tasks.len();
    let app_icon = find_app_icon(&new_path.join("01_Preproduction"));

    Ok(ProjectInfo {
        name: trimmed.to_string(),
        path: new_path.to_string_lossy().to_string(),
        deadline: config.deadline,
        tasks,
        task_count,
        enabled_tasks: config.enabled_tasks,
        completed_subtasks: config.completed_subtasks,
        upload_prompted_tasks: config.upload_prompted_tasks,
        completed_tasks: Vec::new(), // 重命名后不重算上传进度，HomePage 会 loadProjects 刷新
        default_ae_file: config.default_ae_file,
        app_icon,
    })
}
```

**Step 2: 在 lib.rs 注册**

```rust
commands::rename_project,
```

**Step 3: cargo check**

```bash
cd src-tauri && cargo check 2>&1
```
预期：无 error。

---

### Task 4: 前端 — 新建 `EditProjectDialog.vue`

**Files:**
- Create: `src/components/EditProjectDialog.vue`

此组件通过 `mode` prop 控制显示哪种弹窗（rename / deadline / delete）。

**Step 1: 创建组件**

```vue
<script setup lang="ts">
import { ref, computed } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import type { ProjectInfo } from '../composables/useProjects'

const props = defineProps<{
  project: ProjectInfo
  mode: 'rename' | 'deadline' | 'delete'
}>()

const emit = defineEmits<{
  updated: [project: ProjectInfo]
  deleted: [projectPath: string]
  cancel: []
}>()

const inputValue = ref(
  props.mode === 'rename' ? props.project.name :
  props.mode === 'deadline' ? (props.project.deadline ?? '') : ''
)
const errorMsg = ref('')
const loading = ref(false)

const title = computed(() => {
  if (props.mode === 'rename') return '重命名项目'
  if (props.mode === 'deadline') return '修改截止日期'
  return '删除项目'
})

const confirmLabel = computed(() => {
  if (props.mode === 'delete') return loading.value ? '删除中...' : '确认删除'
  return loading.value ? '保存中...' : '保存'
})

/** 将用户输入的日期标准化为 YYYY-MM-DD（复用 CreateProjectDialog 的逻辑） */
function normalizeDeadline(raw: string): string | null {
  const trimmed = raw.trim()
  if (!trimmed) return null
  if (/^\d{8}$/.test(trimmed)) {
    return `${trimmed.slice(0, 4)}-${trimmed.slice(4, 6)}-${trimmed.slice(6, 8)}`
  }
  if (/^\d{4}[-/]\d{1,2}[-/]\d{1,2}$/.test(trimmed)) {
    const parts = trimmed.split(/[-/]/)
    return `${parts[0]}-${parts[1].padStart(2, '0')}-${parts[2].padStart(2, '0')}`
  }
  return trimmed
}

async function handleConfirm() {
  errorMsg.value = ''
  loading.value = true
  try {
    if (props.mode === 'rename') {
      const updated = await invoke<ProjectInfo>('rename_project', {
        projectPath: props.project.path,
        newName: inputValue.value.trim(),
      })
      emit('updated', updated)
    } else if (props.mode === 'deadline') {
      await invoke('update_project_deadline', {
        projectPath: props.project.path,
        deadline: normalizeDeadline(inputValue.value),
      })
      emit('updated', { ...props.project, deadline: normalizeDeadline(inputValue.value) })
    } else {
      await invoke('delete_project', {
        projectPath: props.project.path,
      })
      emit('deleted', props.project.path)
    }
  } catch (e) {
    errorMsg.value = String(e)
  } finally {
    loading.value = false
  }
}
</script>

<template>
  <Teleport to="body">
    <div class="dialog-overlay" @click.self="$emit('cancel')">
      <div class="dialog-content glass-strong">
        <p class="dialog-title">{{ title }}</p>

        <div class="dialog-body">
          <!-- 重命名 / 截止日期：输入框 -->
          <template v-if="mode !== 'delete'">
            <label class="field-label">
              {{ mode === 'rename' ? '新项目名称' : '截止日期' }}
            </label>
            <input
              v-model="inputValue"
              class="field-input"
              type="text"
              :placeholder="mode === 'rename' ? '如 218_NewGame' : 'YYYY-MM-DD'"
              autofocus
              @keydown.enter="handleConfirm"
              @keydown.esc="$emit('cancel')"
            />
          </template>

          <!-- 删除：警告文案 -->
          <template v-else>
            <p class="delete-warning">
              确定要删除项目 <strong>{{ project.name }}</strong> 吗？
            </p>
            <p class="delete-danger">此操作将永久删除项目目录及所有文件，无法恢复。</p>
          </template>

          <p v-if="errorMsg" class="error-text">{{ errorMsg }}</p>
        </div>

        <div class="dialog-actions">
          <button
            class="dialog-btn"
            :class="mode === 'delete' ? 'dialog-btn-danger' : 'dialog-btn-primary'"
            :disabled="loading"
            @click="handleConfirm"
          >
            {{ confirmLabel }}
          </button>
          <button class="dialog-btn dialog-btn-secondary" @click="$emit('cancel')">
            取消
          </button>
        </div>
      </div>
    </div>
  </Teleport>
</template>

<style scoped>
.dialog-overlay {
  position: fixed;
  inset: 0;
  z-index: var(--z-modal, 1000);
  display: flex;
  align-items: center;
  justify-content: center;
  background: rgba(0, 0, 0, 0.4);
  backdrop-filter: blur(4px);
}

.dialog-content {
  min-width: 400px;
  max-width: 480px;
  border-radius: var(--floating-navbar-radius);
  padding: var(--spacing-6);
  display: flex;
  flex-direction: column;
  gap: var(--spacing-5);
}

.dialog-title {
  font-size: var(--text-2xl);
  font-weight: var(--font-weight-heading);
  color: var(--text-primary);
}

.dialog-body {
  display: flex;
  flex-direction: column;
  gap: var(--spacing-3);
}

.field-label {
  font-size: var(--text-base);
  color: var(--text-secondary);
}

.field-input {
  height: var(--button-height);
  padding: 0 var(--spacing-3);
  font-size: var(--text-base);
  color: var(--text-primary);
  background: var(--bg-elevated);
  border: 1px solid var(--border-medium);
  border-radius: var(--radius-md);
  outline: none;
  transition: border-color var(--transition-fast);
}

.field-input:focus {
  border-color: var(--color-primary);
}

.field-input::placeholder {
  color: var(--text-tertiary);
}

.delete-warning {
  font-size: var(--text-base);
  color: var(--text-primary);
}

.delete-danger {
  font-size: var(--text-sm);
  color: var(--color-error, #ef4444);
}

.error-text {
  font-size: var(--text-sm);
  color: var(--color-error, #ef4444);
}

.dialog-actions {
  display: flex;
  justify-content: flex-end;
  gap: var(--spacing-3);
}

.dialog-btn {
  display: inline-flex;
  align-items: center;
  height: var(--button-height);
  padding: 0 var(--spacing-5);
  font-size: var(--text-base);
  font-weight: var(--font-weight-heading);
  border-radius: var(--radius-md);
  border: none;
  cursor: pointer;
  transition: all var(--transition-fast);
}

.dialog-btn:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.dialog-btn-primary {
  background: rgba(33, 150, 243, 0.75);
  backdrop-filter: blur(8px);
  -webkit-backdrop-filter: blur(8px);
  color: var(--color-neutral-0);
}

.dialog-btn-primary:hover:not(:disabled) {
  background: rgba(33, 150, 243, 0.9);
}

.dialog-btn-danger {
  background: rgba(239, 68, 68, 0.75);
  backdrop-filter: blur(8px);
  -webkit-backdrop-filter: blur(8px);
  color: var(--color-neutral-0);
}

.dialog-btn-danger:hover:not(:disabled) {
  background: rgba(239, 68, 68, 0.9);
}

.dialog-btn-secondary {
  background: transparent;
  color: var(--text-secondary);
  border: 1px solid var(--border-medium);
}

.dialog-btn-secondary:hover {
  background: var(--bg-hover);
  color: var(--text-primary);
}
</style>
```

---

### Task 5: 前端 — 修改 `ProjectCard.vue`（添加 hover 菜单）

**Files:**
- Modify: `src/components/ProjectCard.vue`

**Step 1: 在 `<script setup>` 中增加状态和 emits**

在现有 imports 后添加：
```typescript
const showMenu = ref(false)

const emit = defineEmits<{
  click: [project: ProjectInfo]
  action: [project: ProjectInfo, action: 'rename' | 'deadline' | 'delete']
}>()
```
> 注意：现有的 `defineEmits` 只有 `click`，需要**替换**为上面带 `action` 的版本。

**Step 2: 在 `<template>` 中添加 ··· 按钮和菜单**

在 `</button>` 闭合标签前添加（注意：在最外层 `<button>` 内部的末尾）：

```html
<!-- ··· 菜单按钮（hover 时显示） -->
<button
  class="card-menu-btn"
  :class="{ visible: showMenu }"
  @click.stop="showMenu = !showMenu"
  @blur="showMenu = false"
>
  ···
</button>

<!-- 下拉菜单 -->
<div v-if="showMenu" class="card-menu" @click.stop>
  <button class="menu-item" @mousedown.prevent="$emit('action', project, 'rename')">
    重命名
  </button>
  <button class="menu-item" @mousedown.prevent="$emit('action', project, 'deadline')">
    修改截止日期
  </button>
  <button class="menu-item menu-item--danger" @mousedown.prevent="$emit('action', project, 'delete')">
    删除项目
  </button>
</div>
```

> `@mousedown.prevent` 防止按钮 blur 事件先于 click 触发导致菜单消失。

**Step 3: 在 `<style scoped>` 末尾添加样式**

```css
/* 使外层 button 支持绝对定位子元素 */
.project-card {
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
  transition: opacity var(--transition-fast), background var(--transition-fast);
  line-height: 1;
  padding-bottom: 4px; /* 视觉居中微调 */
}

.project-card:hover .card-menu-btn,
.card-menu-btn.visible {
  opacity: 1;
}

.card-menu-btn:hover {
  background: var(--bg-hover);
  color: var(--text-primary);
}

.card-menu {
  position: absolute;
  top: calc(var(--spacing-2) + 28px + 4px);
  right: var(--spacing-2);
  min-width: 140px;
  background: var(--bg-elevated);
  border: 1px solid var(--border-medium);
  border-radius: var(--radius-md);
  box-shadow: var(--shadow-lg, 0 8px 24px rgba(0,0,0,0.2));
  overflow: hidden;
  z-index: 10;
}

.menu-item {
  display: block;
  width: 100%;
  padding: var(--spacing-2) var(--spacing-4);
  font-size: var(--text-base);
  color: var(--text-primary);
  background: transparent;
  border: none;
  text-align: left;
  cursor: pointer;
  transition: background var(--transition-fast);
}

.menu-item:hover {
  background: var(--bg-hover);
}

.menu-item--danger {
  color: var(--color-error, #ef4444);
}

.menu-item--danger:hover {
  background: rgba(239, 68, 68, 0.1);
}
```

---

### Task 6: 前端 — 修改 `HomePage.vue`（接收 action 事件）

**Files:**
- Modify: `src/views/HomePage.vue`

**Step 1: 在 `<script setup>` 中添加弹窗状态**

在现有 imports 中添加：
```typescript
import EditProjectDialog from '../components/EditProjectDialog.vue'
import type { ProjectInfo } from '../composables/useProjects'
```

在现有 `const showCreateDialog = ref(false)` 附近添加：
```typescript
const editTarget = ref<ProjectInfo | null>(null)
const editMode = ref<'rename' | 'deadline' | 'delete' | null>(null)

function onProjectAction(project: ProjectInfo, action: 'rename' | 'deadline' | 'delete') {
  editTarget.value = project
  editMode.value = action
}

function onProjectUpdated(updated: ProjectInfo) {
  editTarget.value = null
  editMode.value = null
  loadProjects()
}

function onProjectDeleted() {
  editTarget.value = null
  editMode.value = null
  loadProjects()
}

function closeEditDialog() {
  editTarget.value = null
  editMode.value = null
}
```

**Step 2: 在 `<template>` 中绑定事件并添加弹窗**

将 `<ProjectCard>` 的 `@click` 保持不变，添加 `@action`：
```html
<ProjectCard
  v-for="project in projects"
  :key="project.name"
  :project="project"
  @click="openProject"
  @action="onProjectAction"
/>
```

在 `<CreateProjectDialog>` 下方添加：
```html
<EditProjectDialog
  v-if="editTarget && editMode"
  :project="editTarget"
  :mode="editMode"
  @updated="onProjectUpdated"
  @deleted="onProjectDeleted"
  @cancel="closeEditDialog"
/>
```

---

### Task 7: 验证

**Step 1: 启动开发服务器**

```bash
cd D:/work/pgsoft/PGB1 && npm run tauri dev 2>&1
```

**Step 2: 手动验证**

1. Hover ProjectCard → 右上角出现 ··· 按钮
2. 点击 ··· → 菜单展开，三项可见
3. 点击"重命名" → 弹窗出现，输入新名称确认 → 磁盘目录改名，列表刷新
4. 点击"修改截止日期" → 弹窗出现，输入新日期 → 卡片截止日期更新
5. 点击"删除项目" → 弹出红色确认弹窗 → 确认后项目从列表消失，目录已删除
6. 点击项目卡片本体 → 正常进入项目页（不误触发菜单）

**Step 3: 边界情况**

- 重命名为已存在的名称 → 弹窗显示后端错误信息
- 删除后点取消 → 项目不删除，列表不刷新
