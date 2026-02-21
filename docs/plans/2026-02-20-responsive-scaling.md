# 多分辨率响应式缩放 Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 让 PGB1 在 1280×720（普通屏）到 3840×2160（4K 屏）的各种分辨率下 UI 比例一致、视觉舒适。

**Architecture:** 采用 `zoom` CSS 属性对 `#app` 根元素做动态缩放。基准分辨率设为 1920×1080（1x），窗口宽度变化时用 ResizeObserver 实时更新缩放比。设计系统中的所有 px 值保持设计稿原值不变，zoom 统一处理视觉比例。同时在程序设置中加"UI缩放"手动覆盖入口。

**Tech Stack:** Vue 3 Composable + CSS `zoom` + Tauri `appSettings.general.uiScale`（新增字段）

---

## 背景：被手动放大的部分（需要回退）

之前为适配 4K，以下对话框尺寸被手动放大过，全局缩放实装后需要回退到设计稿原值：

| 文件 | 属性 | 当前值 | 回退目标值 |
|------|------|--------|-----------|
| `CreateProjectDialog.vue` | min-width / max-width | 400px / 480px | 320px / 400px |
| `EditProjectDialog.vue` | min-width / max-width | 400px / 480px | 320px / 400px |
| `ShortcutDialog.vue` | width | 420px | 320px |
| `ConversionDialog.vue` | width | 760px | 560px |
| `ScalingDialog.vue` | width | 720px | 520px |
| `NormalizationDialog.vue` | width | 640px | 460px |
| `UploadConfirmDialog.vue` | min-width / max-width | 360px / 480px | 260px / 360px |
| `TaskListPage.vue`（确认弹窗） | max-width | 400px | 300px |
| `TaskPage.vue`（确认弹窗） | max-width | 420px | 320px |
| `TaskListDialog.vue`（弹窗） | min-width / max-width | 400px / 520px | 300px / 420px |

---

## 缩放算法

```
scale = clamp(windowWidth / 1920, 0.67, 1.25)
```

- 1280px 宽 → scale = 0.67（最小，防止过小）
- 1920px 宽 → scale = 1.00（标准 1080P，基准）
- 2560px 宽 → scale = 1.25（最大，防止过大）
- 3840px 宽 → scale = 1.25（clamp 上限，4K 屏显示稍大，舒适）

用户可在设置里手动设置 `uiScale`（0.5 ~ 2.0），覆盖自动值（0 = 跟随窗口自动）。

---

## Task 1：新增 useScale composable（核心缩放逻辑）

**Files:**
- Create: `src/composables/useScale.ts`

**目标：** 创建一个模块级单例 composable，监听窗口大小变化，自动更新 `#app` 元素的 `zoom` 样式。

**实现代码：**

```typescript
// src/composables/useScale.ts
import { ref, watch } from 'vue'

// 自动缩放的上下限（基准 1920px）
const BASE_WIDTH = 1920
const MIN_SCALE = 0.67
const MAX_SCALE = 1.25

// 模块级单例
const manualScale = ref(0) // 0 = 自动；> 0 = 用户手动值
const currentScale = ref(1)
let resizeObserver: ResizeObserver | null = null

function calcAutoScale(width: number): number {
  const s = width / BASE_WIDTH
  return Math.min(Math.max(s, MIN_SCALE), MAX_SCALE)
}

function applyScale(scale: number) {
  currentScale.value = scale
  const el = document.getElementById('app')
  if (el) {
    el.style.zoom = String(scale)
  }
}

export function useScale() {
  function initScale(savedManualScale = 0) {
    manualScale.value = savedManualScale

    // 监听窗口宽度变化
    if (!resizeObserver) {
      resizeObserver = new ResizeObserver((entries) => {
        const width = entries[0]?.contentRect.width ?? window.innerWidth
        if (manualScale.value > 0) {
          applyScale(manualScale.value)
        } else {
          applyScale(calcAutoScale(width))
        }
      })
      resizeObserver.observe(document.body)
    }

    // 立即应用一次
    const width = window.innerWidth
    if (manualScale.value > 0) {
      applyScale(manualScale.value)
    } else {
      applyScale(calcAutoScale(width))
    }
  }

  function setManualScale(scale: number) {
    manualScale.value = scale
    if (scale > 0) {
      applyScale(scale)
    } else {
      applyScale(calcAutoScale(window.innerWidth))
    }
  }

  return {
    currentScale,
    manualScale,
    initScale,
    setManualScale,
  }
}
```

**验证：** 在浏览器 Console 运行：
```javascript
// 手动触发，检查 #app 是否有 zoom 样式
document.getElementById('app').style.zoom
// 应该输出一个数字字符串，比如 "0.67" 或 "1"
```

---

## Task 2：在 App.vue 中初始化缩放

**Files:**
- Modify: `src/App.vue`

**目标：** 在应用启动时调用 `initScale()`，并从 `AppSettings.general.uiScale` 读取手动值。

**当前 App.vue：**
```vue
<script setup lang="ts">
import { computed, onMounted } from 'vue'
import { useRoute } from 'vue-router'
import { useTheme } from './composables/useTheme'
import MainLayout from './layouts/MainLayout.vue'

const route = useRoute()
const { initTheme } = useTheme()
onMounted(initTheme)
```

**修改后：**
```vue
<script setup lang="ts">
import { computed, onMounted } from 'vue'
import { useRoute } from 'vue-router'
import { useTheme } from './composables/useTheme'
import { useScale } from './composables/useScale'
import { useSettings } from './composables/useSettings'
import MainLayout from './layouts/MainLayout.vue'

const route = useRoute()
const { initTheme } = useTheme()
const { initScale } = useScale()
const { loadSettings } = useSettings()

onMounted(async () => {
  initTheme()
  // 先用默认值初始化（避免页面闪烁），再加载设置覆盖
  initScale(0)
  const settings = await loadSettings()
  if (settings?.general?.uiScale && settings.general.uiScale > 0) {
    initScale(settings.general.uiScale)
  }
})
```

**注意：** `general.uiScale` 字段还未存在，Task 3 会添加。这里用可选链 `?.` 防御。

---

## Task 3：扩展数据模型（Rust + TypeScript）

### 3a. Rust 模型

**Files:**
- Modify: `src-tauri/src/models.rs`（找 `GeneralSettings` struct，约 L402）

在 `GeneralSettings` 中新增 `ui_scale` 字段：

```rust
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct GeneralSettings {
    pub project_root_dir: String,
    /// UI 缩放比例（0.0 = 自动跟随窗口，> 0 = 手动固定值）
    #[serde(default)]
    pub ui_scale: f32,
}
```

在 `Default for AppSettings` 的 `general` 部分加 `ui_scale: 0.0`：
```rust
general: GeneralSettings {
    project_root_dir: String::new(),
    ui_scale: 0.0,
},
```

### 3b. TypeScript 接口

**Files:**
- Modify: `src/composables/useSettings.ts`

在 `GeneralSettings` 接口加字段：
```typescript
export interface GeneralSettings {
  projectRootDir: string
  uiScale: number  // 0 = 自动；> 0 = 手动固定值
}
```

**验证：** `cargo check`（在 `src-tauri/` 目录下执行）应无编译错误。

---

## Task 4：回退被手动放大的对话框尺寸

**目标：** 把之前为 4K 手动放大过的对话框尺寸回退到设计稿原值。全局缩放上线后，这些大尺寸在 4K 屏上会被 zoom 放到合适大小。

### 4a. CreateProjectDialog.vue

**File:** `src/components/CreateProjectDialog.vue`

找到 `.dialog`（或 `.dialog-content`）的宽度样式：
- `min-width: 400px` → `min-width: 320px`
- `max-width: 480px` → `max-width: 400px`

### 4b. EditProjectDialog.vue

**File:** `src/components/EditProjectDialog.vue`

同上：
- `min-width: 400px` → `min-width: 320px`
- `max-width: 480px` → `max-width: 400px`

### 4c. ShortcutDialog.vue

**File:** `src/components/ShortcutDialog.vue`

- `width: 420px` → `width: 320px`

### 4d. ConversionDialog.vue

**File:** `src/components/ConversionDialog.vue`

- `width: 760px` → `width: 560px`

### 4e. ScalingDialog.vue

**File:** `src/components/ScalingDialog.vue`

- `width: 720px` → `width: 520px`

### 4f. NormalizationDialog.vue

**File:** `src/components/NormalizationDialog.vue`

- `width: 640px` → `width: 460px`

### 4g. UploadConfirmDialog.vue

**File:** `src/components/UploadConfirmDialog.vue`

- `min-width: 360px` → `min-width: 260px`
- `max-width: 480px` → `max-width: 360px`

### 4h. TaskListPage.vue（确认弹窗）

**File:** `src/views/TaskListPage.vue`

搜索删除确认弹窗 CSS：
- `max-width: 400px` → `max-width: 300px`

### 4i. TaskPage.vue（确认弹窗）

**File:** `src/views/TaskPage.vue`

搜索子任务确认对话框 CSS：
- `max-width: 420px` → `max-width: 320px`

### 4j. TaskListDialog.vue

**File:** `src/components/TaskListDialog.vue`

- `min-width: 400px` → `min-width: 300px`
- `max-width: 520px` → `max-width: 420px`

---

## Task 5：在设置页面加 UI 缩放滑块

**Files:**
- Modify: 设置页面组件（找 `src/views/` 下的设置页面，检查 SettingsPage.vue 或类似文件）

**目标：** 在"通用"分组中添加"UI 缩放"设置项，让用户可以手动调节缩放比（或选择自动）。

**交互设计（简洁版）：**
- 一个 `<select>` 下拉：「自动（跟随窗口）/ 75% / 80% / 90% / 100% / 110% / 120% / 150%」
- 选择后立即生效（实时预览），保存到 settings
- 默认：自动

**实现片段（加入到现有设置页面的通用分组）：**

```vue
<!-- 在通用设置分组内添加 -->
<div class="setting-row">
  <label class="setting-label">UI 缩放</label>
  <select
    class="setting-select"
    :value="settings.general.uiScale"
    @change="onScaleChange"
  >
    <option :value="0">自动（跟随窗口）</option>
    <option :value="0.75">75%</option>
    <option :value="0.80">80%</option>
    <option :value="0.90">90%</option>
    <option :value="1.0">100%</option>
    <option :value="1.1">110%</option>
    <option :value="1.2">120%</option>
    <option :value="1.5">150%</option>
  </select>
</div>
```

```typescript
// 在 script setup 中
import { useScale } from '../composables/useScale'
const { setManualScale } = useScale()

function onScaleChange(e: Event) {
  const val = parseFloat((e.target as HTMLSelectElement).value)
  settings.value!.general.uiScale = val
  setManualScale(val)
  // 注意：用户点「保存」才持久化，这里只是实时预览
}
```

---

## Task 6：处理弹出窗口的缩放问题

**背景：** `zoom` 作用在 `#app` 上，但某些元素用了 `Teleport to="body"`（所有弹窗、侧边栏等）。这些被 teleport 出去的元素**不会**继承 `#app` 的 zoom，在 4K 屏上会显得很小。

**解决方案：** 在 `body` 上也应用同样的 zoom，这样所有 Teleport 到 body 的元素也会被缩放。

**修改 useScale.ts：**

```typescript
function applyScale(scale: number) {
  currentScale.value = scale
  // 同时缩放 #app 和 body（body 覆盖所有 Teleport 目标）
  const appEl = document.getElementById('app')
  if (appEl) appEl.style.zoom = String(scale)
  document.body.style.zoom = String(scale)
}
```

**注意事项：** 给 `body` 设 zoom 后，body 的实际可视宽度会变成 `window.innerWidth / scale`。ResizeObserver 监听 `document.body` 读到的 `contentRect.width` 会是缩放后的值。需要改为监听 `document.documentElement`（`<html>` 元素）获取真实物理宽度：

```typescript
resizeObserver = new ResizeObserver(() => {
  const width = window.innerWidth  // 使用 window.innerWidth，不受 zoom 影响
  ...
})
resizeObserver.observe(document.documentElement)
```

---

## Task 7：验证与收尾

### 7a. 在测试环境验证

**启动应用：**
```bash
npm run tauri dev
```

**检查清单：**
1. 窗口大小 1280×720 时，`#app` 的 zoom 值应约为 0.67
2. 拉大窗口到 1920 宽，zoom 应为 1.0
3. 拖动窗口大小，UI 应随之平滑缩放（无卡顿）
4. 打开对话框（新建项目、编辑项目等），弹窗应与主界面同步缩放
5. 翻译窗口（独立 WebviewWindow）**不受影响**（它是独立进程，有自己的 zoom 逻辑，暂不处理）

### 7b. 更新 CODE_INDEX.md

更新 `CODE_INDEX.md` 中 `useTheme.ts` 下方，新增 `useScale.ts` 的说明行：

```markdown
| `useScale.ts` | ~60 | `initScale()`, `setManualScale()` | 全局 UI 缩放单例。基准 1920px，clamp [0.67, 1.25]，同步缩放 #app + body（覆盖 Teleport 元素）。支持用户手动覆盖（0 = 自动） |
```

---

## 注意事项

1. **翻译窗口不做**：`TranslatorPage` 是独立 WebviewWindow，Tauri 另起进程，zoom 不传递过去。保持现状。
2. **提醒弹窗不做**：`ReminderPage`、`OvertimePage` 同样是独立窗口。
3. **zoom 兼容性**：WebView2（Tauri 底层）完整支持 CSS `zoom`，无问题。
4. **回退时要先读文件再改**：Task 4 中每个文件改之前必须先 Read 确认行号，不要盲改。
5. **之后如果有新弹窗**：记得用设计稿原始尺寸，不要手动放大，缩放交给 zoom 处理。
