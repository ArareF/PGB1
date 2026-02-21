# 视频缩略图 + 03_preview 展示 Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 为普通卡片（NormalCard.vue）添加视频静态帧截图，并在任务详情页展示 03_preview 目录的视频文件（用 FileDetailSidebar 播放）。

**Architecture:**
- Task 1：NormalCard.vue 新增视频截帧逻辑（纯前端 HTML5 Canvas，不依赖后端）
- Task 2：后端新增 `scan_preview_videos` 命令，扫描 `{taskPath}/03_preview/` 返回 `Vec<FileEntry>`
- Task 3：TaskPage.vue 新增 `previewVideos` 状态 + 展示区块 + 复用 FileDetailSidebar 播放

**Tech Stack:** Vue 3 Composition API, Tauri 2.x invoke, Rust `std::fs`, HTML5 `<video>` + Canvas

---

## Task 1：NormalCard.vue 视频截帧

**Files:**
- Modify: `src/components/NormalCard.vue`

**背景知识：**
`NormalCard.vue` 当前只处理图片和文件夹两种预览，视频文件走「其他文件图标」分支。
目标：当 `file.extension` 为视频格式时，用隐藏的 `<video>` 元素跳到 `0.1s` 处截帧，
生成 data URL 后作为卡片缩略图显示。

**当前图片预览代码（在 `<script setup>` 中）：**
```ts
const IMAGE_EXTS = new Set(['png', 'jpg', 'jpeg', 'gif', 'bmp', 'webp', 'svg', 'ico'])
const isImage = computed(() => !props.file.is_dir && IMAGE_EXTS.has(props.file.extension))
```

**Step 1: 添加视频截帧逻辑**

在 `NormalCard.vue` 的 `<script setup>` 中，在 `isImage` 之后添加：

```ts
import { ref, computed, onMounted, onUnmounted } from 'vue'
// （注：convertFileSrc 和 ref/computed 已导入，只需补充 onMounted/onUnmounted）

const VIDEO_EXTS = new Set(['mp4', 'mov', 'avi', 'mkv', 'webm', 'flv'])
const isVideo = computed(() => !props.file.is_dir && VIDEO_EXTS.has(props.file.extension))

const videoThumbnail = ref<string | null>(null)

onMounted(() => {
  if (!isVideo.value) return
  const video = document.createElement('video')
  video.crossOrigin = 'anonymous'
  video.preload = 'metadata'
  video.src = convertFileSrc(props.file.path)
  video.currentTime = 0.1

  video.addEventListener('seeked', () => {
    const canvas = document.createElement('canvas')
    canvas.width = video.videoWidth || 200
    canvas.height = video.videoHeight || 150
    const ctx = canvas.getContext('2d')
    if (ctx) {
      ctx.drawImage(video, 0, 0, canvas.width, canvas.height)
      videoThumbnail.value = canvas.toDataURL('image/jpeg', 0.7)
    }
    video.src = ''  // 释放资源
  }, { once: true })

  video.addEventListener('error', () => {
    // 截帧失败，保持 videoThumbnail 为 null（显示播放图标）
    video.src = ''
  }, { once: true })
})
```

**Step 2: 更新模板预览区域**

找到 `NormalCard.vue` 模板中的 `<div class="card-preview">` 部分：

**当前：**
```html
<img v-if="isImage" ... />
<svg v-else-if="file.is_dir" ...> ... </svg>
<svg v-else class="type-icon" ...> ... </svg>
```

**改为：**
```html
<img v-if="isImage" ... />
<!-- 视频：有截帧则显示截帧图，否则显示播放图标 -->
<img
  v-else-if="isVideo && videoThumbnail"
  :src="videoThumbnail"
  :alt="file.name"
  class="preview-img"
/>
<div v-else-if="isVideo" class="video-placeholder">
  <svg width="32" height="32" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
    <polygon points="5,3 19,12 5,21" fill="currentColor" stroke="none" opacity="0.6" />
  </svg>
</div>
<svg v-else-if="file.is_dir" ...> ... </svg>
<svg v-else ...> ... </svg>
```

**Step 3: 添加 `.video-placeholder` 样式**

在 `<style scoped>` 中添加：
```css
.video-placeholder {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 100%;
  height: 100%;
  color: var(--text-tertiary);
}
```

**Step 4: 验证**

在游戏介绍页或项目素材页中有视频文件时（如 `.mp4`），卡片应显示视频第一帧截图；
若截帧失败（视频损坏等），显示播放图标。

**Step 5: Commit**
```bash
git add src/components/NormalCard.vue
git commit -m "feat: NormalCard 视频文件显示静态帧截图"
```

---

## Task 2：后端新增 scan_preview_videos 命令

**Files:**
- Modify: `src-tauri/src/commands.rs`（添加命令函数，约 30 行）
- Modify: `src-tauri/src/lib.rs`（注册命令，1 行）

**背景知识：**
`FileEntry` 模型已存在于 `models.rs`，字段：`name: String, path: String, is_dir: bool, extension: String, size_bytes: u64`。
`scan_directory` 命令已实现通用目录扫描，但需要精确路径且不过滤类型。
新命令专门扫描 `{task_path}/03_preview/`，只返回视频文件（按 ext 过滤），目录不存在时返回空数组。

**Step 1: 在 commands.rs 末尾添加命令**

在文件最后一个 `#[tauri::command]` 函数之后添加：

```rust
/// 扫描任务的 03_preview 目录，返回视频文件列表（不存在则返回空）
#[tauri::command]
pub fn scan_preview_videos(task_path: String) -> Result<Vec<FileEntry>, String> {
    let preview_dir = Path::new(&task_path).join("03_preview");
    if !preview_dir.exists() {
        return Ok(Vec::new());
    }

    let video_exts: &[&str] = &["mp4", "mov", "avi", "mkv", "webm", "flv"];

    let entries = fs::read_dir(&preview_dir)
        .map_err(|e| format!("无法读取 03_preview: {}", e))?;

    let mut files: Vec<FileEntry> = entries
        .filter_map(|e| e.ok())
        .filter_map(|e| {
            let path = e.path();
            if path.is_dir() {
                return None;
            }
            let ext = path
                .extension()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_lowercase();
            if !video_exts.contains(&ext.as_str()) {
                return None;
            }
            let name = path.file_name()?.to_str()?.to_string();
            let size = fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
            Some(FileEntry {
                name,
                path: path.to_string_lossy().to_string(),
                is_dir: false,
                extension: ext,
                size_bytes: size,
            })
        })
        .collect();

    files.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(files)
}
```

**Step 2: 在 lib.rs 注册命令**

找到 `invoke_handler` 中 `commands::set_default_ae_file,` 这一行，在其后插入：
```rust
commands::scan_preview_videos,
```

**Step 3: 编译验证**

```bash
cd D:/work/pgsoft/PGB1
cargo build --manifest-path src-tauri/Cargo.toml 2>&1 | tail -20
```
预期：无编译错误（只有可能的 warnings）。

**Step 4: Commit**
```bash
git add src-tauri/src/commands.rs src-tauri/src/lib.rs
git commit -m "feat: 新增 scan_preview_videos 命令扫描 03_preview 视频"
```

---

## Task 3：TaskPage.vue 展示 03_preview 区块

**Files:**
- Modify: `src/views/TaskPage.vue`（Script + Template + Style）

**背景知识：**
- TaskPage 已有素材侧边栏（`selectedMaterial` + `detail-sidebar`），只处理 `MaterialInfo` 类型。
- `FileDetailSidebar.vue` 已完整实现视频播放器，接受 `file: FileEntry | null` prop。
- 计划：TaskPage 新增 `selectedPreviewVideo` ref（`FileEntry | null`），
  点击预览视频卡片 → 关闭素材侧边栏（如已打开）→ 用 `FileDetailSidebar` 展示视频。
- 两个侧边栏互斥：选中预览视频时清空 `selectedMaterial`，选中素材时清空 `selectedPreviewVideo`。

**Step 1: 添加 import 和状态**

在 `<script setup>` 顶部 import 区域添加：
```ts
import FileDetailSidebar from '../components/FileDetailSidebar.vue'
```

在 `<script setup>` 中 `selectedMaterial` ref 附近添加：
```ts
/** 03_preview 视频文件列表 */
const previewVideos = ref<FileEntry[]>([])
// FileEntry 类型已被 useDirectoryFiles 导出，需确认导入
// 若未导入，在 import 区域添加：
// import type { FileEntry } from '../composables/useDirectoryFiles'

/** 当前选中的预览视频（驱动 FileDetailSidebar） */
const selectedPreviewVideo = ref<FileEntry | null>(null)

/** 侧边栏共享宽度（两个侧边栏用同一宽度变量，避免跳变） */
const fileDetailWidthPercent = ref(30)
```

**Step 2: 加载 previewVideos**

在 `onMounted` 中，`loadMaterials(taskFolderPath)` 之后添加：
```ts
// 加载 03_preview 视频
try {
  previewVideos.value = await invoke<FileEntry[]>('scan_preview_videos', {
    taskPath: taskFolderPath,
  })
} catch (e) {
  console.error('加载预览视频失败:', e)
}
```

同时在 `refresh()` 函数中也补充刷新：
```ts
async function refresh() {
  if (taskFolderPath) {
    await loadMaterials(taskFolderPath)
    selectedPaths.value = new Set()
    // 同步刷新预览视频列表
    try {
      previewVideos.value = await invoke<FileEntry[]>('scan_preview_videos', {
        taskPath: taskFolderPath,
      })
    } catch (e) {
      console.error('刷新预览视频失败:', e)
    }
  }
}
```

**Step 3: 添加预览视频交互函数**

在 `selectMaterial` 函数附近添加：
```ts
function selectPreviewVideo(file: FileEntry) {
  // 再次点击同一视频则关闭
  if (selectedPreviewVideo.value?.path === file.path) {
    selectedPreviewVideo.value = null
    return
  }
  // 互斥：关闭素材侧边栏
  if (selectedMaterial.value) {
    closeSidebar()
  }
  selectedPreviewVideo.value = file
}
```

在 `closeSidebar` 函数中，原代码执行前补充清空预览视频：
```ts
// 注意：closeSidebar 清空素材侧边栏，selectMaterial 也要清空预览视频侧边栏
function selectMaterial(material: MaterialInfo) {
  // 互斥：关闭预览视频侧边栏
  selectedPreviewVideo.value = null
  // ... 原有代码不变 ...
}
```

**Step 4: 在模板中添加 03_preview 区块**

在 `scroll-content` 的 `</div>` 结束标签（即 `</div>` 关闭 `v-else` 名称视图）之后，
在外层 `</div>` 关闭 `scroll-content` 之前，添加预览视频区块：

```html
<!-- 03_preview 预览视频区块 -->
<div v-if="previewVideos.length > 0" class="preview-videos-section">
  <p class="section-label">预览视频</p>
  <div class="preview-videos-grid">
    <button
      v-for="video in previewVideos"
      :key="video.path"
      class="preview-video-card glass-subtle"
      :class="{ selected: selectedPreviewVideo?.path === video.path }"
      @click="selectPreviewVideo(video)"
    >
      <div class="pv-card-preview">
        <svg width="28" height="28" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" opacity="0.6">
          <polygon points="5,3 19,12 5,21" fill="currentColor" stroke="none" />
        </svg>
      </div>
      <div class="pv-card-info">
        <span class="pv-card-name">{{ video.name }}</span>
        <span class="pv-card-ext">{{ video.extension.toUpperCase() }}</span>
      </div>
    </button>
  </div>
</div>
```

**Step 5: 在末尾 Teleport 后添加 FileDetailSidebar**

在 `</Teleport>` 素材侧边栏结束标签之后，在 `<UploadConfirmDialog>` 之前添加：
```html
<!-- 预览视频侧边栏（复用 FileDetailSidebar） -->
<FileDetailSidebar
  :file="selectedPreviewVideo"
  v-model:widthPercent="fileDetailWidthPercent"
  @close="selectedPreviewVideo = null"
/>
```

**Step 6: 添加样式**

在 `<style scoped>` 末尾添加：
```css
/* ─── 03_preview 视频区块 ─── */
.preview-videos-section {
  padding: 0 var(--spacing-5) var(--spacing-6);
  display: flex;
  flex-direction: column;
  gap: var(--spacing-3);
}

.preview-videos-grid {
  display: flex;
  flex-wrap: wrap;
  gap: var(--gap-card);
}

.preview-video-card {
  width: 180px;
  display: flex;
  flex-direction: column;
  padding: var(--spacing-3);
  border-radius: var(--card-border-radius);
  border: 1px solid transparent;
  cursor: pointer;
  transition: var(--transition-card-hover);
  text-align: left;
  gap: var(--spacing-2);
}

.preview-video-card:hover {
  transform: translateY(var(--card-hover-lift));
  box-shadow: var(--card-shadow-hover);
}

.preview-video-card.selected {
  border-color: var(--color-primary);
  background: var(--color-primary-alpha, rgba(59, 130, 246, 0.1));
}

.pv-card-preview {
  width: 100%;
  aspect-ratio: 16 / 9;
  background: var(--bg-hover);
  border-radius: var(--radius-md);
  display: flex;
  align-items: center;
  justify-content: center;
  color: var(--text-tertiary);
  overflow: hidden;
}

.pv-card-info {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--spacing-2);
  min-width: 0;
}

.pv-card-name {
  font-size: var(--text-sm);
  color: var(--text-primary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  flex: 1;
  min-width: 0;
}

.pv-card-ext {
  font-size: var(--text-xs);
  color: var(--text-tertiary);
  flex-shrink: 0;
}
```

**Step 7: 验证**

1. 打开任务详情页，若 `03_preview/` 目录有 `.mp4` 文件，应在页面底部出现「预览视频」区块
2. 点击视频卡片 → 右侧打开 FileDetailSidebar，显示视频播放器
3. 点击素材卡片 → 预览视频侧边栏关闭，素材侧边栏打开
4. 若 `03_preview/` 不存在或为空，区块隐藏不显示

**Step 8: Commit**
```bash
git add src/views/TaskPage.vue
git commit -m "feat: 任务页展示 03_preview 预览视频，复用 FileDetailSidebar 播放"
```

---

## 注意事项

1. **FileEntry 类型导入**：`useDirectoryFiles.ts` 导出 `FileEntry` 接口，TaskPage.vue 如果未导入需补充 `import type { FileEntry } from '../composables/useDirectoryFiles'`

2. **onMainContentClick 互斥**：当前 `onMainContentClick` 点击空白区域只调用 `closeSidebar()`，需要同时关闭预览视频侧边栏：
   ```ts
   function onMainContentClick(e: MouseEvent) {
     const target = e.target as HTMLElement
     if (target.closest('.material-card')) return
     if (target.closest('.preview-video-card')) return  // 新增：不关闭预览视频
     closeSidebar()
     selectedPreviewVideo.value = null  // 新增
   }
   ```

3. **NormalCard.vue 的 import 修复**：当前只 `import { computed } from 'vue'`，截帧逻辑需要 `ref, onMounted`，需更新该行。

4. **`var(--color-primary-alpha)` CSS 变量**：若 design-system.css 中未定义此变量，用 `rgba(59, 130, 246, 0.1)` 的内联值替代，或检查 `design-system.css` 中 primary color 的 alpha 变量名。
