# 贴图板（Pinboard）Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 实现类 Snipaste 的截图收纳画布，集成在各页面中，支持自由拖拽/缩放/标注。

**Architecture:** 弹窗式贴图板（PinboardDialog，Teleport to body），内部自由画布容纳可拖拽/缩放的贴图项。每张贴图有独立的 Canvas 标注层，支持画笔/箭头/矩形/椭圆/文字/橡皮擦/撤销重做。存储采用 per-directory `.pgb1_pinboard.json`（元数据）+ `.pgb1_pins/`（图片文件），和笔记系统同模式。

**Tech Stack:** Tauri 2.x (Rust) + Vue 3 (Composition API) + Canvas 2D API

---

## 存储方案

| 页面 | 画布 key | 图片 & 元数据存储目录 |
|------|---------|---------------------|
| HomePage（我的项目） | `"home"` | `{项目根目录}` |
| ProjectPage（制作任务） | `"project"` | `{项目目录}` |
| TaskPage | `"task:{taskId}"` | `{项目目录}` |
| GameIntroPage | `"game-intro"` | `{项目目录}` |
| MaterialsPage | `"materials"` | `{项目目录}` |

**元数据文件** `.pgb1_pinboard.json`：

```json
{
  "home": {
    "pins": [
      {
        "id": "a1b2c3d4",
        "image": "a1b2c3d4.png",
        "x": 120,
        "y": 80,
        "width": 400,
        "height": 300,
        "annotations": [
          { "type": "pen", "color": "#FF3B30", "strokeWidth": 3, "points": [[10,20],[15,25]] },
          { "type": "arrow", "color": "#007AFF", "strokeWidth": 2, "start": [50,50], "end": [200,150] },
          { "type": "rect", "color": "#34C759", "strokeWidth": 2, "start": [10,10], "end": [100,80] },
          { "type": "ellipse", "color": "#FF9500", "strokeWidth": 2, "start": [50,50], "end": [150,100] },
          { "type": "text", "color": "#FF3B30", "text": "注意这里", "position": [100,50], "fontSize": 16 }
        ],
        "zIndex": 1,
        "created_at": "2026-03-09T10:30:00"
      }
    ],
    "viewport": { "panX": 0, "panY": 0, "zoom": 1 }
  }
}
```

**图片文件** `.pgb1_pins/{id}.png`：剪贴板粘贴的原始截图。

---

## 组件树

```
PinboardDialog.vue        — 弹窗外壳（Teleport to body, glass-strong, 可拖拽调整大小）
├── 顶部工具栏区域         — 粘贴按钮 | 工具选择 | 颜色选择 | 撤销/重做 | 缩放控制 | 关闭
├── PinboardCanvas.vue    — 自由画布容器（鼠标滚轮缩放 + 中键/空格拖拽平移）
│   └── PinItem.vue ×N    — 单张贴图（拖拽移动 + 8方向缩放 + 选中态 + 删除）
│       └── <canvas>       — 标注层（覆盖在图片上方，透明背景）
└── 底部状态栏             — 贴图数量 | 画布坐标 | 缩放百分比
```

---

## Task 1: Rust 数据模型

**Files:**
- Modify: `src-tauri/src/models.rs`（末尾新增）

**Step 1: 添加 Pinboard 数据模型**

在 `models.rs` 末尾追加：

```rust
// ─── 贴图板 ──────────────────────────────────────────────

/// 单条标注
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PinAnnotation {
    #[serde(rename = "type")]
    pub annotation_type: String,   // "pen" | "arrow" | "rect" | "ellipse" | "text"
    pub color: String,
    #[serde(default)]
    pub stroke_width: f64,
    #[serde(default)]
    pub points: Vec<[f64; 2]>,     // pen 用
    #[serde(default)]
    pub start: Option<[f64; 2]>,   // arrow/rect/ellipse 起点
    #[serde(default)]
    pub end: Option<[f64; 2]>,     // arrow/rect/ellipse 终点
    #[serde(default)]
    pub text: Option<String>,      // text 内容
    #[serde(default)]
    pub position: Option<[f64; 2]>, // text 位置
    #[serde(default)]
    pub font_size: Option<f64>,    // text 字号
}

/// 单张贴图
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PinInfo {
    pub id: String,
    pub image: String,             // 文件名（.pgb1_pins/ 下）
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    #[serde(default)]
    pub annotations: Vec<PinAnnotation>,
    #[serde(default)]
    pub z_index: u32,
    #[serde(default)]
    pub created_at: String,
}

/// 画布视口状态
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PinboardViewport {
    #[serde(default)]
    pub pan_x: f64,
    #[serde(default)]
    pub pan_y: f64,
    #[serde(default = "default_zoom")]
    pub zoom: f64,
}

fn default_zoom() -> f64 { 1.0 }

/// 单个画布的数据
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PinboardCanvas {
    #[serde(default)]
    pub pins: Vec<PinInfo>,
    #[serde(default)]
    pub viewport: Option<PinboardViewport>,
}

/// .pgb1_pinboard.json 根结构（key → 画布）
pub type PinboardData = std::collections::HashMap<String, PinboardCanvas>;
```

**Step 2: 验证编译**

Run: `cd src-tauri && cargo check`
Expected: 编译通过，无错误

**Step 3: Commit**

```bash
git add src-tauri/src/models.rs
git commit -m "feat(pinboard): add Rust data models for pinboard system"
```

---

## Task 2: Rust 后端命令

**Files:**
- Modify: `src-tauri/src/commands.rs`（末尾新增，在笔记系统之后）
- Modify: `src-tauri/src/lib.rs`（注册命令）

**Step 1: 在 commands.rs 末尾添加贴图板命令**

在 `set_note` 函数之后追加：

```rust
// ─── 贴图板系统 ───────────────────────────────────────────

/// 读取 .pgb1_pinboard.json
fn read_pinboard_file(dir: &Path) -> models::PinboardData {
    let path = dir.join(".pgb1_pinboard.json");
    fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

/// 写入 .pgb1_pinboard.json（空时删文件）
fn write_pinboard_file(dir: &Path, data: &models::PinboardData) -> Result<(), String> {
    let path = dir.join(".pgb1_pinboard.json");
    // 过滤掉空画布
    let non_empty: models::PinboardData = data.iter()
        .filter(|(_, canvas)| !canvas.pins.is_empty())
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    if non_empty.is_empty() {
        if path.exists() {
            let _ = fs::remove_file(&path);
        }
    } else {
        let json = serde_json::to_string_pretty(&non_empty)
            .map_err(|e| format!("序列化贴图板失败: {}", e))?;
        fs::write(&path, json)
            .map_err(|e| format!("写入贴图板失败: {}", e))?;
    }
    Ok(())
}

/// 获取指定画布的贴图数据
#[tauri::command]
pub fn get_pinboard(dir_path: String, key: String) -> Result<models::PinboardCanvas, String> {
    let dir = Path::new(&dir_path);
    let data = read_pinboard_file(dir);
    Ok(data.get(&key).cloned().unwrap_or(models::PinboardCanvas {
        pins: vec![],
        viewport: None,
    }))
}

/// 保存整个画布的贴图数据（前端批量保存）
#[tauri::command]
pub fn save_pinboard(dir_path: String, key: String, canvas: models::PinboardCanvas) -> Result<(), String> {
    let dir = Path::new(&dir_path);
    let mut data = read_pinboard_file(dir);
    if canvas.pins.is_empty() {
        data.remove(&key);
    } else {
        data.insert(key, canvas);
    }
    write_pinboard_file(dir, &data)
}

/// 保存剪贴板图片到 .pgb1_pins/ 目录，返回文件名
#[tauri::command]
pub fn save_pin_image(dir_path: String, image_data: Vec<u8>) -> Result<String, String> {
    let dir = Path::new(&dir_path);
    let pins_dir = dir.join(".pgb1_pins");
    if !pins_dir.exists() {
        fs::create_dir_all(&pins_dir)
            .map_err(|e| format!("创建 .pgb1_pins 目录失败: {}", e))?;
    }

    // 生成唯一 ID
    let id = format!("{:016x}", std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() & 0xFFFFFFFFFFFFFFFF);
    let filename = format!("{}.png", id);
    let file_path = pins_dir.join(&filename);

    fs::write(&file_path, &image_data)
        .map_err(|e| format!("写入贴图文件失败: {}", e))?;

    Ok(filename)
}

/// 删除贴图图片文件
#[tauri::command]
pub fn delete_pin_image(dir_path: String, filename: String) -> Result<(), String> {
    let dir = Path::new(&dir_path);
    let file_path = dir.join(".pgb1_pins").join(&filename);
    if file_path.exists() {
        fs::remove_file(&file_path)
            .map_err(|e| format!("删除贴图文件失败: {}", e))?;
    }
    Ok(())
}
```

**Step 2: 在 lib.rs 注册新命令**

在 `commands::get_notes,` 行之前添加：

```rust
            commands::get_pinboard,
            commands::save_pinboard,
            commands::save_pin_image,
            commands::delete_pin_image,
```

**Step 3: 验证编译**

Run: `cd src-tauri && cargo check`
Expected: 编译通过

**Step 4: Commit**

```bash
git add src-tauri/src/commands.rs src-tauri/src/lib.rs
git commit -m "feat(pinboard): add Rust backend commands for pinboard CRUD"
```

---

## Task 3: i18n 国际化

**Files:**
- Modify: `src/locales/zh-CN.ts`（新增 pinboard 命名空间）
- Modify: `src/locales/en.ts`（对齐结构）

**Step 1: 在两个 locale 文件中添加 pinboard 命名空间**

zh-CN.ts 末尾（在最后一个命名空间之后）追加：

```typescript
  pinboard: {
    title: '贴图板',
    paste: '粘贴',
    pasteHint: 'Ctrl+V 粘贴截图',
    noClipboardImage: '剪贴板中没有图片',
    empty: '粘贴截图到这里作为参考',
    delete: '删除贴图',
    deleteConfirm: '确定删除这张贴图？',
    tools: {
      select: '选择',
      pen: '画笔',
      arrow: '箭头',
      rect: '矩形',
      ellipse: '椭圆',
      text: '文字',
      eraser: '橡皮擦',
    },
    undo: '撤销',
    redo: '重做',
    zoomIn: '放大',
    zoomOut: '缩小',
    zoomReset: '重置缩放',
    pinCount: '{count} 张贴图',
  },
```

en.ts 对齐：

```typescript
  pinboard: {
    title: 'Pinboard',
    paste: 'Paste',
    pasteHint: 'Ctrl+V to paste screenshot',
    noClipboardImage: 'No image in clipboard',
    empty: 'Paste screenshots here for reference',
    delete: 'Delete pin',
    deleteConfirm: 'Delete this pin?',
    tools: {
      select: 'Select',
      pen: 'Pen',
      arrow: 'Arrow',
      rect: 'Rectangle',
      ellipse: 'Ellipse',
      text: 'Text',
      eraser: 'Eraser',
    },
    undo: 'Undo',
    redo: 'Redo',
    zoomIn: 'Zoom In',
    zoomOut: 'Zoom Out',
    zoomReset: 'Reset Zoom',
    pinCount: '{count} pins',
  },
```

**Step 2: 验证类型**

Run: `npx vue-tsc --noEmit`
Expected: 通过（locale 结构对齐）

**Step 3: Commit**

```bash
git add src/locales/zh-CN.ts src/locales/en.ts
git commit -m "feat(pinboard): add i18n locale keys for pinboard feature"
```

---

## Task 4: usePinboard Composable

**Files:**
- Create: `src/composables/usePinboard.ts`

**Step 1: 创建 composable**

```typescript
import { ref, unref, type Ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { convertFileSrc } from '@tauri-apps/api/core'

// ─── 类型 ─────────────────────────────────────────────────

export interface PinAnnotation {
  type: 'pen' | 'arrow' | 'rect' | 'ellipse' | 'text'
  color: string
  strokeWidth: number
  points?: [number, number][]     // pen
  start?: [number, number]        // arrow/rect/ellipse
  end?: [number, number]          // arrow/rect/ellipse
  text?: string                   // text
  position?: [number, number]     // text
  fontSize?: number               // text
}

export interface PinInfo {
  id: string
  image: string                   // 文件名（.pgb1_pins/ 下）
  x: number
  y: number
  width: number
  height: number
  annotations: PinAnnotation[]
  zIndex: number
  created_at: string
}

export interface PinboardViewport {
  panX: number
  panY: number
  zoom: number
}

export interface PinboardCanvas {
  pins: PinInfo[]
  viewport: PinboardViewport | null
}

// ─── Composable ────────────────────────────────────────────

export function usePinboard(dirPath: Ref<string> | string, canvasKey: Ref<string> | string) {
  const pins = ref<PinInfo[]>([])
  const viewport = ref<PinboardViewport>({ panX: 0, panY: 0, zoom: 1 })
  const loading = ref(false)

  /** 加载画布数据 */
  async function loadPinboard() {
    const dir = unref(dirPath)
    const key = unref(canvasKey)
    if (!dir || !key) return
    loading.value = true
    try {
      const canvas = await invoke<PinboardCanvas>('get_pinboard', { dirPath: dir, key })
      pins.value = canvas.pins ?? []
      if (canvas.viewport) {
        viewport.value = canvas.viewport
      }
    } catch (e) {
      console.error('加载贴图板失败:', e)
      pins.value = []
    } finally {
      loading.value = false
    }
  }

  /** 保存整个画布（拖拽/标注/删除后调用） */
  async function savePinboard() {
    const dir = unref(dirPath)
    const key = unref(canvasKey)
    if (!dir || !key) return
    try {
      await invoke('save_pinboard', {
        dirPath: dir,
        key,
        canvas: {
          pins: pins.value,
          viewport: viewport.value,
        },
      })
    } catch (e) {
      console.error('保存贴图板失败:', e)
    }
  }

  /** 从剪贴板粘贴图片，返回新 PinInfo（或 null） */
  async function pasteImage(): Promise<PinInfo | null> {
    const dir = unref(dirPath)
    if (!dir) return null

    try {
      // 浏览器 Clipboard API 读取图片
      const items = await navigator.clipboard.read()
      let blob: Blob | null = null
      for (const item of items) {
        if (item.types.includes('image/png')) {
          blob = await item.getType('image/png')
          break
        }
      }
      if (!blob) return null

      const arrayBuf = await blob.arrayBuffer()
      const bytes = Array.from(new Uint8Array(arrayBuf))

      // 获取图片尺寸
      const img = new Image()
      const url = URL.createObjectURL(blob)
      const size = await new Promise<{ w: number; h: number }>((resolve) => {
        img.onload = () => {
          resolve({ w: img.naturalWidth, h: img.naturalHeight })
          URL.revokeObjectURL(url)
        }
        img.onerror = () => {
          resolve({ w: 400, h: 300 })
          URL.revokeObjectURL(url)
        }
        img.src = url
      })

      // 保存到磁盘
      const filename = await invoke<string>('save_pin_image', {
        dirPath: dir,
        imageData: bytes,
      })

      // 计算初始尺寸（最大 600px 宽，等比缩放）
      const MAX_INITIAL_WIDTH = 600
      let w = size.w
      let h = size.h
      if (w > MAX_INITIAL_WIDTH) {
        h = Math.round(h * (MAX_INITIAL_WIDTH / w))
        w = MAX_INITIAL_WIDTH
      }

      const pin: PinInfo = {
        id: filename.replace('.png', ''),
        image: filename,
        x: 50 + Math.random() * 100,
        y: 50 + Math.random() * 100,
        width: w,
        height: h,
        annotations: [],
        zIndex: pins.value.length > 0
          ? Math.max(...pins.value.map(p => p.zIndex)) + 1
          : 1,
        created_at: new Date().toISOString(),
      }

      pins.value.push(pin)
      await savePinboard()
      return pin
    } catch (e) {
      console.error('粘贴图片失败:', e)
      return null
    }
  }

  /** 删除贴图 */
  async function deletePin(pinId: string) {
    const dir = unref(dirPath)
    if (!dir) return
    const pin = pins.value.find(p => p.id === pinId)
    if (!pin) return

    pins.value = pins.value.filter(p => p.id !== pinId)

    try {
      await invoke('delete_pin_image', { dirPath: dir, filename: pin.image })
    } catch (e) {
      console.error('删除贴图文件失败:', e)
    }

    await savePinboard()
  }

  /** 更新单张贴图属性（位置/大小/标注等） */
  function updatePin(pinId: string, updates: Partial<PinInfo>) {
    const idx = pins.value.findIndex(p => p.id === pinId)
    if (idx === -1) return
    pins.value[idx] = { ...pins.value[idx], ...updates }
  }

  /** 将指定贴图置顶 */
  function bringToFront(pinId: string) {
    const maxZ = pins.value.reduce((max, p) => Math.max(max, p.zIndex), 0)
    updatePin(pinId, { zIndex: maxZ + 1 })
  }

  /** 获取贴图图片的 asset URL */
  function getPinImageUrl(pin: PinInfo): string {
    const dir = unref(dirPath)
    if (!dir) return ''
    const filePath = `${dir}\\.pgb1_pins\\${pin.image}`
    return convertFileSrc(filePath)
  }

  return {
    pins,
    viewport,
    loading,
    loadPinboard,
    savePinboard,
    pasteImage,
    deletePin,
    updatePin,
    bringToFront,
    getPinImageUrl,
  }
}
```

**Step 2: 验证类型**

Run: `npx vue-tsc --noEmit`
Expected: 通过

**Step 3: Commit**

```bash
git add src/composables/usePinboard.ts
git commit -m "feat(pinboard): add usePinboard composable with CRUD + clipboard paste"
```

---

## Task 5: PinboardDialog 弹窗外壳

**Files:**
- Create: `src/components/PinboardDialog.vue`

**Step 1: 创建弹窗组件**

复用 FolderBrowserDialog 的弹窗 + 8 方向拖拽调整模式。弹窗内容区暂时只放占位文字和粘贴按钮，后续 Task 填入 PinboardCanvas。

关键点：
- Teleport to body，`glass-strong`
- props: `show: boolean`, `dirPath: string`, `canvasKey: string`
- emits: `close`
- 弹窗默认 75vw × 80vh，最小 40%，最大 95%
- localStorage 持久化尺寸（key `pgb1-pinboard-size`）
- 顶部工具栏：粘贴按钮 | 工具选择组（选择/画笔/箭头/矩形/椭圆/文字/橡皮擦）| 颜色选择（5色圆点）| 撤销/重做 | 关闭按钮
- 中间：画布区域（flex: 1）
- 底部状态栏：贴图数量 + 缩放百分比

**工具栏按钮样式**：复用 `.fb-action-btn` 同款 28×28 图标按钮。工具选择组用 `.pinboard-tool-group`（flex row, gap 2px, 选中态蓝色背景）。颜色选择用 14px 实心圆点（border: 2px solid transparent，选中态 border = 白色 + box-shadow）。

**5 色调色板**（从 design-system 语义色中取）：
1. `#FF3B30`（红色，≈ color-danger）
2. `#007AFF`（蓝色，≈ color-primary）
3. `#34C759`（绿色，≈ color-success）
4. `#FF9500`（橙色，≈ color-warning）
5. `#FFFFFF`（白色，暗色模式对比）

**完整组件代码**：由于此组件较大（~350 行），在实现时参考 `FolderBrowserDialog.vue` 的结构（overlay + dialog + resize handles + header + body），但工具栏部分是全新的。

核心 template 结构：

```html
<Teleport to="body">
  <Transition name="dialog">
    <div v-if="show" class="pb-overlay" @mousedown.self="$emit('close')">
      <div class="pb-dialog glass-strong" :style="{ width: dialogWidth + 'vw', height: dialogHeight + 'vh' }">
        <!-- 8方向 resize handles（同 FolderBrowserDialog） -->

        <!-- 工具栏 -->
        <div class="pb-toolbar">
          <button class="pb-tool-btn" :title="$t('pinboard.paste')" @click="onPaste">
            <!-- 粘贴 SVG -->
          </button>
          <div class="pb-separator" />

          <!-- 工具选择组 -->
          <button v-for="tool in tools" :key="tool.id"
            class="pb-tool-btn" :class="{ active: activeTool === tool.id }"
            :title="$t(`pinboard.tools.${tool.id}`)"
            @click="activeTool = tool.id">
            <!-- 各工具 SVG -->
          </button>
          <div class="pb-separator" />

          <!-- 颜色选择 -->
          <button v-for="color in COLORS" :key="color"
            class="pb-color-dot" :class="{ active: activeColor === color }"
            :style="{ background: color }"
            @click="activeColor = color" />
          <div class="pb-separator" />

          <!-- 撤销/重做 -->
          <button class="pb-tool-btn" :disabled="!canUndo" @click="undo"><!-- undo SVG --></button>
          <button class="pb-tool-btn" :disabled="!canRedo" @click="redo"><!-- redo SVG --></button>

          <div class="pb-spacer" />

          <!-- 缩放控制 -->
          <button class="pb-tool-btn" @click="zoomOut">−</button>
          <span class="pb-zoom-label">{{ Math.round(viewport.zoom * 100) }}%</span>
          <button class="pb-tool-btn" @click="zoomIn">+</button>
          <div class="pb-separator" />

          <!-- 关闭 -->
          <button class="pb-tool-btn pb-close-btn" @click="$emit('close')">× SVG</button>
        </div>

        <!-- 画布区域 -->
        <div class="pb-canvas-area">
          <PinboardCanvas
            :pins="pins"
            :viewport="viewport"
            :active-tool="activeTool"
            :active-color="activeColor"
            :get-image-url="getPinImageUrl"
            @update-pin="onUpdatePin"
            @select-pin="onSelectPin"
            @delete-pin="onDeletePin"
            @add-annotation="onAddAnnotation"
            @update-viewport="onUpdateViewport"
          />
          <p v-if="pins.length === 0" class="pb-empty">{{ $t('pinboard.empty') }}</p>
        </div>

        <!-- 底部状态栏 -->
        <div class="pb-statusbar">
          <span>{{ $t('pinboard.pinCount', { count: pins.length }) }}</span>
        </div>
      </div>
    </div>
  </Transition>
</Teleport>
```

script setup 核心逻辑：

```typescript
const props = defineProps<{ show: boolean; dirPath: string; canvasKey: string }>()
defineEmits<{ close: [] }>()

const { pins, viewport, loadPinboard, savePinboard, pasteImage, deletePin, updatePin, bringToFront, getPinImageUrl } = usePinboard(
  computed(() => props.dirPath),
  computed(() => props.canvasKey),
)

// 工具状态
type ToolId = 'select' | 'pen' | 'arrow' | 'rect' | 'ellipse' | 'text' | 'eraser'
const activeTool = ref<ToolId>('select')
const activeColor = ref('#FF3B30')
const COLORS = ['#FF3B30', '#007AFF', '#34C759', '#FF9500', '#FFFFFF']

const selectedPinId = ref<string | null>(null)

// 撤销/重做栈（per-pin）
const undoStacks = ref<Record<string, PinAnnotation[][]>>({})
const redoStacks = ref<Record<string, PinAnnotation[][]>>({})
// ...undo/redo 逻辑

// 弹窗尺寸（同 FolderBrowserDialog）
// ...resize 逻辑

// watch show → loadPinboard
watch(() => props.show, (v) => { if (v) loadPinboard() })

// Ctrl+V 快捷键
function onKeydown(e: KeyboardEvent) {
  if (e.ctrlKey && e.key === 'v') { onPaste(); e.preventDefault() }
  if (e.ctrlKey && e.key === 'z') { undo(); e.preventDefault() }
  if (e.ctrlKey && e.key === 'y') { redo(); e.preventDefault() }
  if (e.key === 'Delete' && selectedPinId.value) { onDeletePin(selectedPinId.value) }
}

onMounted(() => window.addEventListener('keydown', onKeydown))
onUnmounted(() => window.removeEventListener('keydown', onKeydown))
```

**Step 2: 验证类型**

Run: `npx vue-tsc --noEmit`

**Step 3: Commit**

```bash
git add src/components/PinboardDialog.vue
git commit -m "feat(pinboard): add PinboardDialog shell with toolbar and resize"
```

---

## Task 6: PinboardCanvas + PinItem

**Files:**
- Create: `src/components/PinboardCanvas.vue`
- Create: `src/components/PinItem.vue`

### PinboardCanvas

自由画布容器，实现：
- **鼠标滚轮缩放**：以鼠标位置为中心缩放（zoom 0.1 ~ 5.0）
- **中键 / 空格+左键 拖拽平移**
- 内部 div 用 `transform: translate(panX, panY) scale(zoom)` 变换
- 接收 PinItem 事件转发给父组件

```html
<div class="pb-canvas" ref="canvasRef"
  @wheel.prevent="onWheel"
  @mousedown="onCanvasMouseDown"
  @mousemove="onCanvasMouseMove"
  @mouseup="onCanvasMouseUp">
  <div class="pb-canvas-inner" :style="canvasTransform">
    <PinItem
      v-for="pin in pins" :key="pin.id"
      :pin="pin"
      :image-url="getImageUrl(pin)"
      :is-selected="pin.id === selectedPinId"
      :active-tool="activeTool"
      :active-color="activeColor"
      :active-stroke-width="activeStrokeWidth"
      @select="$emit('select-pin', pin.id)"
      @update="(updates) => $emit('update-pin', pin.id, updates)"
      @delete="$emit('delete-pin', pin.id)"
      @add-annotation="(ann) => $emit('add-annotation', pin.id, ann)"
    />
  </div>
</div>
```

### PinItem

单张贴图组件：
- **拖拽移动**：`select` 工具时，mousedown → mousemove 更新 x/y → mouseup emit update
- **8 方向缩放**：边缘 resize handles（同 FolderBrowserDialog 模式，但更小 = 6px）
- **选中态**：蓝色边框 2px + 显示 resize handles + 右上角删除按钮
- **置顶**：mousedown 时 emit select → 父组件调 bringToFront
- **Canvas 标注层**：`<canvas>` 覆盖在 `<img>` 上方，透明背景，尺寸跟随 pin.width/height
- **标注绘制**：根据 activeTool 处理 mousedown/mousemove/mouseup：
  - `pen`：收集 points 数组，实时绘制路径
  - `arrow`：记录 start → 实时绘制到当前位置 → mouseup 确定 end
  - `rect`/`ellipse`：同 arrow 模式
  - `text`：click 位置弹出小输入框，确认后添加 text annotation
  - `eraser`：鼠标经过时，检测与现有 annotation 的碰撞，命中则删除

**标注坐标系**：所有坐标相对于图片原始尺寸（归一化到 0~1 比例），渲染时乘以当前显示宽高。这样缩放贴图后标注位置不变。

**Canvas 渲染函数**：`renderAnnotations(ctx, annotations, displayWidth, displayHeight)`
- 遍历 annotations 数组，按类型分发绘制：
  - `pen`：`beginPath` → `moveTo` → `lineTo` 连线 → `stroke`
  - `arrow`：画线段 + 末端三角形箭头（计算角度）
  - `rect`：`strokeRect`
  - `ellipse`：`ellipse()` 或 `save/scale/arc/restore` 近似
  - `text`：`fillText`，字号按归一化比例缩放

**完整组件代码**：PinItem 约 300 行（template + script + style），PinboardCanvas 约 150 行。

**Step 1: 创建 PinboardCanvas.vue**

（完整代码见上述架构描述）

**Step 2: 创建 PinItem.vue**

（完整代码见上述架构描述）

**Step 3: 验证类型**

Run: `npx vue-tsc --noEmit`

**Step 4: Commit**

```bash
git add src/components/PinboardCanvas.vue src/components/PinItem.vue
git commit -m "feat(pinboard): add PinboardCanvas and PinItem with drag/resize/annotation"
```

---

## Task 7: 标注系统完整实现

**Files:**
- Modify: `src/components/PinItem.vue`（完善标注交互）

此 Task 确保 PinItem 的 Canvas 标注层完整支持所有工具。

### 画笔（pen）

mousedown 开始收集 points → mousemove 追加 point 并实时绘制 → mouseup 结束，emit `add-annotation`

```typescript
function startPen(e: MouseEvent) {
  const [nx, ny] = toNormalized(e)
  currentStroke = { type: 'pen', color: props.activeColor, strokeWidth: props.activeStrokeWidth, points: [[nx, ny]] }
}
function movePen(e: MouseEvent) {
  if (!currentStroke) return
  const [nx, ny] = toNormalized(e)
  currentStroke.points!.push([nx, ny])
  renderAll() // 重绘所有 annotations + currentStroke
}
function endPen() {
  if (currentStroke && currentStroke.points!.length > 1) {
    emit('add-annotation', currentStroke)
  }
  currentStroke = null
}
```

### 箭头（arrow）/ 矩形（rect）/ 椭圆（ellipse）

mousedown 记录 start → mousemove 更新 end 并实时预览 → mouseup emit

箭头末端三角形计算：
```typescript
function drawArrowHead(ctx: CanvasRenderingContext2D, x1: number, y1: number, x2: number, y2: number, size: number) {
  const angle = Math.atan2(y2 - y1, x2 - x1)
  ctx.beginPath()
  ctx.moveTo(x2, y2)
  ctx.lineTo(x2 - size * Math.cos(angle - Math.PI / 6), y2 - size * Math.sin(angle - Math.PI / 6))
  ctx.lineTo(x2 - size * Math.cos(angle + Math.PI / 6), y2 - size * Math.sin(angle + Math.PI / 6))
  ctx.closePath()
  ctx.fill()
}
```

### 文字（text）

click 时在 pin 上方弹出小 input（absolute 定位），输入后回车确认，创建 text annotation。

### 橡皮擦（eraser）

mousedown + mousemove 时，计算鼠标位置与每个 annotation 的碰撞：
- pen：遍历 points，检测距离 < 阈值
- arrow/rect/ellipse：检测鼠标是否在 start-end 包围盒内
- text：检测鼠标是否在 position ± fontSize 范围内

命中则 emit `remove-annotation(index)`，PinboardDialog 从 pin.annotations 中移除并压入 undo 栈。

### 撤销/重做

PinboardDialog 维护 per-pin 的 undo/redo 栈：

```typescript
const undoStacks = new Map<string, PinAnnotation[][]>()
const redoStacks = new Map<string, PinAnnotation[][]>()

function onAddAnnotation(pinId: string, ann: PinAnnotation) {
  const pin = pins.value.find(p => p.id === pinId)!
  // 压入 undo 栈（保存当前 annotations 快照）
  if (!undoStacks.has(pinId)) undoStacks.set(pinId, [])
  undoStacks.get(pinId)!.push([...pin.annotations])
  // 清空 redo
  redoStacks.set(pinId, [])
  // 添加标注
  pin.annotations.push(ann)
  savePinboard()
}

function undo() {
  if (!selectedPinId.value) return
  const stack = undoStacks.get(selectedPinId.value)
  if (!stack?.length) return
  const pin = pins.value.find(p => p.id === selectedPinId.value)!
  // 当前状态压入 redo
  if (!redoStacks.has(selectedPinId.value)) redoStacks.set(selectedPinId.value, [])
  redoStacks.get(selectedPinId.value)!.push([...pin.annotations])
  // 恢复上一步
  pin.annotations = stack.pop()!
  savePinboard()
}

function redo() {
  // 对称逻辑
}
```

**Step 1: 完善 PinItem 标注交互**

**Step 2: 完善 PinboardDialog 撤销/重做逻辑**

**Step 3: 验证类型**

Run: `npx vue-tsc --noEmit`

**Step 4: Commit**

```bash
git add src/components/PinItem.vue src/components/PinboardDialog.vue
git commit -m "feat(pinboard): complete annotation system with all tools + undo/redo"
```

---

## Task 8: 页面集成

**Files:**
- Modify: `src/views/HomePage.vue`
- Modify: `src/views/ProjectPage.vue`
- Modify: `src/views/TaskPage.vue`
- Modify: `src/views/GameIntroPage.vue`
- Modify: `src/views/MaterialsPage.vue`

每个页面的改动模式相同：

### 1. 导入

```typescript
import PinboardDialog from '../components/PinboardDialog.vue'
```

### 2. 状态

```typescript
const showPinboard = ref(false)
```

### 3. Template — 在 `.note-btn` 旁添加按钮

```html
<button
  class="note-btn"
  :title="$t('pinboard.title')"
  @click="showPinboard = true"
>
  <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
    <rect x="3" y="3" width="18" height="18" rx="2" />
    <path d="M9 3v18" />
    <path d="M3 9h6" />
  </svg>
</button>
```

SVG 图标说明：一个带网格线的矩形，暗示"画板/贴图板"。

### 4. Template — 弹窗

```html
<PinboardDialog
  :show="showPinboard"
  :dir-path="对应目录路径"
  :canvas-key="对应 key"
  @close="showPinboard = false"
/>
```

### 各页面具体参数

| 页面 | dirPath | canvasKey |
|------|---------|-----------|
| HomePage | `projectRootDir` | `'home'` |
| ProjectPage | `projectPath`（已有 let 变量） | `'project'` |
| TaskPage | `projectPath`（从路由 params 解析） | `` `task:${taskId}` `` |
| GameIntroPage | `dirPath`（已有 let 变量） | `'game-intro'` |
| MaterialsPage | `projectPath`（已有 let 变量） | `'materials'` |

### 按钮位置

紧跟在现有 `.note-btn` 之后，在 `.sort-tabs` / `.add-btn` 之前。

**Step 1: 修改 5 个页面文件**

**Step 2: 验证类型**

Run: `npx vue-tsc --noEmit`

**Step 3: 功能验证**

Run: `npm run tauri dev`
- 各页面可见贴图板按钮
- 点击打开弹窗
- Ctrl+V 粘贴截图
- 拖拽贴图移动
- 缩放贴图大小
- 使用各标注工具
- 撤销/重做
- 删除贴图
- 关闭弹窗后重新打开，数据持久化
- 切换页面/项目，画布独立

**Step 4: Commit**

```bash
git add src/views/HomePage.vue src/views/ProjectPage.vue src/views/TaskPage.vue src/views/GameIntroPage.vue src/views/MaterialsPage.vue
git commit -m "feat(pinboard): integrate pinboard button into all 5 pages"
```

---

## Task 9: CSS 公共样式

**Files:**
- Modify: `src/styles/design-system.css`（添加贴图板公共类）

在笔记相关样式之后追加：

```css
/* ─── 贴图板 ─── */
.pb-tool-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 28px;
  height: 28px;
  border: 1px solid transparent;
  border-radius: var(--radius-md);
  background: transparent;
  color: var(--text-secondary);
  cursor: pointer;
  transition: var(--transition-bg);
  flex-shrink: 0;
}
.pb-tool-btn:hover:not(:disabled) {
  background: var(--bg-hover);
  color: var(--text-primary);
}
.pb-tool-btn.active {
  background: var(--color-primary);
  color: var(--color-neutral-0);
  border-color: var(--color-primary);
}
.pb-tool-btn:disabled {
  opacity: 0.3;
  cursor: default;
}

.pb-separator {
  width: 1px;
  height: 18px;
  background: var(--border-light);
  margin: 0 var(--spacing-1);
  flex-shrink: 0;
}

.pb-color-dot {
  width: 16px;
  height: 16px;
  border-radius: 50%;
  border: 2px solid transparent;
  cursor: pointer;
  transition: var(--transition-bg);
  flex-shrink: 0;
}
.pb-color-dot:hover {
  transform: scale(1.2);
}
.pb-color-dot.active {
  border-color: var(--text-primary);
  box-shadow: 0 0 0 2px var(--bg-primary);
}
```

**Step 1: 添加样式**

**Step 2: Commit**

```bash
git add src/styles/design-system.css
git commit -m "feat(pinboard): add public CSS classes for pinboard toolbar"
```

---

## Task 10: 文档更新

**Files:**
- Modify: `CODE_INDEX.md`（新增组件和 composable 条目、更新统计）
- Modify: `INDEX.md`（如有必要）

更新内容：
- 组件表新增：`PinboardDialog.vue`、`PinboardCanvas.vue`、`PinItem.vue`
- Composable 表新增：`usePinboard.ts`
- 命令表新增：`get_pinboard`、`save_pinboard`、`save_pin_image`、`delete_pin_image`
- 文件统计更新

**Step 1: 更新 CODE_INDEX.md**

**Step 2: Commit**

```bash
git add CODE_INDEX.md
git commit -m "docs: update CODE_INDEX.md with pinboard system entries"
```

---

## 风险与注意事项

1. **剪贴板权限**：WebView2 的 `navigator.clipboard.read()` 需要页面处于 focus 状态，弹窗内应该满足。如果不行，fallback 用 Tauri clipboard plugin 的 `readImage()`。

2. **Canvas 性能**：每次鼠标移动重绘整个 canvas。贴图数量多（>20）且标注密集时可能卡顿。初期不优化，观察实际表现。

3. **大图片内存**：截图通过 `Vec<u8>` 传给 Rust，4K 截图约 30MB。如果内存压力大，后续考虑在 Rust 端压缩。

4. **backdrop-filter 冲突**：PinboardDialog 使用 `glass-strong`，Teleport to body，不与 `#content-row` 内的元素同层，不触发兄弟冲突。安全。

5. **标注坐标归一化**：所有标注坐标存储为 0~1 的归一化值，渲染时乘以当前显示宽高。这样贴图缩放后标注位置/大小自动适配。

---

## 实现顺序总结

| 顺序 | Task | 依赖 | 预估文件变更 |
|------|------|------|-------------|
| 1 | Rust 数据模型 | 无 | models.rs |
| 2 | Rust 后端命令 | Task 1 | commands.rs, lib.rs |
| 3 | i18n | 无 | zh-CN.ts, en.ts |
| 4 | usePinboard composable | Task 2 | usePinboard.ts (新) |
| 5 | PinboardDialog 弹窗 | Task 3, 4 | PinboardDialog.vue (新) |
| 6 | PinboardCanvas + PinItem | Task 5 | 2 个新文件 |
| 7 | 标注系统完善 | Task 6 | PinItem.vue, PinboardDialog.vue |
| 8 | 页面集成 | Task 5 | 5 个页面文件 |
| 9 | CSS 公共样式 | Task 5 | design-system.css |
| 10 | 文档更新 | 全部 | CODE_INDEX.md |
