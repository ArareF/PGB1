# 序列帧侧边栏额外功能 Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 为序列帧素材侧边栏补全两个专属功能：[打开工程文件] 按钮（用 TexturePacker 打开 .tps）+ 帧速率内联编辑（修改后自动重命名 02_done 文件夹并同步卡片动画速度）。

**Architecture:**
- Rust 后端新增 2 个命令：`open_file`（ShellExecuteW 用关联程序打开文件）、`rename_sequence_fps`（重命名 02_done/[an-XX-{old}]/ → [an-XX-{new}]/）
- 前端 TaskPage.vue 侧边栏：帧率行改为内联编辑模式；新增 [打开工程文件] 按钮（仅序列帧且已转换时显示）
- MaterialCard.vue：SequencePreview 加 `:key` 绑定 fps，使帧率修改后动画速度即时更新

**Tech Stack:** Rust (Tauri 2.x, Win32 ShellExecuteW), Vue 3 (ref, nextTick), TypeScript

---

## Task 1：Rust 后端 — `open_file` 命令

**目标**：用系统关联程序打开指定文件（.tps → TexturePacker）

**Files:**
- Modify: `src-tauri/src/commands.rs`（文件末尾 read_text_file 之后追加）
- Modify: `src-tauri/src/lib.rs`（invoke_handler 注册）

**Step 1：在 commands.rs 末尾（read_text_file 之后）追加命令**

```rust
/// 用系统关联程序打开指定文件（如 .tps → TexturePacker）
#[tauri::command]
pub fn open_file(path: String) -> Result<(), String> {
    let p = Path::new(&path);
    if !p.exists() {
        return Err(format!("文件不存在: {}", path));
    }

    #[cfg(target_os = "windows")]
    {
        use std::os::windows::ffi::OsStrExt;
        use windows::core::PCWSTR;
        use windows::Win32::UI::Shell::ShellExecuteW;
        use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

        let path_wide: Vec<u16> = p.as_os_str().encode_wide().chain(std::iter::once(0)).collect();
        let verb: Vec<u16> = "open\0".encode_utf16().collect();
        let result = unsafe {
            ShellExecuteW(
                None,
                PCWSTR(verb.as_ptr()),
                PCWSTR(path_wide.as_ptr()),
                None,
                None,
                SW_SHOWNORMAL,
            )
        };
        // ShellExecuteW 返回值 > 32 表示成功
        if result.0 <= 32 {
            return Err(format!("打开文件失败，错误码: {}", result.0));
        }
    }

    Ok(())
}
```

**Step 2：在 lib.rs invoke_handler 中注册**

在 `commands::read_text_file,` 这行之后加一行：
```rust
commands::open_file,
```

**Step 3：编译验证**

```bash
cd D:/work/pgsoft/PGB1
cargo build --manifest-path src-tauri/Cargo.toml 2>&1 | tail -5
```

预期：`Finished` 无错误。如果 windows crate 无 ShellExecuteW，改用 std::process：

```rust
// 备用方案（不依赖 windows crate 额外 feature）
#[cfg(target_os = "windows")]
{
    std::process::Command::new("cmd")
        .args(["/c", "start", "", &path])
        .spawn()
        .map_err(|e| format!("打开文件失败: {}", e))?;
}
```

---

## Task 2：Rust 后端 — `rename_sequence_fps` 命令

**目标**：修改序列帧帧率时，把 `02_done/[an-XX-{old_fps}]/` 重命名为 `02_done/[an-XX-{new_fps}]/`（一个素材可能有多个缩放版本，全部处理）

**Files:**
- Modify: `src-tauri/src/commands.rs`（open_file 之后追加）
- Modify: `src-tauri/src/lib.rs`

**Step 1：在 commands.rs 追加命令**

```rust
/// 修改序列帧的帧率：重命名 02_done/ 下所有 [an-XX-{old_fps}] 目录为 [an-XX-{new_fps}]
/// 其中目录名匹配规则：以 "[an-" 开头，以 "-{old_fps}]" 结尾，且目录内含有以 base_name 开头的文件
#[tauri::command]
pub fn rename_sequence_fps(
    task_path: String,
    base_name: String,
    old_fps: u32,
    new_fps: u32,
) -> Result<(), String> {
    if old_fps == new_fps {
        return Ok(());
    }

    let done_dir = Path::new(&task_path).join("02_done");
    if !done_dir.exists() {
        return Ok(());
    }

    let old_suffix = format!("-{}]", old_fps);
    let mut renamed = 0u32;

    let entries = fs::read_dir(&done_dir)
        .map_err(|e| format!("读取 02_done 失败: {}", e))?;

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let dir_name = match path.file_name().and_then(|n| n.to_str()) {
            Some(n) => n.to_string(),
            None => continue,
        };
        // 目录名必须符合 [an-XX-{old_fps}] 格式
        if !dir_name.starts_with("[an-") || !dir_name.ends_with(old_suffix.as_str()) {
            continue;
        }
        // 目录内必须含有以 base_name 开头的文件，确认是同一素材
        let has_match = fs::read_dir(&path)
            .map(|rd| rd.flatten().any(|e| {
                e.file_name()
                    .to_str()
                    .map(|n| n.starts_with(base_name.as_str()))
                    .unwrap_or(false)
            }))
            .unwrap_or(false);
        if !has_match {
            continue;
        }
        // 构造新目录名：替换末尾的 -{old_fps}] 为 -{new_fps}]
        let new_dir_name = format!(
            "{}{}]",
            &dir_name[..dir_name.len() - old_suffix.len()],
            format!("-{}", new_fps)
        );
        let new_path = done_dir.join(&new_dir_name);
        if new_path.exists() {
            return Err(format!("目标目录已存在: {}", new_dir_name));
        }
        fs::rename(&path, &new_path)
            .map_err(|e| format!("重命名 {} → {} 失败: {}", dir_name, new_dir_name, e))?;
        renamed += 1;
    }

    if renamed == 0 {
        // 没有找到匹配目录（可能素材尚未转换），静默成功
    }

    Ok(())
}
```

**Step 2：在 lib.rs 注册**

在 `commands::open_file,` 之后加：
```rust
commands::rename_sequence_fps,
```

**Step 3：编译验证**

```bash
cargo build --manifest-path src-tauri/Cargo.toml 2>&1 | tail -5
```

预期：`Finished` 无错误。

---

## Task 3：前端 — 帧率内联编辑（TaskPage.vue）

**目标**：侧边栏帧率行从只读改为可点击编辑；确认后调用 `rename_sequence_fps` 并 `refresh()`

**Files:**
- Modify: `src/views/TaskPage.vue`

### Step 1：在 script 区追加响应式状态和函数

在 `closeSidebarDialog` 函数（约第 309 行）之后插入：

```typescript
/** 帧率内联编辑 */
const editingFps = ref(false)
const fpsInput = ref('')

function startEditFps() {
  const mat = selectedMaterial.value
  if (!mat || mat.fps == null) return
  fpsInput.value = String(mat.fps)
  editingFps.value = true
  nextTick(() => {
    (document.querySelector('.fps-input') as HTMLInputElement)?.select()
  })
}

function cancelEditFps() {
  editingFps.value = false
  fpsInput.value = ''
}

async function confirmEditFps() {
  const mat = selectedMaterial.value
  if (!mat) return
  const newFps = parseInt(fpsInput.value, 10)
  if (!newFps || newFps <= 0 || newFps === mat.fps) {
    cancelEditFps()
    return
  }
  try {
    await invoke('rename_sequence_fps', {
      taskPath: taskFolderPath,
      baseName: mat.name,
      oldFps: mat.fps,
      newFps,
    })
    cancelEditFps()
    await refresh()
    // refresh 后 selectedMaterial 会被重置，需要重新选中
    const updated = materials.value.find(m => m.name === mat.name)
    if (updated) {
      selectedMaterial.value = updated
      loadVersions(updated)
    }
  } catch (e) {
    console.error('修改帧率失败:', e)
  }
}

/** 打开工程文件（.tps）*/
async function openTpsFile() {
  const mat = selectedMaterial.value
  if (!mat) return
  // 在 versions 中找 02_done 阶段的 folder_path，拼 .tps 路径
  const doneVersion = versions.value.find(v => v.stage === '02_done')
  if (!doneVersion) return
  const tpsPath = doneVersion.folder_path.replace(/\\/g, '/') + '/' + mat.name + '.tps'
  try {
    await invoke('open_file', { path: tpsPath })
  } catch (e) {
    console.error('打开工程文件失败:', e)
  }
}
```

> **注意**：`loadVersions` 是已有函数（加载 versions 列表），`taskFolderPath` 是已有变量。

### Step 2：改模板 — 帧率行改为内联编辑

将原来的帧率 `info-row`（约第 784-787 行）：
```html
<div v-if="selectedMaterial.material_type === 'sequence'" class="info-row">
  <span class="info-label">帧率</span>
  <span class="info-value">{{ selectedMaterial.fps != null ? selectedMaterial.fps + ' fps' : '未转换' }}</span>
</div>
```

替换为：
```html
<div v-if="selectedMaterial.material_type === 'sequence'" class="info-row fps-row">
  <span class="info-label">帧率</span>
  <template v-if="selectedMaterial.fps != null">
    <template v-if="!editingFps">
      <span class="info-value fps-clickable" @click="startEditFps" title="点击修改帧率">
        {{ selectedMaterial.fps }} fps
      </span>
    </template>
    <template v-else>
      <input
        class="fps-input"
        type="number"
        min="1"
        max="120"
        v-model="fpsInput"
        @keydown.enter="confirmEditFps"
        @keydown.escape="cancelEditFps"
        @blur="cancelEditFps"
      />
      <span class="fps-unit">fps</span>
    </template>
  </template>
  <span v-else class="info-value">未转换</span>
</div>
```

### Step 3：改模板 — 底部按钮区追加 [打开工程文件]

原来的底部按钮区（约第 831-835 行）：
```html
<!-- 底部悬浮操作按钮 -->
<div class="sidebar-actions">
  <button class="sidebar-action-btn" @click="openRenameDialog">重命名</button>
  <button class="sidebar-action-btn danger" @click="openDeleteDialog">删除</button>
</div>
```

替换为：
```html
<!-- 底部悬浮操作按钮 -->
<div class="sidebar-actions">
  <button
    v-if="selectedMaterial?.material_type === 'sequence' && versions.some(v => v.stage === '02_done')"
    class="sidebar-action-btn"
    @click="openTpsFile"
  >打开工程文件</button>
  <button class="sidebar-action-btn" @click="openRenameDialog">重命名</button>
  <button class="sidebar-action-btn danger" @click="openDeleteDialog">删除</button>
</div>
```

### Step 4：追加 CSS（非 scoped 样式块）

在 TaskPage.vue 的非 scoped `<style>` 块中追加：

```css
/* 帧率内联编辑 */
.fps-clickable {
  cursor: pointer;
  border-bottom: 1px dashed var(--text-tertiary);
  transition: color var(--transition-fast);
}
.fps-clickable:hover {
  color: var(--accent-primary);
  border-bottom-color: var(--accent-primary);
}
.fps-row {
  align-items: center;
  gap: var(--space-2);
}
.fps-input {
  width: 48px;
  padding: 1px var(--space-1);
  background: var(--bg-secondary);
  border: 1px solid var(--border-default);
  border-radius: var(--radius-sm);
  color: var(--text-primary);
  font-size: var(--font-size-sm);
  font-family: inherit;
  text-align: right;
}
.fps-input:focus {
  outline: none;
  border-color: var(--accent-primary);
}
/* 隐藏 number input 的上下箭头 */
.fps-input::-webkit-outer-spin-button,
.fps-input::-webkit-inner-spin-button {
  -webkit-appearance: none;
}
.fps-unit {
  font-size: var(--font-size-sm);
  color: var(--text-secondary);
}
```

---

## Task 4：前端 — MaterialCard.vue SequencePreview 加 `:key`

**目标**：fps 修改后，材质卡片上的动画速度即时更新（强制重新挂载 SequencePreview）

**Files:**
- Modify: `src/components/MaterialCard.vue`

找到（约第 56-61 行）：
```html
<SequencePreview
  v-if="material.material_type === 'sequence'"
  :folder-path="material.path"
  :fps="material.fps ?? 24"
  :max-width="200"
/>
```

替换为：
```html
<SequencePreview
  v-if="material.material_type === 'sequence'"
  :key="`${material.path}-${material.fps ?? 24}`"
  :folder-path="material.path"
  :fps="material.fps ?? 24"
  :max-width="200"
/>
```

> `:key` 包含 fps，fps 变化时 Vue 会销毁旧实例、挂载新实例，`frameInterval` 从新 fps 重新计算。

---

## Task 5：验证 MaterialVersion.stage 字段

`openTpsFile` 和 [打开工程文件] 的 v-if 都用了 `v.stage === 'done'`，需要确认 `MaterialVersion` 接口里 `stage` 字段的值。

**Files:**
- Read: `src/composables/useMaterials.ts` 或 `src-tauri/src/models.rs`

查看 `MaterialVersion` 结构，确认 stage 字段含义：

```bash
grep -n "stage\|MaterialVersion" src-tauri/src/models.rs
grep -n "MaterialVersion\|stage" src/composables/useMaterials.ts 2>/dev/null || true
```

如果 stage 不是字符串 `"done"` 而是枚举序列化值，调整前端判断逻辑：
- Rust `MaterialProgress::Done` 序列化为 `"done"` ✅（当前设计）
- 若 `stage_label` 里是中文，改用 `v.stage_label.includes('完成')` 作为回退

---

## Task 6：集成测试

```bash
cd D:/work/pgsoft/PGB1 && npm run tauri dev
```

**测试步骤**：
1. 打开有序列帧素材的任务页（`D:\work\pgsoft\exp\217_RedDevil\`）
2. 点击一个已转换的序列帧卡片（fps 不为 null 的）
3. **帧率行测试**：点击帧率值 → 出现输入框 → 输入新帧率（如 24→30）→ 回车 → 侧边栏帧率刷新
4. **卡片动画速度**：观察对应卡片的动画播放速度是否变快/慢
5. **打开工程文件**：底部出现 [打开工程文件] 按钮 → 点击 → TexturePacker 启动并打开对应 .tps
6. **未转换素材**：帧率行显示"未转换"，无编辑交互，无 [打开工程文件] 按钮

---

## 关键注意事项

1. **`open_file` 备用方案**：若 windows crate 无 `ShellExecuteW`，用 `cmd /c start "" <path>` 替代，无需新增 Cargo.toml 依赖
2. **`rename_sequence_fps` 静默成功**：未转换的素材没有 `02_done/[an-*]` 目录，不应报错
3. **帧率编辑 `@blur`**：失焦时取消编辑，防止用户点击其他地方后 input 残留
4. **`confirmEditFps` 中的 `loadVersions`**：`refresh()` 会重置 `selectedMaterial`，所以需要在 refresh 后手动重新赋值 + 重新加载 versions
5. **`versions.some(v => v.stage === '02_done')`**：已确认，Rust `stage` 字段值为目录名字符串
