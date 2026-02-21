# 侧边栏重命名/删除 + fps 显示 实现计划

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 在 TaskPage 素材侧边栏中补全 fps 显示、重命名按钮、删除按钮三个功能。

**Architecture:**
- Task 1：纯前端——在侧边栏基本信息区补全 fps 行
- Task 2：Rust 后端——新增 `rename_material` 命令
- Task 3：Rust 后端——新增 `delete_material` 命令
- Task 4：注册两个新命令到 invoke_handler
- Task 5：前端——添加悬浮操作按钮 + 内联弹窗交互

**Tech Stack:** Vue 3 (Composition API) + Tauri 2.x invoke + Rust std::fs

---

## Task 1：前端 — 侧边栏 fps 行

**Files:**
- Modify: `src/views/TaskPage.vue`（侧边栏基本信息区，约第 719 行附近）

### Step 1：在「帧数」行下方增加「帧率」行

在以下代码块（`v-if="selectedMaterial.material_type === 'sequence'"` 帧数行）后面紧接插入：

```html
<div v-if="selectedMaterial.material_type === 'sequence'" class="info-row">
  <span class="info-label">帧率</span>
  <span class="info-value">
    {{ selectedMaterial.fps != null ? selectedMaterial.fps + ' fps' : '未转换' }}
  </span>
</div>
```

**当前帧数行位置**（TaskPage.vue ~719）：
```html
<div v-if="selectedMaterial.material_type === 'sequence'" class="info-row">
  <span class="info-label">帧数</span>
  <span class="info-value">{{ selectedMaterial.frame_count }}</span>
</div>
```

### Step 2：验证

运行 `pnpm tauri dev`，点击一个序列帧素材卡片，侧边栏应显示：
- 帧数：N
- 帧率：Xfps（已转换）或「未转换」（未转换）

---

## Task 2：Rust 后端 — `rename_material` 命令

**Files:**
- Modify: `src-tauri/src/commands.rs`（末尾追加，第 4241 行后）

### Step 1：在 commands.rs 末尾追加命令

```rust
/// 重命名素材（所有工作流版本同步改名，包括 nextcloud）
///
/// 逻辑：
///   - 扫描 00_original / 01_scale/**/ / 02_done/**/ / nextcloud/<task>/ 中
///     所有文件名或目录名以 `base_name` 开头的条目
///   - 将名称中的 `base_name` 前缀替换为 `new_base_name`（保留后缀，如 _01、[50] 等）
///   - 静帧：重命名文件；序列帧：重命名目录 + 内部所有帧文件前缀
#[tauri::command]
pub fn rename_material(
    task_path: String,
    base_name: String,
    new_base_name: String,
    material_type: String,
) -> Result<(), String> {
    use std::path::Path;

    let task_dir = Path::new(&task_path);
    let is_sequence = material_type == "sequence";

    // 构建要扫描的目录列表
    let mut dirs_to_scan: Vec<std::path::PathBuf> = vec![
        task_dir.join("00_original"),
    ];

    // 01_scale 的所有子目录
    let scale_dir = task_dir.join("01_scale");
    if scale_dir.exists() {
        if let Ok(entries) = fs::read_dir(&scale_dir) {
            for e in entries.flatten() {
                if e.path().is_dir() {
                    dirs_to_scan.push(e.path());
                }
            }
        }
    }

    // 02_done 的所有子目录
    let done_dir = task_dir.join("02_done");
    if done_dir.exists() {
        if let Ok(entries) = fs::read_dir(&done_dir) {
            for e in entries.flatten() {
                if e.path().is_dir() {
                    dirs_to_scan.push(e.path());
                }
            }
        }
    }

    // nextcloud/<task_name>/
    let nc_dir = task_dir
        .parent()
        .and_then(|p| p.parent())
        .map(|vfx| {
            vfx.join("nextcloud")
                .join(task_dir.file_name().unwrap_or_default())
        });
    if let Some(ref nc) = nc_dir {
        if nc.exists() {
            dirs_to_scan.push(nc.clone());
        }
    }

    // 在每个目录中查找并重命名
    for dir in &dirs_to_scan {
        if !dir.exists() {
            continue;
        }
        let entries = match fs::read_dir(dir) {
            Ok(e) => e,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let file_name = match path.file_name().and_then(|n| n.to_str()) {
                Some(n) => n.to_string(),
                None => continue,
            };
            if !file_name.starts_with(&*base_name) {
                continue;
            }
            // 替换前缀
            let suffix = &file_name[base_name.len()..];
            let new_name = format!("{}{}", new_base_name, suffix);
            let new_path = dir.join(&new_name);

            if is_sequence && path.is_dir() {
                // 序列帧：先重命名目录内部所有帧文件
                if let Ok(frames) = fs::read_dir(&path) {
                    for frame_entry in frames.flatten() {
                        let fpath = frame_entry.path();
                        let fname = match fpath.file_name().and_then(|n| n.to_str()) {
                            Some(n) => n.to_string(),
                            None => continue,
                        };
                        if fname.starts_with(&*base_name) {
                            let fsuffix = &fname[base_name.len()..];
                            let new_fname = format!("{}{}", new_base_name, fsuffix);
                            let _ = fs::rename(&fpath, fpath.parent().unwrap().join(&new_fname));
                        }
                    }
                }
                // 再重命名目录本身
                fs::rename(&path, &new_path)
                    .map_err(|e| format!("重命名目录 {} 失败: {}", file_name, e))?;
            } else if !path.is_dir() {
                // 静帧：直接重命名文件
                fs::rename(&path, &new_path)
                    .map_err(|e| format!("重命名文件 {} 失败: {}", file_name, e))?;
            }
        }
    }

    Ok(())
}
```

---

## Task 3：Rust 后端 — `delete_material` 命令

**Files:**
- Modify: `src-tauri/src/commands.rs`（紧接 rename_material 后追加）

### Step 1：追加命令

```rust
/// 删除素材的所有工作流版本（包括 nextcloud）
///
/// 逻辑：
///   - 扫描 00_original / 01_scale/**/ / 02_done/**/ / nextcloud/<task>/ 中
///     所有文件名或目录名以 `base_name` 开头的条目，全部删除
#[tauri::command]
pub fn delete_material(
    task_path: String,
    base_name: String,
    material_type: String,
) -> Result<(), String> {
    use std::path::Path;

    let task_dir = Path::new(&task_path);
    let is_sequence = material_type == "sequence";

    // 构建要扫描的目录列表（与 rename_material 相同逻辑）
    let mut dirs_to_scan: Vec<std::path::PathBuf> = vec![
        task_dir.join("00_original"),
    ];

    let scale_dir = task_dir.join("01_scale");
    if scale_dir.exists() {
        if let Ok(entries) = fs::read_dir(&scale_dir) {
            for e in entries.flatten() {
                if e.path().is_dir() {
                    dirs_to_scan.push(e.path());
                }
            }
        }
    }

    let done_dir = task_dir.join("02_done");
    if done_dir.exists() {
        if let Ok(entries) = fs::read_dir(&done_dir) {
            for e in entries.flatten() {
                if e.path().is_dir() {
                    dirs_to_scan.push(e.path());
                }
            }
        }
    }

    let nc_dir = task_dir
        .parent()
        .and_then(|p| p.parent())
        .map(|vfx| {
            vfx.join("nextcloud")
                .join(task_dir.file_name().unwrap_or_default())
        });
    if let Some(ref nc) = nc_dir {
        if nc.exists() {
            dirs_to_scan.push(nc.clone());
        }
    }

    for dir in &dirs_to_scan {
        if !dir.exists() {
            continue;
        }
        let entries = match fs::read_dir(dir) {
            Ok(e) => e,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let file_name = match path.file_name().and_then(|n| n.to_str()) {
                Some(n) => n.to_string(),
                None => continue,
            };
            if !file_name.starts_with(&*base_name) {
                continue;
            }
            if is_sequence && path.is_dir() {
                fs::remove_dir_all(&path)
                    .map_err(|e| format!("删除目录 {} 失败: {}", file_name, e))?;
            } else if !path.is_dir() {
                fs::remove_file(&path)
                    .map_err(|e| format!("删除文件 {} 失败: {}", file_name, e))?;
            }
        }
    }

    Ok(())
}
```

---

## Task 4：注册新命令

**Files:**
- Modify: `src-tauri/src/lib.rs`（invoke_handler 列表，在 `commands::fetch_favicon` 后加两行）

### Step 1：在 lib.rs 的 invoke_handler 中追加

找到：
```rust
            commands::fetch_favicon,
        ])
```

改为：
```rust
            commands::fetch_favicon,
            commands::rename_material,
            commands::delete_material,
        ])
```

### Step 2：编译验证

```bash
cd /D/work/pgsoft/PGB1
pnpm tauri build --no-bundle 2>&1 | tail -20
```

期望：`Finished` 无报错。

---

## Task 5：前端 — 悬浮操作按钮 + 内联弹窗

**Files:**
- Modify: `src/views/TaskPage.vue`

### Step 1：在 `<script setup>` 中添加状态变量

在 `const versions = ref<MaterialVersion[]>([])` 下方添加：

```typescript
/** 操作弹窗状态 */
type SidebarDialog = 'none' | 'rename' | 'delete'
const sidebarDialog = ref<SidebarDialog>('none')
const renameInput = ref('')

function openRenameDialog() {
  renameInput.value = selectedMaterial.value?.name ?? ''
  sidebarDialog.value = 'rename'
}

function openDeleteDialog() {
  sidebarDialog.value = 'delete'
}

function closeSidebarDialog() {
  sidebarDialog.value = 'none'
  renameInput.value = ''
}

async function confirmRename() {
  const mat = selectedMaterial.value
  if (!mat || !renameInput.value.trim() || renameInput.value.trim() === mat.name) {
    closeSidebarDialog()
    return
  }
  try {
    await invoke('rename_material', {
      taskPath: taskFolderPath,
      baseName: mat.name,
      newBaseName: renameInput.value.trim(),
      materialType: mat.material_type,
    })
    closeSidebarDialog()
    closeSidebar()
    await refresh()
  } catch (e) {
    console.error('重命名失败:', e)
  }
}

async function confirmDelete() {
  const mat = selectedMaterial.value
  if (!mat) return
  try {
    await invoke('delete_material', {
      taskPath: taskFolderPath,
      baseName: mat.name,
      materialType: mat.material_type,
    })
    closeSidebarDialog()
    closeSidebar()
    await refresh()
  } catch (e) {
    console.error('删除失败:', e)
  }
}
```

### Step 2：在侧边栏模板中添加悬浮按钮和弹窗

在侧边栏 `</div>` 结束标签（`</Transition>` 前的最后一个 `</div>`）之前，紧接 `</div>`（sidebar-body 关闭）后添加：

```html
      <!-- 底部悬浮操作按钮 -->
      <div class="sidebar-actions">
        <button class="sidebar-action-btn" @click="openRenameDialog">重命名</button>
        <button class="sidebar-action-btn danger" @click="openDeleteDialog">删除</button>
      </div>

      <!-- 内联操作弹窗（毛玻璃遮罩覆盖整个侧边栏） -->
      <div v-if="sidebarDialog !== 'none'" class="sidebar-dialog-overlay" @click.self="closeSidebarDialog">
        <!-- 重命名弹窗 -->
        <div v-if="sidebarDialog === 'rename'" class="sidebar-dialog glass-strong">
          <p class="sidebar-dialog-title">重命名</p>
          <input
            v-model="renameInput"
            class="sidebar-dialog-input"
            placeholder="输入新名称"
            autofocus
            @keydown.enter="confirmRename"
            @keydown.escape="closeSidebarDialog"
          />
          <div class="sidebar-dialog-actions">
            <button class="sidebar-dialog-btn" @click="closeSidebarDialog">取消</button>
            <button class="sidebar-dialog-btn primary" @click="confirmRename">确认</button>
          </div>
        </div>
        <!-- 删除确认弹窗 -->
        <div v-if="sidebarDialog === 'delete'" class="sidebar-dialog glass-strong">
          <p class="sidebar-dialog-title">删除素材</p>
          <p class="sidebar-dialog-desc">将删除「{{ selectedMaterial?.name }}」的所有版本文件，包括 nextcloud，操作不可撤销。</p>
          <div class="sidebar-dialog-actions">
            <button class="sidebar-dialog-btn" @click="closeSidebarDialog">取消</button>
            <button class="sidebar-dialog-btn danger" @click="confirmDelete">确认删除</button>
          </div>
        </div>
      </div>
```

### Step 3：在非 scoped `<style>` 块中添加样式

在 `.sidebar-leave-to` 样式后追加：

```css
/* 底部悬浮操作按钮 */
.sidebar-actions {
  position: absolute;
  bottom: var(--spacing-4);
  left: var(--spacing-4);
  right: var(--spacing-4);
  display: flex;
  gap: var(--spacing-3);
  pointer-events: none; /* 防止遮挡下方内容的滚动 */
}

.sidebar-action-btn {
  pointer-events: all;
  flex: 1;
  height: var(--button-height);
  border-radius: var(--radius-md);
  border: 1px solid var(--border-medium);
  background: var(--glass-bg-medium);
  backdrop-filter: blur(8px);
  -webkit-backdrop-filter: blur(8px);
  color: var(--text-secondary);
  font-size: var(--text-sm);
  cursor: pointer;
  transition: all var(--transition-fast);
}

.sidebar-action-btn:hover {
  background: var(--bg-hover);
  color: var(--text-primary);
}

.sidebar-action-btn.danger:hover {
  background: rgba(239, 68, 68, 0.15);
  border-color: rgba(239, 68, 68, 0.5);
  color: rgb(239, 68, 68);
}

/* 侧边栏内联弹窗遮罩 */
.sidebar-dialog-overlay {
  position: absolute;
  inset: 0;
  z-index: 20;
  display: flex;
  align-items: center;
  justify-content: center;
  background: rgba(0, 0, 0, 0.35);
  backdrop-filter: blur(4px);
  -webkit-backdrop-filter: blur(4px);
  border-radius: var(--floating-main-radius);
}

.sidebar-dialog {
  width: calc(100% - var(--spacing-8) * 2);
  border-radius: var(--radius-xl);
  padding: var(--spacing-5);
  display: flex;
  flex-direction: column;
  gap: var(--spacing-4);
}

.sidebar-dialog-title {
  font-size: var(--text-lg);
  font-weight: var(--font-weight-heading);
  color: var(--text-primary);
}

.sidebar-dialog-desc {
  font-size: var(--text-sm);
  color: var(--text-secondary);
  line-height: 1.6;
}

.sidebar-dialog-input {
  width: 100%;
  height: var(--button-height);
  padding: 0 var(--spacing-3);
  border-radius: var(--radius-md);
  border: 1px solid var(--border-medium);
  background: var(--bg-input);
  color: var(--text-primary);
  font-size: var(--text-base);
  font-family: inherit;
  outline: none;
  box-sizing: border-box;
}

.sidebar-dialog-input:focus {
  border-color: var(--color-primary);
}

.sidebar-dialog-actions {
  display: flex;
  gap: var(--spacing-3);
  justify-content: flex-end;
}

.sidebar-dialog-btn {
  height: var(--button-height);
  padding: 0 var(--spacing-4);
  border-radius: var(--radius-md);
  border: 1px solid var(--border-medium);
  background: transparent;
  color: var(--text-secondary);
  font-size: var(--text-sm);
  cursor: pointer;
  transition: all var(--transition-fast);
}

.sidebar-dialog-btn:hover {
  background: var(--bg-hover);
  color: var(--text-primary);
}

.sidebar-dialog-btn.primary {
  background: var(--color-primary);
  border-color: var(--color-primary);
  color: var(--color-neutral-0);
}

.sidebar-dialog-btn.primary:hover {
  opacity: 0.9;
}

.sidebar-dialog-btn.danger {
  background: rgba(239, 68, 68, 0.15);
  border-color: rgba(239, 68, 68, 0.4);
  color: rgb(239, 68, 68);
}

.sidebar-dialog-btn.danger:hover {
  background: rgba(239, 68, 68, 0.25);
}
```

### Step 4：验证

- 打开任务页，点击素材卡片打开侧边栏
- 侧边栏底部应悬浮「重命名」「删除」两个按钮
- 点「重命名」→ 弹出输入框，输入新名称按 Enter → 侧边栏关闭，素材列表刷新，文件已改名
- 点「删除」→ 弹出确认，点「确认删除」→ 侧边栏关闭，素材消失
- 点弹窗遮罩或按 Escape → 弹窗关闭，无副作用

---

## 注意事项

1. **`closeSidebar` 调用前先 `closeSidebarDialog`**：确认操作后要先关弹窗再关侧边栏，防止 v-if 消失导致弹窗动画异常。代码里 `confirmRename` 和 `confirmDelete` 已按顺序调用。

2. **`refresh` 函数**：TaskPage.vue 中已有 `refresh()` 函数（重新加载素材列表），直接复用，无需新增。

3. **侧边栏 `position: relative`**：`detail-sidebar` 已有此样式，内联弹窗的 `position: absolute` 才能生效。

4. **CSS 变量 `--bg-input`**：在 design-system.css 确认此变量存在；若不存在，用 `var(--glass-bg-subtle)` 替代。

5. **`autofocus` 属性**：Vue 内联 `autofocus` 在动态渲染后不一定生效，如果测试时输入框没有自动聚焦，可在 `openRenameDialog` 里加：
   ```typescript
   nextTick(() => {
     (document.querySelector('.sidebar-dialog-input') as HTMLInputElement)?.focus()
   })
   ```
