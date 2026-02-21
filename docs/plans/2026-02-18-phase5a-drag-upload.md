# Phase 5a: 文件拖拽上传 实施计划

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 实现多选模式 + 拖拽文件到外部浏览器 + 上传确认 + 自动复制到 nextcloud 的完整上传工作流。

**Architecture:** TaskPage 新增多选状态管理，MaterialCard 支持多选模式下的复选框和选中视觉。使用 `@crabnebula/tauri-plugin-drag` 插件实现拖拽文件到外部应用（这是 Tauri 选型的核心理由）。Rust 后端新增命令：根据素材列表收集 02_done 中的实际文件路径（用于拖拽）、复制文件到 nextcloud（用于确认上传后）。

**Tech Stack:** Tauri 2.x, `tauri-plugin-drag`（Rust crate + JS binding），Vue 3 Composition API

---

## 功能流程概览

```
用户点击 [多选] 按钮
  → 小标题栏变化：[多选✓] [全选] 出现
  → 卡片左上角出现复选框
  → 点击卡片 = 切换选中状态（不再打开侧边栏）
  → 关闭侧边栏（如果打开）

用户选中若干素材 → 拖拽卡片
  → 程序通过 tauri-plugin-drag 的 startDrag 发起 OS 级拖拽
  → 拖出的文件 = 素材最新版本（根据进度自动选择最终产物）
  → 用户拖到浏览器网盘页面上传

用户回到应用 → 程序弹出确认弹窗
  → "检测到您拖拽了 N 个文件，是否已成功上传到网盘？"
  → 用户点击 [是，已上传]
  → 程序自动复制 02_done 文件到 nextcloud/（排除 .tps）
  → 刷新素材列表，进度状态变为"已上传"

用户点击 [多选✓] → 退出多选模式，清除所有选择
```

---

## Task 1: 安装 tauri-plugin-drag 依赖

**Files:**
- Modify: `src-tauri/Cargo.toml`
- Modify: `package.json`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/capabilities/default.json`

**Step 1: 添加 Rust 依赖**

在 `src-tauri/Cargo.toml` 的 `[dependencies]` 中添加：
```toml
tauri-plugin-drag = "0.6"
```

**Step 2: 注册 Tauri 插件**

在 `src-tauri/src/lib.rs` 的 `tauri::Builder` 链中添加：
```rust
.plugin(tauri_plugin_drag::init())
```

**Step 3: 添加权限**

在 `src-tauri/capabilities/default.json` 的 `permissions` 数组中添加：
```json
"drag:default"
```

**Step 4: 添加 JS 依赖**

```bash
npm install @crabnebula/tauri-plugin-drag
```

**Step 5: 验证编译通过**

```bash
cd src-tauri && cargo check
```

---

## Task 2: Rust 后端 — 收集素材拖拽文件路径

**Files:**
- Modify: `src-tauri/src/commands.rs`
- Modify: `src-tauri/src/lib.rs`

**功能说明：** 给定一组素材信息（name + material_type），返回每个素材在文件系统中的最终产物路径（用于拖拽到外部）。选择策略：按进度链条从高到低找最新版本。

**Step 1: 在 commands.rs 中添加新命令**

```rust
/// 收集素材的拖拽文件列表
/// 根据素材进度，选择最终产物路径（02_done > 01_scale > 00_original）
#[tauri::command]
pub fn collect_drag_files(
    task_path: String,
    materials: Vec<DragMaterialRequest>,
) -> Result<Vec<String>, String> {
    let task_dir = Path::new(&task_path);
    let done_dir = task_dir.join("02_done");
    let scale_dir = task_dir.join("01_scale");
    let original_dir = task_dir.join("00_original");

    let mut file_paths = Vec::new();

    for mat in &materials {
        let paths = find_best_files_for_material(
            &mat.name,
            &mat.material_type,
            &original_dir,
            &scale_dir,
            &done_dir,
        );
        file_paths.extend(paths);
    }

    Ok(file_paths)
}
```

**Step 2: 在 models.rs 中添加请求结构体**

```rust
/// 拖拽请求中的素材信息
#[derive(Debug, Deserialize, Clone)]
pub struct DragMaterialRequest {
    pub name: String,
    pub material_type: String,
}
```

**Step 3: 实现 find_best_files_for_material 辅助函数**

```rust
/// 查找素材的最佳文件路径（优先 02_done，回退到 01_scale，最后 00_original）
fn find_best_files_for_material(
    base_name: &str,
    material_type: &str,
    original_dir: &Path,
    scale_dir: &Path,
    done_dir: &Path,
) -> Vec<String> {
    // 对于静帧：优先找 02_done/[img-*]/ 中的 .webp
    // 对于序列帧：优先找 02_done/[an-*]/ 中的精灵图三件套（.webp + .plist，排除 .tps）
    // 回退：01_scale 中的文件 → 00_original 中的文件

    let mut results = Vec::new();

    if material_type == "image" {
        // 在 02_done 中查找
        if done_dir.exists() {
            if let Some(path) = find_image_in_done(done_dir, base_name) {
                results.push(path);
                return results;
            }
        }
        // 在 01_scale 中查找
        if scale_dir.exists() {
            if let Some(path) = find_image_in_scale(scale_dir, base_name) {
                results.push(path);
                return results;
            }
        }
        // 在 00_original 中查找
        if let Some(path) = find_file_by_base_name(original_dir, base_name) {
            results.push(path);
        }
    } else if material_type == "sequence" {
        // 在 02_done 中查找精灵图三件套（排除 .tps）
        if done_dir.exists() {
            let sprite_files = find_sequence_in_done(done_dir, base_name);
            if !sprite_files.is_empty() {
                results.extend(sprite_files);
                return results;
            }
        }
        // 回退到 00_original 中的序列帧文件夹（所有帧文件）
        let seq_dir = original_dir.join(base_name);
        if seq_dir.is_dir() {
            if let Ok(entries) = fs::read_dir(&seq_dir) {
                for entry in entries.flatten() {
                    let p = entry.path();
                    if p.is_file() {
                        results.push(p.to_string_lossy().to_string());
                    }
                }
            }
        }
    } else {
        // video / other：直接用 00_original 中的文件
        if let Some(path) = find_file_by_base_name(original_dir, base_name) {
            results.push(path);
        }
    }

    results
}
```

> **注意**：辅助函数 `find_image_in_done`、`find_image_in_scale`、`find_sequence_in_done`、`find_file_by_base_name` 需要遍历对应目录的子文件夹，按文件名（base_name）匹配。实现时参考已有的 `determine_progress_image` 和 `determine_progress_sequence` 中的匹配逻辑。

**Step 4: 注册命令**

在 `lib.rs` 的 `invoke_handler` 中添加 `commands::collect_drag_files`。

---

## Task 3: Rust 后端 — 复制文件到 nextcloud

**Files:**
- Modify: `src-tauri/src/commands.rs`
- Modify: `src-tauri/src/lib.rs`

**功能说明：** 用户确认上传后，程序将选中素材从 02_done 复制到 nextcloud/{任务名}/ 目录。普通任务扁平化复制，Prototype 保留子分类 + 额外复制 `_original`。

**Step 1: 添加复制到 nextcloud 命令**

```rust
/// 将选中素材从 02_done 复制到 nextcloud/
/// Prototype 特例：保留子分类结构 + 额外复制 01_scale 到 _original/
#[tauri::command]
pub fn copy_to_nextcloud(
    task_path: String,
    material_names: Vec<CopyMaterialRequest>,
) -> Result<CopyResult, String> {
    let task_dir = Path::new(&task_path);
    let task_name = task_dir
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown");

    let done_dir = task_dir.join("02_done");
    let scale_dir = task_dir.join("01_scale");

    // 推导 nextcloud 路径
    let nextcloud_dir = task_dir
        .parent() // Export/
        .and_then(|p| p.parent()) // VFX/
        .map(|vfx| vfx.join("nextcloud").join(task_name))
        .ok_or_else(|| "无法推导 nextcloud 路径".to_string())?;

    // 确保 nextcloud 目录存在
    fs::create_dir_all(&nextcloud_dir)
        .map_err(|e| format!("创建 nextcloud 目录失败: {}", e))?;

    let is_prototype = task_name.to_lowercase() == "prototype";
    let mut copied_count = 0u32;
    let mut errors = Vec::new();

    for mat in &material_names {
        let result = if is_prototype {
            copy_material_prototype(
                &mat.name,
                &mat.material_type,
                &done_dir,
                &scale_dir,
                &nextcloud_dir,
            )
        } else {
            copy_material_normal(
                &mat.name,
                &mat.material_type,
                &done_dir,
                &nextcloud_dir,
            )
        };

        match result {
            Ok(count) => copied_count += count,
            Err(e) => errors.push(format!("{}: {}", mat.name, e)),
        }
    }

    Ok(CopyResult {
        copied_count,
        errors,
    })
}
```

**Step 2: 在 models.rs 中添加请求/响应结构体**

```rust
#[derive(Debug, Deserialize, Clone)]
pub struct CopyMaterialRequest {
    pub name: String,
    pub material_type: String,
}

#[derive(Debug, Serialize, Clone)]
pub struct CopyResult {
    pub copied_count: u32,
    pub errors: Vec<String>,
}
```

**Step 3: 实现普通任务复制函数**

```rust
/// 普通任务：从 02_done/[img-*]/ 或 [an-*]/ 扁平化复制到 nextcloud/{task}/
/// 排除 .tps 文件
fn copy_material_normal(
    base_name: &str,
    material_type: &str,
    done_dir: &Path,
    nextcloud_dir: &Path,
) -> Result<u32, String> {
    // 遍历 done_dir 的子目录（[img-XX]/ 或 [an-XX-YY]/）
    // 查找 base_name 匹配的文件
    // 复制到 nextcloud_dir 根目录（扁平化）
    // 排除 .tps 文件
    // 返回复制的文件数
    ...
}
```

**Step 4: 实现 Prototype 复制函数**

```rust
/// Prototype：保留子分类结构
/// 1. 从 02_done/[*]/{子分类}/ 复制到 nextcloud/Prototype/{子分类}/
/// 2. 额外从 01_scale/[XX]/{子分类}/ 复制到 nextcloud/Prototype/{子分类}/_original/
fn copy_material_prototype(
    base_name: &str,  // 格式: "subcategory/basename"
    material_type: &str,
    done_dir: &Path,
    scale_dir: &Path,
    nextcloud_dir: &Path,
) -> Result<u32, String> {
    // 解析 sub_category 和 actual_base_name
    // 创建 nextcloud/{sub_category}/ 和 nextcloud/{sub_category}/_original/
    // 从 02_done 复制处理后的文件
    // 从 01_scale 复制原始文件到 _original/
    // 排除 .tps 文件
    ...
}
```

**Step 5: 注册命令**

在 `lib.rs` 的 `invoke_handler` 中添加 `commands::copy_to_nextcloud`。

---

## Task 4: 前端 — 多选状态管理

**Files:**
- Modify: `src/views/TaskPage.vue`

**功能说明：** TaskPage 新增多选模式的状态管理逻辑。

**Step 1: 添加多选状态变量**

在 `<script setup>` 中添加：
```typescript
/** 是否处于多选模式 */
const isMultiSelect = ref(false)

/** 当前选中的素材路径集合 */
const selectedPaths = ref<Set<string>>(new Set())

/** 进入多选模式 */
function enterMultiSelect() {
  isMultiSelect.value = true
  // 关闭侧边栏
  if (selectedMaterial.value) {
    closeSidebar()
  }
}

/** 退出多选模式 */
function exitMultiSelect() {
  isMultiSelect.value = false
  selectedPaths.value = new Set()
}

/** 切换多选模式 */
function toggleMultiSelect() {
  if (isMultiSelect.value) {
    exitMultiSelect()
  } else {
    enterMultiSelect()
  }
}

/** 切换单个素材的选中状态 */
function toggleMaterialSelection(material: MaterialInfo) {
  const paths = new Set(selectedPaths.value)
  if (paths.has(material.path)) {
    paths.delete(material.path)
  } else {
    paths.add(material.path)
  }
  selectedPaths.value = paths
}

/** 全选 / 取消全选 */
const isAllSelected = computed(() => {
  return materials.value.length > 0 && selectedPaths.value.size === materials.value.length
})

function toggleSelectAll() {
  if (isAllSelected.value) {
    selectedPaths.value = new Set()
  } else {
    selectedPaths.value = new Set(materials.value.map(m => m.path))
  }
}

/** 选中的素材列表 */
const selectedMaterials = computed(() => {
  return materials.value.filter(m => selectedPaths.value.has(m.path))
})
```

**Step 2: 修改卡片点击逻辑**

修改 `selectMaterial` 的调用处和 MaterialCard 的 `@click`，在多选模式下改为切换选中：

```typescript
function onCardClick(material: MaterialInfo) {
  if (isMultiSelect.value) {
    toggleMaterialSelection(material)
  } else {
    selectMaterial(material)
  }
}
```

---

## Task 5: 前端 — 小标题栏多选按钮

**Files:**
- Modify: `src/views/TaskPage.vue`

**功能说明：** 小标题栏新增 [多选] / [多选✓] 按钮和 [全选] / [取消全选] 按钮。

**Step 1: 修改 sub-title-bar 模板**

```html
<div class="sub-title-bar">
  <span class="sub-title">素材列表</span>
  <div class="view-buttons">
    <button
      class="view-btn"
      :class="{ active: viewMode === 'tree' }"
      @click="viewMode = 'tree'"
    >
      树形视图
    </button>
    <button
      class="view-btn"
      :class="{ active: viewMode === 'name' }"
      @click="viewMode = 'name'"
    >
      名称视图
    </button>
    <button class="view-btn" @click="refresh">
      刷新
    </button>
    <!-- 多选按钮 -->
    <button
      class="view-btn"
      :class="{ active: isMultiSelect }"
      @click="toggleMultiSelect"
    >
      {{ isMultiSelect ? '多选 ✓' : '多选' }}
    </button>
    <!-- 全选按钮（仅多选模式下显示） -->
    <button
      v-if="isMultiSelect"
      class="view-btn"
      @click="toggleSelectAll"
    >
      {{ isAllSelected ? '取消全选' : '全选' }}
    </button>
  </div>
</div>
```

---

## Task 6: 前端 — MaterialCard 多选模式支持

**Files:**
- Modify: `src/components/MaterialCard.vue`

**功能说明：** 素材卡片支持多选模式：显示复选框、选中状态视觉反馈。

**Step 1: 添加 Props**

```typescript
defineProps<{
  material: MaterialInfo
  multiSelect?: boolean
  checked?: boolean
}>()
```

**Step 2: 添加复选框模板**

在 `preview-wrapper` 内部最前面添加：
```html
<!-- 多选复选框 -->
<span v-if="multiSelect" class="card-checkbox" :class="{ checked }">
  <svg v-if="checked" width="14" height="14" viewBox="0 0 24 24" fill="currentColor">
    <path d="M9 16.17L4.83 12l-1.42 1.41L9 19 21 7l-1.41-1.41z" />
  </svg>
</span>
```

**Step 3: 添加选中边框样式**

```css
.material-card.multi-checked {
  border: 2px solid var(--color-primary);
}

.card-checkbox {
  position: absolute;
  top: var(--spacing-2);
  left: var(--spacing-2);
  width: 24px;
  height: 24px;
  border-radius: var(--radius-sm);
  border: 2px solid var(--border-medium);
  background: var(--glass-subtle-bg);
  backdrop-filter: blur(8px);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 2;
  transition: all var(--transition-fast);
}

.card-checkbox.checked {
  background: var(--color-primary);
  border-color: var(--color-primary);
  color: var(--color-neutral-0);
}
```

**Step 4: 在 TaskPage 中传递 Props**

```html
<MaterialCard
  v-for="m in group.items"
  :key="m.path"
  :material="m"
  :multi-select="isMultiSelect"
  :checked="selectedPaths.has(m.path)"
  :class="{
    selected: !isMultiSelect && selectedMaterial?.path === m.path,
    'multi-checked': isMultiSelect && selectedPaths.has(m.path),
  }"
  @click="onCardClick(m)"
/>
```

---

## Task 7: 前端 — 拖拽到外部浏览器

**Files:**
- Modify: `src/views/TaskPage.vue`

**功能说明：** 在多选模式下，用户拖拽选中的卡片时，通过 tauri-plugin-drag 发起 OS 级文件拖拽。

**Step 1: 添加拖拽处理函数**

```typescript
import { startDrag } from '@crabnebula/tauri-plugin-drag'

/** 记录是否发生了拖拽（用于后续弹出确认弹窗） */
const hasDraggedOut = ref(false)

async function onCardDragStart(e: DragEvent, material: MaterialInfo) {
  // 非多选模式下也支持单个拖拽
  if (!isMultiSelect.value) {
    // 单卡片拖拽：临时将当前卡片作为选中
    await performDrag([material])
    return
  }

  // 如果拖拽的卡片未被选中，先选中它
  if (!selectedPaths.value.has(material.path)) {
    toggleMaterialSelection(material)
  }

  // 多选拖拽
  e.preventDefault()
  await performDrag(selectedMaterials.value)
}

async function performDrag(materialsToD drag: MaterialInfo[]) {
  try {
    // 调用 Rust 后端收集实际文件路径
    const filePaths = await invoke<string[]>('collect_drag_files', {
      taskPath: taskFolderPath,
      materials: materialsToD drag.map(m => ({
        name: m.name,
        material_type: m.material_type,
      })),
    })

    if (filePaths.length === 0) {
      console.warn('没有可拖拽的文件')
      return
    }

    // 调用 tauri-plugin-drag 发起 OS 级拖拽
    await startDrag({ item: filePaths, icon: '' })

    // 拖拽完成后，标记已拖拽（用于弹出确认弹窗）
    hasDraggedOut.value = true
    draggedMaterials.value = materialsToD drag
  } catch (err) {
    console.error('拖拽失败:', err)
  }
}
```

**Step 2: 在卡片上绑定 dragstart**

在 MaterialCard 上监听原生 `mousedown` 事件，在多选模式下拦截并发起 startDrag：

```html
<MaterialCard
  ...
  @mousedown="(e: MouseEvent) => { if (isMultiSelect || !isMultiSelect) onCardMouseDown(e, m) }"
/>
```

> **实现细节**：由于 `startDrag` 是 OS 级别的拖拽 API（不走 HTML5 drag），需要在 `mousedown` 时直接调用。具体实现需要根据 `tauri-plugin-drag` 的 API 行为调整——它会自动接管鼠标事件并发起系统拖拽。

---

## Task 8: 前端 — 上传确认弹窗

**Files:**
- Create: `src/components/UploadConfirmDialog.vue`
- Modify: `src/views/TaskPage.vue`

**功能说明：** 拖拽完成后弹出确认弹窗，用户确认已上传则触发 nextcloud 复制。

**Step 1: 创建确认弹窗组件**

```vue
<script setup lang="ts">
defineProps<{
  fileCount: number
}>()

defineEmits<{
  confirm: []
  cancel: []
}>()
</script>

<template>
  <Teleport to="body">
    <div class="dialog-overlay" @click.self="$emit('cancel')">
      <div class="dialog-content glass-strong">
        <p class="dialog-title">上传确认</p>
        <div class="dialog-body">
          <p>检测到您拖拽了 <strong>{{ fileCount }}</strong> 个文件</p>
          <p class="dialog-hint">是否已成功上传到网盘？</p>
        </div>
        <div class="dialog-actions">
          <button class="dialog-btn dialog-btn-primary" @click="$emit('confirm')">
            是，已上传
          </button>
          <button class="dialog-btn dialog-btn-secondary" @click="$emit('cancel')">
            取消
          </button>
        </div>
      </div>
    </div>
  </Teleport>
</template>
```

样式使用 design-system tokens，毛玻璃背景 + 居中弹窗。

**Step 2: 在 TaskPage 中集成弹窗**

```typescript
const showUploadConfirm = ref(false)
const draggedMaterials = ref<MaterialInfo[]>([])

// 拖拽完成后显示确认弹窗
watch(hasDraggedOut, (val) => {
  if (val) {
    showUploadConfirm.value = true
    hasDraggedOut.value = false
  }
})

async function confirmUpload() {
  showUploadConfirm.value = false

  try {
    const result = await invoke<{ copied_count: number; errors: string[] }>('copy_to_nextcloud', {
      taskPath: taskFolderPath,
      materialNames: draggedMaterials.value.map(m => ({
        name: m.name,
        material_type: m.material_type,
      })),
    })

    // 刷新素材列表
    await refresh()

    // 可选：退出多选模式
    exitMultiSelect()

    if (result.errors.length > 0) {
      console.warn('部分文件复制失败:', result.errors)
    }
  } catch (err) {
    console.error('复制到 nextcloud 失败:', err)
  }
}

function cancelUpload() {
  showUploadConfirm.value = false
  draggedMaterials.value = []
}
```

```html
<UploadConfirmDialog
  v-if="showUploadConfirm"
  :file-count="draggedMaterials.length"
  @confirm="confirmUpload"
  @cancel="cancelUpload"
/>
```

---

## Task 9: 集成测试与边界处理

**Files:**
- Modify: `src/views/TaskPage.vue`

**边界情况处理：**

1. **空选择拖拽**：选中列表为空时不触发拖拽
2. **进度为 "none" 的素材**：不参与拖拽（没有可用文件），UI 上显示提示
3. **nextcloud 目录不存在**：Rust 命令自动创建
4. **文件冲突**：nextcloud 中已存在同名文件时直接覆盖（设计文档确认）
5. **Prototype 特例**：素材名格式为 `subcategory/basename`，复制时保留子分类结构
6. **侧边栏互斥**：进入多选模式时关闭侧边栏，多选模式下不显示侧边栏
7. **页面切换**：切换视图（树形/名称）时保持多选状态和选中列表
8. **刷新**：刷新后清除选中列表（因为素材列表可能变化）

---

## 执行顺序建议

```
Task 1 (依赖安装) → Task 2 + 3 (Rust 后端，可并行)
                  → Task 4 + 5 + 6 (前端多选 UI，顺序执行)
                  → Task 7 (拖拽集成，依赖 Task 1-3)
                  → Task 8 (确认弹窗，依赖 Task 7)
                  → Task 9 (边界处理，最后)
```

---

## 测试验证清单

- [ ] 点击 [多选] 进入多选模式，卡片显示复选框
- [ ] 点击卡片切换选中状态，视觉反馈正确
- [ ] [全选] / [取消全选] 功能正常
- [ ] 再次点击 [多选✓] 退出多选模式，选中状态清除
- [ ] 多选模式下拖拽卡片，OS 级拖拽启动
- [ ] 拖拽到外部浏览器后回到应用，确认弹窗出现
- [ ] 确认上传后，文件复制到 nextcloud，素材进度变为"已上传"
- [ ] 取消上传，不执行任何操作
- [ ] Prototype 素材的 nextcloud 复制保留子分类结构
- [ ] 进入多选模式时侧边栏自动关闭
- [ ] 空选择时拖拽无反应
