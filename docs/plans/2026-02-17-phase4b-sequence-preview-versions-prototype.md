# Phase 4b: 序列帧动画预览 + 侧边栏其他版本 + Prototype 子分类

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 素材卡片支持序列帧 Canvas 动画预览（悬停播放）、侧边栏显示素材在各工作流目录的版本列表、Prototype 任务按 7 个子分类展示素材。

**Scope 界定:**
- **本 Phase 做的**：Canvas 序列帧动画、LRU 帧缓存、侧边栏"其他版本"区域（含 Rust 命令）、Prototype 子分类扫描与展示
- **不做**：侧边栏图片缩放拖拽交互、视频播放器、文档预览、IndexedDB 持久化缓存

**Architecture:**
- 新增 `SequencePreview` Vue 组件，用 Canvas + requestAnimationFrame 播放序列帧
- 新增 `useFrameCache` composable 管理 LRU 帧缓存（最多 10 个序列，120 帧上限）
- Rust 后端新增 `scan_material_versions` 命令，扫描素材在各工作流目录的版本
- Rust 后端修改 `scan_materials` 支持 Prototype 子分类（多一层目录）
- 前端 TaskPage 识别 Prototype 任务，按子分类分组展示

**前置条件:** Phase 4a 已完成。

**测试数据:** `D:\work\pgsoft\exp\217_RedDevil\03_Render_VFX\VFX\Export\Ambient\`（普通任务）和 `...\Export\Prototype\`（Prototype 任务）

**设计文档参考:**
- 序列帧预览: `design/卡片设计.md` L240-319
- 侧边栏其他版本: `design/界面设计.md` L1701-1724
- Prototype: `design/Prototype特例规则.md`
- 素材进度: `design/文件命名与组织规则.md` L1460-1690

---

## 总体路线图

| 阶段 | 内容 | 状态 |
|------|------|------|
| **Phase 1** | 脚手架 + 设计系统 + 主窗口框架 | ✅ |
| **Phase 2** | 三级页面导航 + Vue Router + 页面骨架 | ✅ |
| **Phase 3a** | 项目卡片 + 任务卡片 + Rust 文件系统后端 | ✅ |
| **Phase 3b** | 普通卡片 + 辅助页面文件扫描 + 打开文件夹 | ✅ |
| **Phase 4a** | 素材卡片 + 任务页双视图 + 侧边栏基础 | ✅ |
| **Phase 4b**（本文档） | 序列帧动画预览 + 版本追踪 + Prototype 完整处理 | ✅ |
| **Phase 5** | 文件拖拽 + 任务管理系统 + 工作流功能 | ⬜ 下一步 |
| **Phase 6** | 日报打卡 + 翻译 + 转换进度悬浮窗 + 设置页 | ⬜ |

---

## Task 1: 序列帧 Canvas 预览组件

**Files:**
- Create: `src/components/SequencePreview.vue`
- Create: `src/composables/useFrameCache.ts`

**Step 1: 创建 LRU 帧缓存 composable**

`src/composables/useFrameCache.ts` — 全局单例，管理序列帧图片缓存。

```typescript
import { convertFileSrc } from '@tauri-apps/api/core'

interface CachedSequence {
  key: string
  frames: HTMLImageElement[]
  lastUsed: number
}

const MAX_CACHED = 10
const MAX_FRAMES = 120
const cache: CachedSequence[] = []

/** 加载序列帧图片（带 LRU 缓存） */
export async function loadSequenceFrames(
  folderPath: string,
  framePaths: string[],
  maxWidth: number,
): Promise<HTMLImageElement[]> {
  const key = `${folderPath}:${maxWidth}`

  // 命中缓存
  const existing = cache.find(c => c.key === key)
  if (existing) {
    existing.lastUsed = Date.now()
    return existing.frames
  }

  // 降采样：超过 MAX_FRAMES 则均匀取样
  let paths = framePaths
  if (paths.length > MAX_FRAMES) {
    const step = paths.length / MAX_FRAMES
    paths = Array.from({ length: MAX_FRAMES }, (_, i) => framePaths[Math.floor(i * step)])
  }

  // 并行加载图片
  const frames = await Promise.all(
    paths.map(p => loadImage(convertFileSrc(p)))
  )

  // LRU 淘汰
  if (cache.length >= MAX_CACHED) {
    cache.sort((a, b) => a.lastUsed - b.lastUsed)
    cache.shift()
  }

  cache.push({ key, frames, lastUsed: Date.now() })
  return frames
}

function loadImage(src: string): Promise<HTMLImageElement> {
  return new Promise((resolve, reject) => {
    const img = new Image()
    img.onload = () => resolve(img)
    img.onerror = reject
    img.src = src
  })
}
```

**Step 2: 创建 SequencePreview 组件**

`src/components/SequencePreview.vue` — Canvas 序列帧播放器，悬停时播放，离开时暂停回到首帧。

```vue
<script setup lang="ts">
import { ref, onMounted, onUnmounted, watch } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { loadSequenceFrames } from '../composables/useFrameCache'

const props = defineProps<{
  folderPath: string
  fps?: number
  maxWidth?: number
}>()

const canvasRef = ref<HTMLCanvasElement | null>(null)
const loaded = ref(false)

let frames: HTMLImageElement[] = []
let currentFrame = 0
let animationId: number | null = null
let lastFrameTime = 0
const frameInterval = 1000 / (props.fps ?? 24)

async function init() {
  // 获取序列帧文件列表
  const filePaths = await invoke<string[]>('list_sequence_frames', { dirPath: props.folderPath })
  if (filePaths.length === 0) return

  frames = await loadSequenceFrames(props.folderPath, filePaths, props.maxWidth ?? 200)
  loaded.value = true
  drawFrame(0)
}

function drawFrame(index: number) {
  const canvas = canvasRef.value
  if (!canvas || frames.length === 0) return

  const ctx = canvas.getContext('2d')
  if (!ctx) return

  const img = frames[index]
  canvas.width = canvas.clientWidth
  canvas.height = canvas.clientHeight

  // 黑色背景
  ctx.fillStyle = '#000000'
  ctx.fillRect(0, 0, canvas.width, canvas.height)

  // 居中绘制，保持比例
  const scale = Math.min(canvas.width / img.width, canvas.height / img.height)
  const w = img.width * scale
  const h = img.height * scale
  const x = (canvas.width - w) / 2
  const y = (canvas.height - h) / 2
  ctx.drawImage(img, x, y, w, h)
}

function play() {
  if (frames.length === 0) return

  function tick(timestamp: number) {
    if (timestamp - lastFrameTime >= frameInterval) {
      currentFrame = (currentFrame + 1) % frames.length
      drawFrame(currentFrame)
      lastFrameTime = timestamp
    }
    animationId = requestAnimationFrame(tick)
  }
  lastFrameTime = performance.now()
  animationId = requestAnimationFrame(tick)
}

function pause() {
  if (animationId !== null) {
    cancelAnimationFrame(animationId)
    animationId = null
  }
  // 回到首帧
  currentFrame = 0
  drawFrame(0)
}

onMounted(init)
onUnmounted(pause)
</script>

<template>
  <canvas
    ref="canvasRef"
    class="sequence-canvas"
    @mouseenter="play"
    @mouseleave="pause"
  />
</template>

<style scoped>
.sequence-canvas {
  width: 100%;
  height: 100%;
  display: block;
}
</style>
```

**Step 3: Rust 新增 list_sequence_frames 命令**

在 `commands.rs` 添加：

```rust
/// 列出序列帧目录中的所有帧文件路径（按文件名排序）
#[tauri::command]
pub fn list_sequence_frames(dir_path: String) -> Result<Vec<String>, String> {
    let dir = Path::new(&dir_path);
    if !dir.exists() {
        return Ok(Vec::new());
    }

    let mut files: Vec<String> = Vec::new();

    let entries = fs::read_dir(dir).map_err(|e| format!("无法读取目录: {}", e))?;

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file() {
            let ext = path.extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_lowercase();
            // 只返回图片文件
            if matches!(ext.as_str(), "png" | "jpg" | "jpeg" | "webp" | "bmp") {
                files.push(path.to_string_lossy().to_string());
            }
        }
    }

    files.sort();
    Ok(files)
}
```

在 `lib.rs` 注册 `commands::list_sequence_frames`。

**Step 4: cargo check**

---

## Task 2: MaterialCard 集成序列帧预览

**Files:**
- Modify: `src/components/MaterialCard.vue`

将序列帧卡片的预览区域从静态首帧图片改为 `SequencePreview` 组件。

在 template 中，替换预览区的 `<img>` 逻辑：

```vue
<!-- 序列帧：Canvas 动画预览 -->
<SequencePreview
  v-if="material.material_type === 'sequence'"
  :folder-path="material.path"
  :fps="24"
  :max-width="200"
/>
<!-- 静帧/其他：图片预览 -->
<img
  v-else-if="material.preview_path"
  :src="convertFileSrc(material.preview_path)"
  :alt="material.name"
  class="preview-img"
  loading="lazy"
/>
```

import SequencePreview 组件。

**Step 2: vue-tsc 验证**

---

## Task 3: Rust 素材版本扫描命令

**Files:**
- Modify: `src-tauri/src/models.rs`（新增 MaterialVersion）
- Modify: `src-tauri/src/commands.rs`（新增 scan_material_versions）
- Modify: `src-tauri/src/lib.rs`（注册命令）

**Step 1: 新增 MaterialVersion 数据模型**

在 `models.rs` 末尾添加：

```rust
/// 素材版本信息（侧边栏"其他版本"用）
#[derive(Debug, Serialize, Clone)]
pub struct MaterialVersion {
    /// 阶段名称（"00_original", "01_scale", "02_done", "nextcloud"）
    pub stage: String,
    /// 阶段中文标签（"原始", "已缩放", "已完成", "已上传"）
    pub stage_label: String,
    /// 缩放比例（如 "100"、"70"、"50"），原始阶段为空
    pub scale: String,
    /// 文件完整路径
    pub file_path: String,
    /// 文件扩展名
    pub extension: String,
}
```

**Step 2: 新增 scan_material_versions 命令**

在 `commands.rs` 添加：

```rust
/// 扫描指定素材在各工作流目录中的所有版本
#[tauri::command]
pub fn scan_material_versions(
    task_path: String,
    base_name: String,
    material_type: String,
) -> Result<Vec<MaterialVersion>, String> {
    let task_dir = Path::new(&task_path);
    let mut versions = Vec::new();

    // 00_original
    let original_dir = task_dir.join("00_original");
    if original_dir.exists() {
        collect_versions_flat(&original_dir, &base_name, "00_original", "原始", "", &mut versions);
    }

    // 01_scale — 子目录 [100], [70], [50] 等
    let scale_dir = task_dir.join("01_scale");
    if scale_dir.exists() {
        collect_versions_in_scale_dirs(&scale_dir, &base_name, &mut versions);
    }

    // 02_done — 子目录 [img-XX] 或 [an-XX-YY]
    let done_dir = task_dir.join("02_done");
    let prefix = if material_type == "sequence" { "an" } else { "img" };
    if done_dir.exists() {
        collect_versions_in_done_dirs(&done_dir, &base_name, prefix, &mut versions);
    }

    // nextcloud
    let nextcloud_dir = task_dir
        .parent()
        .and_then(|p| p.parent())
        .map(|vfx| vfx.join("nextcloud").join(task_dir.file_name().unwrap_or_default()));
    if let Some(nc) = nextcloud_dir {
        if nc.exists() {
            collect_versions_flat(&nc, &base_name, "nextcloud", "已上传", "", &mut versions);
        }
    }

    Ok(versions)
}

fn collect_versions_flat(
    dir: &Path, base_name: &str, stage: &str, label: &str, scale: &str,
    versions: &mut Vec<MaterialVersion>,
) {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if name.starts_with(base_name) {
                let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
                versions.push(MaterialVersion {
                    stage: stage.to_string(),
                    stage_label: label.to_string(),
                    scale: scale.to_string(),
                    file_path: path.to_string_lossy().to_string(),
                    extension: ext,
                });
            }
        }
    }
}

fn collect_versions_in_scale_dirs(
    scale_dir: &Path, base_name: &str, versions: &mut Vec<MaterialVersion>,
) {
    if let Ok(entries) = fs::read_dir(scale_dir) {
        let mut dirs: Vec<_> = entries.flatten().filter(|e| e.path().is_dir()).collect();
        dirs.sort_by_key(|e| e.file_name());

        for entry in dirs {
            let dir_name = entry.file_name().to_string_lossy().to_string();
            // 解析 [100], [70] 等
            let scale = dir_name.trim_start_matches('[').trim_end_matches(']').to_string();
            collect_versions_flat(&entry.path(), base_name, "01_scale", "已缩放", &scale, versions);
        }
    }
}

fn collect_versions_in_done_dirs(
    done_dir: &Path, base_name: &str, prefix: &str, versions: &mut Vec<MaterialVersion>,
) {
    if let Ok(entries) = fs::read_dir(done_dir) {
        let mut dirs: Vec<_> = entries.flatten().filter(|e| e.path().is_dir()).collect();
        dirs.sort_by_key(|e| e.file_name());

        for entry in dirs {
            let dir_name = entry.file_name().to_string_lossy().to_string();
            if !dir_name.starts_with(&format!("[{}-", prefix)) {
                continue;
            }
            // 解析 [img-100] → "100", [an-30-12] → "30"
            let inner = dir_name.trim_start_matches('[').trim_end_matches(']');
            let scale = inner.split('-').nth(1).unwrap_or("").to_string();
            collect_versions_flat(&entry.path(), base_name, "02_done", "已完成", &scale, versions);
        }
    }
}
```

**Step 3: 注册命令**

lib.rs 添加 `commands::scan_material_versions` 和 `commands::list_sequence_frames`。

**Step 4: cargo check**

---

## Task 4: 侧边栏"其他版本"区域

**Files:**
- Modify: `src/views/TaskPage.vue`

**Step 1: 新增版本数据和加载逻辑**

在 `<script setup>` 中添加：

```typescript
import { invoke } from '@tauri-apps/api/core'

interface MaterialVersion {
  stage: string
  stage_label: string
  scale: string
  file_path: string
  extension: string
}

const versions = ref<MaterialVersion[]>([])

// 修改 selectMaterial，加载版本数据
async function selectMaterial(material: MaterialInfo) {
  selectedMaterial.value = material
  versions.value = []
  try {
    versions.value = await invoke<MaterialVersion[]>('scan_material_versions', {
      taskPath: taskFolderPath,
      baseName: material.name,
      materialType: material.material_type,
    })
  } catch (e) {
    console.error('加载版本失败:', e)
  }
}
```

**Step 2: 在侧边栏模板中添加"其他版本"区域**

在 `.sidebar-info` 之后添加：

```html
<!-- 其他版本 -->
<div v-if="versions.length > 0" class="sidebar-versions">
  <p class="versions-title">其他版本</p>
  <div class="version-list">
    <div v-for="v in versions" :key="v.file_path" class="version-item">
      <button
        class="version-format-btn"
        :title="v.file_path"
        @click="openInExplorer(v.file_path)"
      >
        {{ v.extension.toUpperCase() }}
      </button>
      <span class="version-info">
        <template v-if="v.scale">{{ v.scale }}% - </template>
        {{ v.stage_label }}
      </span>
    </div>
  </div>
</div>
```

**Step 3: 添加样式**

```css
.sidebar-versions {
  display: flex;
  flex-direction: column;
  gap: var(--spacing-2);
}

.versions-title {
  font-size: var(--text-sm);
  font-weight: var(--font-weight-heading);
  color: var(--text-secondary);
  padding-bottom: var(--spacing-1);
  border-bottom: 1px solid var(--border-light);
}

.version-list {
  display: flex;
  flex-direction: column;
  gap: var(--spacing-1);
}

.version-item {
  display: flex;
  align-items: center;
  gap: var(--spacing-2);
}

.version-format-btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  min-width: 44px;
  height: 28px;
  padding: 0 var(--spacing-2);
  font-size: var(--text-xs);
  font-weight: var(--font-medium);
  border-radius: var(--radius-sm);
  border: 1px solid var(--border-medium);
  background: transparent;
  color: var(--text-secondary);
  cursor: pointer;
  transition: all var(--transition-fast);
}

.version-format-btn:hover {
  background: var(--color-primary);
  color: var(--color-neutral-0);
  border-color: var(--color-primary);
}

.version-info {
  font-size: var(--text-sm);
  color: var(--text-tertiary);
}
```

**Step 4: vue-tsc 验证**

---

## Task 5: Prototype 子分类扫描

**Files:**
- Modify: `src-tauri/src/commands.rs`（修改 scan_materials 支持 Prototype）

**Step 1: 修改 scan_materials 命令**

当前 `scan_materials` 直接扫描 `00_original/` 的一层内容。Prototype 的 `00_original/` 下有 7 个子分类目录，每个目录下才是真正的素材文件。

需要新增一个参数 `is_prototype: bool`，或根据任务名自动判定。推荐自动判定：

修改 `scan_materials` 函数签名，不变。在函数内部判断：

```rust
// 判断是否为 Prototype（任务目录名）
let task_name = task_dir.file_name()
    .and_then(|n| n.to_str())
    .unwrap_or("");
let is_prototype = task_name.to_lowercase() == "prototype";

if is_prototype {
    // Prototype：扫描每个子分类目录
    return scan_materials_prototype(task_dir);
}
// 正常流程（保持不变）...
```

新增 `scan_materials_prototype` 函数：

```rust
/// Prototype 特例：扫描子分类目录下的素材
fn scan_materials_prototype(task_dir: &Path) -> Result<Vec<MaterialInfo>, String> {
    let original_dir = task_dir.join("00_original");
    if !original_dir.exists() {
        return Ok(Vec::new());
    }

    let scale_dir = task_dir.join("01_scale");
    let done_dir = task_dir.join("02_done");
    let nextcloud_dir = task_dir
        .parent()
        .and_then(|p| p.parent())
        .map(|vfx| vfx.join("nextcloud").join("Prototype"));

    let mut materials = Vec::new();

    // 读取子分类目录
    let sub_entries = fs::read_dir(&original_dir)
        .map_err(|e| format!("无法读取 Prototype/00_original: {}", e))?;

    for sub_entry in sub_entries {
        let sub_entry = sub_entry.map_err(|e| format!("读取失败: {}", e))?;
        let sub_path = sub_entry.path();
        if !sub_path.is_dir() { continue; }

        let sub_name = sub_path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();
        if sub_name.starts_with('.') { continue; }

        // 扫描子分类目录下的素材文件
        let sub_entries_inner = fs::read_dir(&sub_path)
            .map_err(|e| format!("无法读取子分类 {}: {}", sub_name, e))?;

        for entry in sub_entries_inner {
            let entry = entry.map_err(|e| format!("读取失败: {}", e))?;
            let path = entry.path();

            let file_name = path.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown")
                .to_string();

            if file_name.starts_with('.') { continue; }

            if path.is_dir() {
                // 序列帧
                let frame_count = count_frames(&path);
                let first_frame = find_first_frame(&path);
                let size_bytes = calc_dir_size(&path);
                let base_name = file_name.clone();

                // Prototype 序列帧进度：检查 02_done/[an-*]/{sub_name}/ 和 nextcloud/Prototype/{sub_name}/
                let progress = determine_progress_prototype_seq(
                    &base_name, &sub_name, &done_dir, &nextcloud_dir,
                );

                materials.push(MaterialInfo {
                    name: format!("{}/{}", sub_name, base_name),
                    file_name,
                    path: path.to_string_lossy().to_string(),
                    material_type: MaterialType::Sequence,
                    progress,
                    size_bytes,
                    frame_count,
                    extension: "seq".to_string(),
                    preview_path: first_frame,
                });
            } else {
                // 单个文件
                let ext = path.extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("")
                    .to_lowercase();
                let size_bytes = path.metadata().map(|m| m.len()).unwrap_or(0);
                let stem = path.file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("unknown");
                let base_name = stem.strip_suffix("_01").unwrap_or(stem).to_string();

                let material_type = match ext.as_str() {
                    "png" | "jpg" | "jpeg" | "webp" | "bmp" | "gif" => MaterialType::Image,
                    "mp4" | "mov" | "avi" | "webm" => MaterialType::Video,
                    _ => MaterialType::Other,
                };

                let progress = if material_type == MaterialType::Image {
                    determine_progress_prototype_img(
                        &base_name, &sub_name, &scale_dir, &done_dir, &nextcloud_dir,
                    )
                } else {
                    MaterialProgress::Original
                };

                materials.push(MaterialInfo {
                    name: format!("{}/{}", sub_name, base_name),
                    file_name,
                    path: path.to_string_lossy().to_string(),
                    material_type: material_type,
                    progress,
                    size_bytes,
                    frame_count: 0,
                    extension: ext,
                    preview_path: Some(path.to_string_lossy().to_string()),
                });
            }
        }
    }

    materials.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(materials)
}
```

Prototype 专用进度判定辅助函数（在各子目录下多查一层子分类名）：

```rust
fn determine_progress_prototype_img(
    base_name: &str, sub_name: &str,
    scale_dir: &Path, done_dir: &Path,
    nextcloud_dir: &Option<std::path::PathBuf>,
) -> MaterialProgress {
    // nextcloud/Prototype/{sub_name}/
    if let Some(nc) = nextcloud_dir {
        let nc_sub = nc.join(sub_name);
        if nc_sub.exists() && find_file_in_dir(&nc_sub, base_name) {
            return MaterialProgress::Uploaded;
        }
    }
    // 02_done/[img-XX]/{sub_name}/
    if done_dir.exists() {
        if find_file_in_proto_subdirs(done_dir, base_name, sub_name, "img") {
            return MaterialProgress::Done;
        }
    }
    // 01_scale/[XX]/{sub_name}/
    if scale_dir.exists() {
        if find_file_in_proto_subdirs(scale_dir, base_name, sub_name, "") {
            return MaterialProgress::Scaled;
        }
    }
    MaterialProgress::Original
}

fn determine_progress_prototype_seq(
    base_name: &str, sub_name: &str,
    done_dir: &Path,
    nextcloud_dir: &Option<std::path::PathBuf>,
) -> MaterialProgress {
    if let Some(nc) = nextcloud_dir {
        let nc_sub = nc.join(sub_name);
        if nc_sub.exists() && find_file_in_dir(&nc_sub, base_name) {
            return MaterialProgress::Uploaded;
        }
    }
    if done_dir.exists() {
        if find_file_in_proto_subdirs(done_dir, base_name, sub_name, "an") {
            return MaterialProgress::Done;
        }
    }
    MaterialProgress::Original
}

/// 在 [prefix-XX]/{sub_name}/ 下查找文件
fn find_file_in_proto_subdirs(
    dir: &Path, base_name: &str, sub_name: &str, prefix: &str,
) -> bool {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() { continue; }
            let dir_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if prefix.is_empty() || dir_name.starts_with(&format!("[{}-", prefix)) {
                let sub_dir = path.join(sub_name);
                if sub_dir.exists() && find_file_in_dir(&sub_dir, base_name) {
                    return true;
                }
            }
        }
    }
    false
}
```

**Step 2: cargo check**

---

## Task 6: 前端 Prototype 子分类视图

**Files:**
- Modify: `src/views/TaskPage.vue`

**Step 1: 树形视图支持 Prototype 子分类分组**

Prototype 的素材 `name` 格式为 `"symbol/h1_dice"`，可以用 `/` 分割提取子分类。

修改 `groupedMaterials` computed：

```typescript
/** 判断是否为 Prototype 任务 */
const isPrototype = computed(() => taskId.toLowerCase() === 'prototype')

/** 树形视图分组数据 */
const groupedMaterials = computed(() => {
  if (isPrototype.value) {
    // Prototype：按子分类分组
    const groupMap = new Map<string, MaterialInfo[]>()
    for (const m of materials.value) {
      const slashIndex = m.name.indexOf('/')
      const subCategory = slashIndex > 0 ? m.name.substring(0, slashIndex) : '其他'
      if (!groupMap.has(subCategory)) groupMap.set(subCategory, [])
      groupMap.get(subCategory)!.push(m)
    }
    return Array.from(groupMap.entries())
      .sort(([a], [b]) => a.localeCompare(b))
      .map(([label, items]) => ({ label, items }))
  }

  // 普通任务：按类型分组
  const images = materials.value.filter(m => m.material_type === 'image')
  const sequences = materials.value.filter(m => m.material_type === 'sequence')
  const videos = materials.value.filter(m => m.material_type === 'video')
  const others = materials.value.filter(m => m.material_type === 'other')

  const groups: { label: string; items: MaterialInfo[] }[] = []
  if (images.length) groups.push({ label: '静帧', items: images })
  if (sequences.length) groups.push({ label: '序列帧', items: sequences })
  if (videos.length) groups.push({ label: '视频', items: videos })
  if (others.length) groups.push({ label: '其他', items: others })
  return groups
})
```

**Step 2: MaterialCard 显示名优化**

Prototype 素材的 `name` 是 `"symbol/h1_dice"` 格式，卡片上只显示文件基础名（去掉子分类前缀）。

在 MaterialCard 中，可以用 computed 或直接在模板处理：

```vue
<!-- 显示名：去掉子分类前缀 -->
<span class="card-name" :title="material.file_name">
  {{ material.name.includes('/') ? material.name.split('/')[1] : material.name }}
</span>
```

**Step 3: vue-tsc 验证**

---

## Task 7: 构建验证与收尾

**Step 1: TypeScript 检查**
```bash
npx vue-tsc --noEmit
```

**Step 2: Vite 构建**
```bash
npm run build
```

**Step 3: Cargo 编译**
```bash
cargo build
```

**Step 4: 验证清单**
- [ ] 序列帧卡片悬停时播放动画，离开时回到首帧
- [ ] 静帧卡片预览不受影响
- [ ] 侧边栏显示"其他版本"区域（点击格式按钮打开文件）
- [ ] Prototype 任务页按子分类分组（big_win, symbol 等）
- [ ] Prototype 素材进度判定正确
- [ ] 普通任务不受影响
- [ ] 明暗主题正常
- [ ] 构建全通过

**Step 5: 更新计划文档**

---

## Phase 4b 实施记录

**完成时间:** 2026-02-17

### 已完成

- [x] Task 1: useFrameCache.ts LRU 缓存 + SequencePreview.vue Canvas 组件 + list_sequence_frames Rust 命令
- [x] Task 2: MaterialCard 集成序列帧预览（v-if 条件渲染 SequencePreview/img）
- [x] Task 3: MaterialVersion 数据模型 + scan_material_versions Rust 命令（含 00_original/01_scale/02_done/nextcloud 版本收集）
- [x] Task 4: 侧边栏"其他版本"区域 UI（格式按钮 + 阶段标签 + 缩放比例）
- [x] Task 5: scan_materials 支持 Prototype 自动判定 + 子分类扫描（含专用进度判定函数）
- [x] Task 6: 前端 Prototype 子分类分组视图 + MaterialCard 显示名去前缀
- [x] Task 7: vue-tsc + Vite build + cargo build 全部通过

### 计划外改进（会话中迭代完成）

- **序列帧默认循环播放**：SequencePreview 改为 mount 后自动播放，不再需要 hover 触发
- **侧边栏 Teleport 重构**：用 `<Teleport to="#content-row">` 将侧边栏移到 MainLayout 的 content-row 层级，与 main-content 平级展示
- **侧边栏滑入/滑出动画**：Vue `<Transition name="sidebar">` 包装，进入 300ms ease-out、离开 200ms ease-in
- **侧边栏可拖拽宽度**：支持 20%-60% 范围的拖拽调节，默认 30%
- **侧边栏点击空白关闭**：移除关闭按钮，改为点击主功能区空白处 / 再次点击同一卡片关闭
- **侧边栏高清预览**：序列帧使用 SequencePreview 组件（maxWidth=400），非序列帧使用 img
- **侧边栏字号调大**：标题、标签、版本信息统一调大一号
- **侧边栏内容对齐**：padding/radius 与 MainLayout 的 main-content 保持一致
- **侧边栏分区重构**：添加"基本信息"和"其他版本"两个带标题+下划线的分区
- **版本卡片重设计**：每个版本独立卡片，含版本名/大小/格式标签/圆形打开文件夹按钮
- **素材卡片大小改用 done 版本**：优先读取 02_done 中的文件大小（排除 .tps），fallback 到 00_original
- **卡片网格自适应**：flex 改为 CSS Grid `repeat(auto-fill, minmax(...))`，自动分配间距
- **卡片位置保持**：打开/关闭侧边栏时记录选中卡片视口位置，layout 变化后补偿 scrollTop
- **MaterialVersion 扩展**：新增 folder_path 和 size_bytes 字段

### 遗留问题

- MaterialProgress::None 变体 dead_code 警告（未被使用，保留备用）
