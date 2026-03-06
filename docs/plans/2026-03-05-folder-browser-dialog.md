# FolderBrowserDialog 实现计划

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 创建可复用的文件夹浏览弹窗组件，点击子文件夹卡片后在弹窗内展示文件夹内容，支持递归进入更深层子文件夹，替代当前"用系统资源管理器打开"的行为。

**Architecture:** 新建 `FolderBrowserDialog.vue` 组件，Teleport to body，内部维护路径栈实现递归浏览。弹窗尺寸默认占视口 70%×70%，四边可拖拽调整。复用 NormalCard 展示文件、FileDetailSidebar 查看文件详情。MaterialsPage 和 GameIntroPage 的 `onCardClick` 中 `is_dir` 分支改为打开此弹窗。

**Tech Stack:** Vue 3 Composition API, Tauri IPC (`scan_directory`), 现有 NormalCard + FileDetailSidebar 组件

---

## Task 1: 新建 FolderBrowserDialog 组件

**Files:**
- Create: `src/components/FolderBrowserDialog.vue`

**核心 Props/Emits:**
```ts
props: {
  show: boolean
  initialPath: string  // 初始文件夹路径
}
emits: {
  close: []
}
```

**内部状态：**
- `pathStack: string[]` — 路径栈，`pathStack[0]` = initialPath，`pathStack[pathStack.length-1]` = 当前目录
- `files: FileEntry[]` — 当前目录文件列表
- `selectedFile: FileEntry | null` — 选中文件（打开侧边栏）
- `dialogWidth / dialogHeight` — 弹窗尺寸（百分比，默认 70/70，localStorage 持久化）

**路径导航逻辑：**
- `currentPath` = computed → `pathStack[pathStack.length - 1]`
- `breadcrumbs` = computed → 从 `initialPath` 到 `currentPath` 的每层目录名+路径
- 点击子文件夹 → `pathStack.push(subDir.path)` + `loadFiles()`
- 点击面包屑某层 → `pathStack.splice(index + 1)` + `loadFiles()`
- watch `show` → true 时重置 `pathStack = [initialPath]` + `loadFiles()`

**拖拽调整尺寸：**
- 四条边 + 四个角共 8 个 resize handle（6px 热区，`position: absolute`）
- `mousedown` → 记录起始尺寸/位置 → `mousemove` 动态更新 `dialogWidth/dialogHeight`（最小 40%×40%，最大 95%×95%）
- `mouseup` → 持久化到 localStorage（key `pgb1-folder-browser-size`）

**Step 1: 创建组件骨架**

```vue
<script setup lang="ts">
import { ref, computed, watch, nextTick } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { useI18n } from 'vue-i18n'
import type { FileEntry } from '../composables/useDirectoryFiles'
import { useDirectoryFiles } from '../composables/useDirectoryFiles'
import NormalCard from './NormalCard.vue'
import FileDetailSidebar from './FileDetailSidebar.vue'

const props = defineProps<{
  show: boolean
  initialPath: string
}>()

const emit = defineEmits<{
  close: []
}>()

const { t } = useI18n()
const { openInExplorer } = useDirectoryFiles()

// ─── 路径栈 ───
const pathStack = ref<string[]>([])
const files = ref<FileEntry[]>([])
const loading = ref(false)

const currentPath = computed(() => pathStack.value[pathStack.value.length - 1] ?? '')

/** 面包屑：从 initialPath 开始的每层目录 */
const breadcrumbs = computed(() => {
  const initial = props.initialPath
  return pathStack.value.map((p, i) => ({
    label: i === 0 ? p.split('\\').pop()! : p.split('\\').pop()!,
    path: p,
    isLast: i === pathStack.value.length - 1,
  }))
})

async function loadCurrentDir() {
  const dir = currentPath.value
  if (!dir) return
  loading.value = true
  try {
    files.value = await invoke<FileEntry[]>('scan_directory', { dirPath: dir })
  } catch {
    files.value = []
  } finally {
    loading.value = false
  }
  // 重置选中
  selectedFile.value = null
}

function enterFolder(folder: FileEntry) {
  pathStack.value = [...pathStack.value, folder.path]
  loadCurrentDir()
}

function navigateTo(index: number) {
  if (index < pathStack.value.length - 1) {
    pathStack.value = pathStack.value.slice(0, index + 1)
    loadCurrentDir()
  }
}

function goBack() {
  if (pathStack.value.length > 1) {
    pathStack.value = pathStack.value.slice(0, -1)
    loadCurrentDir()
  }
}

// ─── 文件选中（侧边栏） ───
const selectedFile = ref<FileEntry | null>(null)

function onCardClick(file: FileEntry) {
  if (file.is_dir) {
    enterFolder(file)
    return
  }
  if (selectedFile.value?.path === file.path) {
    selectedFile.value = null
  } else {
    selectedFile.value = file
  }
}

function onMainClick(e: MouseEvent) {
  if (!(e.target as HTMLElement).closest('.normal-card')) {
    selectedFile.value = null
  }
}

// ─── 弹窗尺寸 ───
const STORAGE_KEY = 'pgb1-folder-browser-size'
const MIN_PCT = 40
const MAX_PCT = 95

function loadSavedSize(): { w: number; h: number } {
  try {
    const raw = localStorage.getItem(STORAGE_KEY)
    if (raw) {
      const { w, h } = JSON.parse(raw)
      return { w: clamp(w), h: clamp(h) }
    }
  } catch { /* 忽略 */ }
  return { w: 70, h: 75 }
}

function clamp(v: number) { return Math.max(MIN_PCT, Math.min(MAX_PCT, v)) }

const saved = loadSavedSize()
const dialogWidth = ref(saved.w)
const dialogHeight = ref(saved.h)

function saveSize() {
  localStorage.setItem(STORAGE_KEY, JSON.stringify({ w: dialogWidth.value, h: dialogHeight.value }))
}

/** 边缘拖拽调整尺寸 */
function onResizeStart(e: MouseEvent, edge: string) {
  e.preventDefault()
  const startX = e.clientX
  const startY = e.clientY
  const startW = dialogWidth.value
  const startH = dialogHeight.value
  const vw = window.innerWidth
  const vh = window.innerHeight

  function onMove(ev: MouseEvent) {
    const dx = ev.clientX - startX
    const dy = ev.clientY - startY

    if (edge.includes('e')) dialogWidth.value = clamp(startW + (dx / vw) * 100)
    if (edge.includes('w')) dialogWidth.value = clamp(startW - (dx / vw) * 100)
    if (edge.includes('s')) dialogHeight.value = clamp(startH + (dy / vh) * 100)
    if (edge.includes('n')) dialogHeight.value = clamp(startH - (dy / vh) * 100)
  }

  function onUp() {
    saveSize()
    window.removeEventListener('mousemove', onMove)
    window.removeEventListener('mouseup', onUp)
  }

  window.addEventListener('mousemove', onMove)
  window.addEventListener('mouseup', onUp)
}

// ─── show 变化时初始化 ───
watch(() => props.show, (v) => {
  if (v) {
    pathStack.value = [props.initialPath]
    selectedFile.value = null
    loadCurrentDir()
  }
})
</script>

<template>
  <Teleport to="body">
    <Transition name="dialog">
      <div v-if="show" class="fb-overlay" @mousedown.self="$emit('close')">
        <div
          class="fb-dialog glass-strong"
          :style="{ width: dialogWidth + 'vw', height: dialogHeight + 'vh' }"
          @click="onMainClick"
        >
          <!-- 拖拽调整尺寸手柄（8方向） -->
          <div class="resize-handle resize-n" @mousedown="onResizeStart($event, 'n')" />
          <div class="resize-handle resize-s" @mousedown="onResizeStart($event, 's')" />
          <div class="resize-handle resize-e" @mousedown="onResizeStart($event, 'e')" />
          <div class="resize-handle resize-w" @mousedown="onResizeStart($event, 'w')" />
          <div class="resize-handle resize-ne" @mousedown="onResizeStart($event, 'ne')" />
          <div class="resize-handle resize-nw" @mousedown="onResizeStart($event, 'nw')" />
          <div class="resize-handle resize-se" @mousedown="onResizeStart($event, 'se')" />
          <div class="resize-handle resize-sw" @mousedown="onResizeStart($event, 'sw')" />

          <!-- 顶部：面包屑导航 -->
          <div class="fb-header">
            <button
              class="fb-back-btn"
              :disabled="pathStack.length <= 1"
              @click="goBack"
            >
              <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                <polyline points="15 18 9 12 15 6" />
              </svg>
            </button>
            <div class="fb-breadcrumbs">
              <template v-for="(crumb, i) in breadcrumbs" :key="crumb.path">
                <span v-if="i > 0" class="fb-separator">/</span>
                <button
                  class="fb-crumb"
                  :class="{ active: crumb.isLast }"
                  :disabled="crumb.isLast"
                  @click="navigateTo(i)"
                >{{ crumb.label }}</button>
              </template>
            </div>
            <button class="fb-action-btn" :title="t('common.openFolder')" @click="openInExplorer(currentPath)">
              <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                <path d="M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6" />
                <polyline points="15 3 21 3 21 9" />
                <line x1="10" y1="14" x2="21" y2="3" />
              </svg>
            </button>
            <button class="fb-action-btn fb-close-btn" @click="$emit('close')">
              <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                <line x1="18" y1="6" x2="6" y2="18" />
                <line x1="6" y1="6" x2="18" y2="18" />
              </svg>
            </button>
          </div>

          <!-- 内容区：卡片网格 -->
          <div class="fb-body">
            <div class="fb-content">
              <p v-if="loading" class="fb-status">{{ t('common.scanning') }}</p>
              <p v-else-if="files.length === 0" class="fb-status">{{ t('folderBrowser.empty') }}</p>
              <TransitionGroup v-else name="card" tag="div" class="card-grid">
                <NormalCard
                  v-for="(file, i) in files"
                  :key="file.path"
                  :style="{ '--delay': i * 30 + 'ms' }"
                  :file="file"
                  :class="{ selected: selectedFile?.path === file.path }"
                  @click="onCardClick(file)"
                />
              </TransitionGroup>
            </div>

            <!-- 侧边栏（弹窗内嵌） -->
            <FileDetailSidebar
              :file="selectedFile"
              :width-percent="35"
              @close="selectedFile = null"
            />
          </div>
        </div>
      </div>
    </Transition>
  </Teleport>
</template>
```

**Step 2: 编写 scoped CSS**

弹窗样式要点：
- `.fb-overlay`：全屏遮罩，`position: fixed; inset: 0; z-index: var(--z-modal)`
- `.fb-dialog`：`glass-strong`，居中定位，`display: flex; flex-direction: column`
- `.fb-header`：固定顶部，面包屑横排 + 返回/打开/关闭按钮
- `.fb-body`：`flex: 1; overflow: hidden; display: flex`（左侧内容 + 右侧可选侧边栏）
- `.fb-content`：`flex: 1; overflow-y: auto`，内含 `.card-grid`
- 8 个 resize handle：`position: absolute`，对应方向 cursor

**Step 3: Commit**

```bash
git add src/components/FolderBrowserDialog.vue
git commit -m "feat: add FolderBrowserDialog component"
```

---

## Task 2: 添加 i18n 文本

**Files:**
- Modify: `src/locales/zh-CN.ts`
- Modify: `src/locales/en.ts`

**新增 namespace `folderBrowser`：**

zh-CN:
```ts
folderBrowser: {
  empty: '此文件夹为空',
},
```

en:
```ts
folderBrowser: {
  empty: 'This folder is empty',
},
```

**Commit:**
```bash
git add src/locales/zh-CN.ts src/locales/en.ts
git commit -m "feat: add folderBrowser i18n keys"
```

---

## Task 3: MaterialsPage 集成

**Files:**
- Modify: `src/views/MaterialsPage.vue`

**改动点：**

1. import FolderBrowserDialog
2. 新增两个 ref：
   ```ts
   const showFolderBrowser = ref(false)
   const folderBrowserPath = ref('')
   ```
3. 修改 `onCardClick` 的 `is_dir` 分支：
   ```ts
   if (file.is_dir) {
     folderBrowserPath.value = file.path
     showFolderBrowser.value = true
     return
   }
   ```
4. template 末尾加入组件：
   ```vue
   <FolderBrowserDialog
     :show="showFolderBrowser"
     :initial-path="folderBrowserPath"
     @close="showFolderBrowser = false"
   />
   ```

**Commit:**
```bash
git add src/views/MaterialsPage.vue
git commit -m "feat: integrate FolderBrowserDialog into MaterialsPage"
```

---

## Task 4: GameIntroPage 集成

**Files:**
- Modify: `src/views/GameIntroPage.vue`

**改动与 Task 3 完全相同的模式：**

1. import FolderBrowserDialog
2. 新增 `showFolderBrowser` + `folderBrowserPath` ref
3. 修改 `onCardClick` 的 `is_dir` 分支
4. template 加入 `<FolderBrowserDialog />`

**Commit:**
```bash
git add src/views/GameIntroPage.vue
git commit -m "feat: integrate FolderBrowserDialog into GameIntroPage"
```

---

## Task 5: 弹窗内 FileDetailSidebar 适配

**注意事项：**

FileDetailSidebar 当前通过 `Teleport to #content-row` 挂载。在弹窗内使用时，需要确认它能正常工作。两种可能：

**方案 a**（优先尝试）：在弹窗内放一个 `<div id="fb-content-row">` 作为 Teleport 目标，FileDetailSidebar 需要支持可选的 teleport target。

**方案 b**（更简单）：弹窗不使用 FileDetailSidebar，而是点击文件直接用 `open_file` 系统打开。考虑到这是辅助素材（非核心工作流），这个方案可能就够了。

> 实现时根据 FileDetailSidebar 的 Teleport 机制选择具体方案。如果 Teleport 目标硬编码为 `#content-row`，可以给 FolderBrowserDialog 的 body 区域加上相同的 id（但要注意 id 冲突），或者传 prop 覆盖。

**Commit:**
```bash
git commit -m "fix: adapt FileDetailSidebar for FolderBrowserDialog context"
```

---

## Task 6: 验证与收尾

- `npm run tauri dev` 启动应用
- 进入任意项目 → 项目素材页 → 找一个有子文件夹的分组 → 点击子文件夹卡片 → 验证弹窗打开
- 在弹窗内点击更深层子文件夹 → 验证递归进入
- 点击面包屑 → 验证跳回
- 拖拽弹窗边缘 → 验证尺寸调整
- 关闭弹窗重新打开 → 验证尺寸持久化
- 在弹窗内点击文件 → 验证侧边栏/文件预览
- GameIntroPage 重复同样验证
- 更新 CODE_INDEX.md

**Commit:**
```bash
git commit -m "docs: update CODE_INDEX for FolderBrowserDialog"
```
