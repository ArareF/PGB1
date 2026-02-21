# 框选多选 (Rubber Band Selection) 实现方案

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 在五个页面（TaskPage/GameIntroPage/MaterialsPage/ScalePage/ConvertPage）添加鼠标拖拽框选功能；游戏介绍和项目素材页同步补齐多选/全选按钮；缩放/转换页面本身处于多选状态，直接叠加框选。

**Architecture:** 新建 `useRubberBandSelect` composable 封装框选核心（mousedown→mousemove→mouseup，`position: fixed` 视口矩形 + `getBoundingClientRect` 碰撞检测）；`NormalCard` 补齐 `multiSelect/checked/data-path` 三件套对齐 `MaterialCard`；五个页面分两类接入：有多选开关的（TaskPage/GameIntroPage/MaterialsPage）以 `isMultiSelect` 为 gate，始终多选的（ScalePage/ConvertPage）直接 `isEnabled = ref(true)`。

**Tech Stack:** Vue 3 Composition API, TypeScript, CSS variables (design-system.css SSOT)

---

## 冲突分析

| 冲突点 | 受影响页面 | 解决方案 |
|--------|-----------|----------|
| 卡片 mousedown 拖拽导出 vs 框选 | Task/GameIntro/Materials | 框选 mousedown 检测 `e.target`，点在卡片上直接跳过 |
| 框选结束后 click 事件关闭侧边栏 | Task/GameIntro/Materials | composable 暴露 `justFinished` ref，click handler 里 early-return |
| 非多选模式下空白拖拽误触发 | Task/GameIntro/Materials | `isEnabled = isMultiSelect`，为 false 时直接 return |
| 滚动时框选起点失效 | 全部 | 监听容器 scroll 事件，触发时终止框选 |
| ScalePage / ConvertPage 无 scrollRef | Scale/Convert | 给 `.card-area` 加 `ref="cardAreaRef"` |
| MaterialsPage 卡片分散在多个 section | Materials | 容器是整个 `.scroll-content`，选择器 `.normal-card[data-path]` 仍可找到所有卡片 |
| 多选模式拖拽应批量导出 | GameIntro/Materials | 在 `onCardMouseDown` 内判断 `isMultiSelect`，合并 `selectedPaths` 一起拖 |

---

## Task 1：新建 `useRubberBandSelect.ts`

**Files:**
- Create: `src/composables/useRubberBandSelect.ts`

```typescript
import { ref } from 'vue'
import type { Ref } from 'vue'

export interface RubberBandRect {
  left: number
  top: number
  right: number
  bottom: number
}

export interface UseRubberBandOptions {
  /** 卡片所在的滚动容器 ref */
  containerRef: Ref<HTMLElement | null>
  /** 卡片元素 CSS 选择器，必须带 data-path 属性 */
  cardSelector: string
  /** 是否允许框选（false 时完全跳过） */
  isEnabled: Ref<boolean>
  /** 框选过程中命中集合变化时回调（传入当前命中的 path Set） */
  onSelect: (paths: Set<string>) => void
}

const MOVE_THRESHOLD = 5 // 像素，超过阈值才算"真正在框选"

export function useRubberBandSelect(options: UseRubberBandOptions) {
  const isSelecting = ref(false)
  const selectionRect = ref<RubberBandRect | null>(null)
  /** 框选刚结束标志：用于屏蔽随后触发的 click 事件 */
  const justFinished = ref(false)

  let startX = 0
  let startY = 0
  let didMove = false

  function onContainerMouseDown(e: MouseEvent) {
    if (!options.isEnabled.value) return
    if (e.button !== 0) return
    // 点在卡片上时跳过，让卡片自己的 mousedown 处理
    if ((e.target as HTMLElement).closest(options.cardSelector)) return

    startX = e.clientX
    startY = e.clientY
    didMove = false

    document.addEventListener('mousemove', onMouseMove)
    document.addEventListener('mouseup', onMouseUp)
  }

  function onMouseMove(e: MouseEvent) {
    const dx = e.clientX - startX
    const dy = e.clientY - startY

    if (!didMove) {
      if (Math.sqrt(dx * dx + dy * dy) < MOVE_THRESHOLD) return
      didMove = true
      isSelecting.value = true
    }

    const left = Math.min(startX, e.clientX)
    const top = Math.min(startY, e.clientY)
    const right = Math.max(startX, e.clientX)
    const bottom = Math.max(startY, e.clientY)
    selectionRect.value = { left, top, right, bottom }

    // 碰撞检测：遍历容器内所有卡片
    const container = options.containerRef.value
    if (!container) return
    const cards = container.querySelectorAll<HTMLElement>(options.cardSelector)
    const hit = new Set<string>()
    for (const card of cards) {
      const r = card.getBoundingClientRect()
      if (r.right > left && r.left < right && r.bottom > top && r.top < bottom) {
        const path = card.dataset.path
        if (path) hit.add(path)
      }
    }
    options.onSelect(hit)
  }

  function onMouseUp() {
    document.removeEventListener('mousemove', onMouseMove)
    document.removeEventListener('mouseup', onMouseUp)

    if (didMove) {
      justFinished.value = true
      // mouseup 后 click 事件同步触发，setTimeout(0) 保证 click handler 能读到 true 再重置
      setTimeout(() => { justFinished.value = false }, 0)
    }

    isSelecting.value = false
    selectionRect.value = null
    didMove = false
  }

  /** 供容器绑定：滚动时立即终止框选，防止起点失效 */
  function onContainerScroll() {
    if (isSelecting.value) onMouseUp()
  }

  return { isSelecting, selectionRect, justFinished, onContainerMouseDown, onContainerScroll }
}
```

---

## Task 2：CSS — 框选矩形 + 通用多选复选框工具类

**Files:**
- Modify: `src/styles/design-system.css`（追加到文件末尾）

```css
/* ─── 框选覆盖层 ────────────────────────────────────────── */
.rubber-band-overlay {
  position: fixed;
  pointer-events: none;
  border: 1px solid var(--color-primary-400);
  background: color-mix(in srgb, var(--color-primary) 8%, transparent);
  border-radius: var(--radius-sm);
  z-index: 300;
}

/* ─── 通用多选复选框（MaterialCard / NormalCard 共用） ──── */
.card-checkbox-shared {
  position: absolute;
  top: var(--spacing-2);
  left: var(--spacing-2);
  width: 24px;
  height: 24px;
  border-radius: var(--radius-sm);
  border: 2px solid var(--border-medium);
  background: var(--glass-subtle-bg);
  backdrop-filter: blur(var(--glass-subtle-blur));
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 2;
  transition: all var(--transition-fast);
}

.card-checkbox-shared.checked {
  background: var(--color-primary);
  border-color: var(--color-primary);
  color: var(--color-neutral-0);
}
```

> **注意**：MaterialCard 现有的 `.card-checkbox` 和 `.card-checkbox.checked` 样式与上面完全等价——待这两个类迁移到 design-system.css 后，MaterialCard 的 scoped style 中对应规则可删除（可选优化，不在本 task 范围内）。

---

## Task 3：NormalCard.vue — 补齐多选三件套

**Files:**
- Modify: `src/components/NormalCard.vue`

### Step 1：Props 定义（script 区）

在 `defineProps` 中增加两个可选 prop：
```typescript
// 修改前
defineProps<{
  file: FileEntry
}>()

// 修改后
defineProps<{
  file: FileEntry
  multiSelect?: boolean
  checked?: boolean
}>()
```

### Step 2：根元素加 `data-path` + `position: relative`

```html
<!-- 修改前 -->
<button
  class="normal-card glass-subtle"
  @click="$emit('click', file)"
>

<!-- 修改后 -->
<button
  class="normal-card glass-subtle"
  :data-path="file.path"
  @click="$emit('click', file)"
>
```

CSS：
```css
/* 修改前 */
.normal-card {
  width: var(--card-material-width);
  ...
}

/* 修改后：加 position: relative */
.normal-card {
  position: relative;
  width: var(--card-material-width);
  ...
}
```

### Step 3：在 preview-wrapper 前插入复选框

```html
<!-- 在 <div class="preview-wrapper"> 之前插入 -->
<span
  v-if="multiSelect"
  class="card-checkbox-shared"
  :class="{ checked }"
>
  <svg v-if="checked" width="14" height="14" viewBox="0 0 24 24" fill="currentColor">
    <path d="M9 16.17L4.83 12l-1.42 1.41L9 19 21 7l-1.41-1.41z" />
  </svg>
</span>
```

### Step 4：多选选中态描边

```css
/* 追加到 NormalCard scoped style */
.normal-card.multi-checked {
  outline: 2px solid var(--color-primary);
  outline-offset: -2px;
}
```

---

## Task 4：TaskPage.vue — 接入框选

**Files:**
- Modify: `src/views/TaskPage.vue`

### script setup 区：引入 composable

在 `import` 区域末尾加：
```typescript
import { useRubberBandSelect } from '../composables/useRubberBandSelect'
```

在 `/** 多选模式状态 */` 块之后、`/** 拖拽上传 */` 之前加：

```typescript
// ─── 框选多选 ──────────────────────────────────────
const { isSelecting, selectionRect, justFinished, onContainerMouseDown, onContainerScroll } =
  useRubberBandSelect({
    containerRef: scrollRef,
    cardSelector: '.material-card[data-path]',
    isEnabled: isMultiSelect,
    onSelect: (paths) => {
      // 框选期间实时替换选中集合
      selectedPaths.value = paths
    },
  })
```

### 修改 `onMainContentClick`：拦截框选后的 click

```typescript
// 修改前
function onMainContentClick(e: MouseEvent) {
  const target = e.target as HTMLElement
  if (target.closest('.material-card')) return
  if (target.closest('.preview-video-card')) return
  closeSidebar()
  selectedPreviewVideo.value = null
  selectedPreviewGroup.value = null
}

// 修改后
function onMainContentClick(e: MouseEvent) {
  if (justFinished.value) return  // 框选刚结束，跳过本次 click
  const target = e.target as HTMLElement
  if (target.closest('.material-card')) return
  if (target.closest('.preview-video-card')) return
  closeSidebar()
  selectedPreviewVideo.value = null
  selectedPreviewGroup.value = null
}
```

### template 区：给 scroll-content 绑定事件

```html
<!-- 修改前 -->
<div ref="scrollRef" class="scroll-content">

<!-- 修改后 -->
<div
  ref="scrollRef"
  class="scroll-content"
  @mousedown="onContainerMouseDown"
  @scroll="onContainerScroll"
>
```

### template 区：在 `</template>` 前添加框选覆盖层

```html
<!-- 框选矩形覆盖层 -->
<Teleport to="body">
  <div
    v-if="isSelecting && selectionRect"
    class="rubber-band-overlay"
    :style="{
      left: selectionRect.left + 'px',
      top: selectionRect.top + 'px',
      width: (selectionRect.right - selectionRect.left) + 'px',
      height: (selectionRect.bottom - selectionRect.top) + 'px',
    }"
  />
</Teleport>
```

---

## Task 5：GameIntroPage.vue — 多选开关 + 框选

**Files:**
- Modify: `src/views/GameIntroPage.vue`

### Step 1：script — 新增多选状态 + scrollRef + 框选

```typescript
// 在 selectedFile、sidebarWidth 定义后追加：
import { useRubberBandSelect } from '../composables/useRubberBandSelect'

const scrollRef = ref<HTMLElement | null>(null)
const isMultiSelect = ref(false)
const selectedPaths = ref<Set<string>>(new Set())

const isAllSelected = computed(() =>
  files.value.filter(f => !f.is_dir).length > 0 &&
  files.value.filter(f => !f.is_dir).every(f => selectedPaths.value.has(f.path))
)

function toggleMultiSelect() {
  if (isMultiSelect.value) {
    isMultiSelect.value = false
    selectedPaths.value = new Set()
  } else {
    isMultiSelect.value = true
    selectedFile.value = null  // 进入多选时关闭侧边栏
  }
}

function toggleSelectAll() {
  const fileOnly = files.value.filter(f => !f.is_dir)
  if (isAllSelected.value) {
    selectedPaths.value = new Set()
  } else {
    selectedPaths.value = new Set(fileOnly.map(f => f.path))
  }
}

function toggleFileSelection(file: FileEntry) {
  const newSet = new Set(selectedPaths.value)
  if (newSet.has(file.path)) {
    newSet.delete(file.path)
  } else {
    newSet.add(file.path)
  }
  selectedPaths.value = newSet
}

const { isSelecting, selectionRect, justFinished, onContainerMouseDown, onContainerScroll } =
  useRubberBandSelect({
    containerRef: scrollRef,
    cardSelector: '.normal-card[data-path]',
    isEnabled: isMultiSelect,
    onSelect: (paths) => { selectedPaths.value = paths },
  })
```

### Step 2：修改 `onCardClick`

```typescript
// 修改前
function onCardClick(file: FileEntry) {
  if (file.is_dir) {
    openInExplorer(file.path)
    return
  }
  if (selectedFile.value?.path === file.path) {
    selectedFile.value = null
  } else {
    selectedFile.value = file
  }
}

// 修改后
function onCardClick(file: FileEntry) {
  if (file.is_dir) {
    openInExplorer(file.path)
    return
  }
  if (isMultiSelect.value) {
    toggleFileSelection(file)
    return
  }
  if (selectedFile.value?.path === file.path) {
    selectedFile.value = null
  } else {
    selectedFile.value = file
  }
}
```

### Step 3：修改 `onMainClick`（拦截框选后的 click）

```typescript
// 修改后
function onMainClick(e: MouseEvent) {
  if (justFinished.value) return
  if (!(e.target as HTMLElement).closest('.normal-card')) {
    selectedFile.value = null
  }
}
```

### Step 4：修改 `onCardMouseDown`（多选模式批量拖拽）

```typescript
// 修改后
function onCardMouseDown(e: MouseEvent, file: FileEntry) {
  if (e.button !== 0 || file.is_dir) return

  const startX = e.clientX
  const startY = e.clientY
  let dragStarted = false

  function onMouseMove(ev: MouseEvent) {
    if (dragStarted) return
    const dx = ev.clientX - startX
    const dy = ev.clientY - startY
    if (Math.sqrt(dx * dx + dy * dy) > DRAG_THRESHOLD) {
      dragStarted = true
      cleanup()
      if (isMultiSelect.value) {
        // 确保被拖卡片在选中集合中
        if (!selectedPaths.value.has(file.path)) {
          const newSet = new Set(selectedPaths.value)
          newSet.add(file.path)
          selectedPaths.value = newSet
        }
        const paths = [...selectedPaths.value]
        if (paths.length > 0) {
          startDrag({ item: paths, icon: '' }).catch(err => console.error('拖拽失败:', err))
        }
      } else {
        startDrag({ item: [file.path], icon: '' }).catch(err => console.error('拖拽失败:', err))
      }
    }
  }

  function onMouseUp() { cleanup() }
  function cleanup() {
    document.removeEventListener('mousemove', onMouseMove)
    document.removeEventListener('mouseup', onMouseUp)
  }
  document.addEventListener('mousemove', onMouseMove)
  document.addEventListener('mouseup', onMouseUp)
}
```

### Step 5：template — 小标题栏加多选/全选按钮

```html
<!-- 修改前 -->
<div class="view-buttons">
  <button class="view-btn" @click="() => { if (dirPath) loadFiles(dirPath) }">刷新</button>
</div>

<!-- 修改后 -->
<div class="view-buttons">
  <button class="view-btn" @click="() => { if (dirPath) loadFiles(dirPath) }">刷新</button>
  <button
    class="view-btn"
    :class="{ active: isMultiSelect }"
    @click="toggleMultiSelect"
  >
    {{ isMultiSelect ? '多选 ✓' : '多选' }}
  </button>
  <button
    v-if="isMultiSelect"
    class="view-btn"
    @click="toggleSelectAll"
  >
    {{ isAllSelected ? '取消全选' : '全选' }}
  </button>
</div>
```

### Step 6：template — scroll-content 加 ref + 事件，NormalCard 加多选 props

```html
<!-- scroll-content -->
<div
  ref="scrollRef"
  class="scroll-content"
  @mousedown="onContainerMouseDown"
  @scroll="onContainerScroll"
>
```

```html
<!-- NormalCard：加 multiSelect/checked/class -->
<NormalCard
  v-for="(file, i) in files"
  :key="file.name"
  :style="{ '--delay': i * 40 + 'ms' }"
  :file="file"
  :multi-select="isMultiSelect"
  :checked="selectedPaths.has(file.path)"
  :class="{
    selected: !isMultiSelect && selectedFile?.path === file.path,
    'multi-checked': isMultiSelect && selectedPaths.has(file.path),
  }"
  @click="onCardClick(file)"
  @mousedown="onCardMouseDown($event, file)"
/>
```

### Step 7：template — 框选覆盖层（在 `</template>` 前）

```html
<Teleport to="body">
  <div
    v-if="isSelecting && selectionRect"
    class="rubber-band-overlay"
    :style="{
      left: selectionRect.left + 'px',
      top: selectionRect.top + 'px',
      width: (selectionRect.right - selectionRect.left) + 'px',
      height: (selectionRect.bottom - selectionRect.top) + 'px',
    }"
  />
</Teleport>
```

### Step 8：style — 补充 `.view-btn.active`（如 design-system.css 已有则跳过）

```css
.view-btn.active {
  background: var(--bg-active);
  color: var(--text-primary);
  border-color: var(--border-strong);
}
```

---

## Task 6：MaterialsPage.vue — 多选开关 + 框选

**Files:**
- Modify: `src/views/MaterialsPage.vue`

> MaterialsPage 的变更内容与 GameIntroPage 几乎一致，主要差异是卡片来源是 `groups[]`（多分组）而非 `files[]`，以及 `allFiles` 的计算。

### Step 1：script — 新增多选状态 + scrollRef + 框选

```typescript
import { useRubberBandSelect } from '../composables/useRubberBandSelect'

const scrollRef = ref<HTMLElement | null>(null)
const isMultiSelect = ref(false)
const selectedPaths = ref<Set<string>>(new Set())

// 从所有分组中收集所有文件（用于全选）
const allFiles = computed(() => {
  const result: FileEntry[] = []
  for (const g of groups.value) {
    result.push(...g.files.filter(f => !f.is_dir))
    if (g.subGroups) {
      for (const sg of g.subGroups) {
        result.push(...sg.files.filter(f => !f.is_dir))
      }
    }
  }
  return result
})

const isAllSelected = computed(() =>
  allFiles.value.length > 0 && allFiles.value.every(f => selectedPaths.value.has(f.path))
)

function toggleMultiSelect() {
  if (isMultiSelect.value) {
    isMultiSelect.value = false
    selectedPaths.value = new Set()
  } else {
    isMultiSelect.value = true
    selectedFile.value = null
  }
}

function toggleSelectAll() {
  if (isAllSelected.value) {
    selectedPaths.value = new Set()
  } else {
    selectedPaths.value = new Set(allFiles.value.map(f => f.path))
  }
}

function toggleFileSelection(file: FileEntry) {
  const newSet = new Set(selectedPaths.value)
  if (newSet.has(file.path)) {
    newSet.delete(file.path)
  } else {
    newSet.add(file.path)
  }
  selectedPaths.value = newSet
}

const { isSelecting, selectionRect, justFinished, onContainerMouseDown, onContainerScroll } =
  useRubberBandSelect({
    containerRef: scrollRef,
    cardSelector: '.normal-card[data-path]',
    isEnabled: isMultiSelect,
    onSelect: (paths) => { selectedPaths.value = paths },
  })
```

### Step 2：修改 `onCardClick`

与 GameIntroPage 相同逻辑，多选模式下调用 `toggleFileSelection(file)`。

### Step 3：修改 `onMainClick`

```typescript
function onMainClick(e: MouseEvent) {
  if (justFinished.value) return
  if (!(e.target as HTMLElement).closest('.normal-card')) {
    selectedFile.value = null
  }
}
```

### Step 4：修改 `onCardMouseDown`（多选模式批量拖拽）

与 GameIntroPage 完全相同逻辑。

### Step 5：template — 小标题栏加多选/全选按钮

```html
<div class="view-buttons">
  <button class="view-btn" @click="refreshAll">刷新</button>
  <button
    class="view-btn"
    :class="{ active: isMultiSelect }"
    @click="toggleMultiSelect"
  >
    {{ isMultiSelect ? '多选 ✓' : '多选' }}
  </button>
  <button
    v-if="isMultiSelect"
    class="view-btn"
    @click="toggleSelectAll"
  >
    {{ isAllSelected ? '取消全选' : '全选' }}
  </button>
</div>
```

### Step 6：template — scroll-content 加 ref + 事件

```html
<div
  ref="scrollRef"
  class="scroll-content"
  @mousedown="onContainerMouseDown"
  @scroll="onContainerScroll"
>
```

### Step 7：template — NormalCard 加多选 props

所有出现 `<NormalCard` 的地方（根级 files 和 subGroups files）均加：
```html
:multi-select="isMultiSelect"
:checked="selectedPaths.has(file.path)"
:class="{
  selected: !isMultiSelect && selectedFile?.path === file.path,
  'multi-checked': isMultiSelect && selectedPaths.has(file.path),
}"
```
点击由 `@click="onCardClick(file)"` 统一处理，无需额外改动。

### Step 8：template — 框选覆盖层（在 `</template>` 前）

与 GameIntroPage 相同的 Teleport 块。

---

## Task 7：ScalePage.vue — 直接接入框选（始终开启）

**Files:**
- Modify: `src/views/ScalePage.vue`

ScalePage 本身已处于多选模式（`:multi-select="true"`），只需叠加框选。

### script 区

```typescript
import { ref } from 'vue'  // 已有，只需追加以下内容
import { useRubberBandSelect } from '../composables/useRubberBandSelect'

const cardAreaRef = ref<HTMLElement | null>(null)
const alwaysEnabled = ref(true)

const { isSelecting, selectionRect, onContainerMouseDown, onContainerScroll } =
  useRubberBandSelect({
    containerRef: cardAreaRef,
    cardSelector: '.material-card[data-path]',
    isEnabled: alwaysEnabled,
    onSelect: (paths) => {
      // 框选期间实时替换选中集合
      selectedPaths.value = paths
    },
  })
```

### template 区：`.card-area` 加 ref + 事件

```html
<!-- 修改前 -->
<div class="card-area">

<!-- 修改后 -->
<div
  ref="cardAreaRef"
  class="card-area"
  @mousedown="onContainerMouseDown"
  @scroll="onContainerScroll"
>
```

### template 区：框选覆盖层（在 `</template>` 前）

```html
<Teleport to="body">
  <div
    v-if="isSelecting && selectionRect"
    class="rubber-band-overlay"
    :style="{
      left: selectionRect.left + 'px',
      top: selectionRect.top + 'px',
      width: (selectionRect.right - selectionRect.left) + 'px',
      height: (selectionRect.bottom - selectionRect.top) + 'px',
    }"
  />
</Teleport>
```

> ScalePage 没有 `onMainContentClick` 类的 click handler，无需处理 `justFinished`。

---

## Task 8：ConvertPage.vue — 直接接入框选（始终开启）

**Files:**
- Modify: `src/views/ConvertPage.vue`

与 ScalePage 完全对称：

### script 区

```typescript
import { useRubberBandSelect } from '../composables/useRubberBandSelect'

const cardAreaRef = ref<HTMLElement | null>(null)
const alwaysEnabled = ref(true)

const { isSelecting, selectionRect, onContainerMouseDown, onContainerScroll } =
  useRubberBandSelect({
    containerRef: cardAreaRef,
    cardSelector: '.material-card[data-path]',
    isEnabled: alwaysEnabled,
    onSelect: (paths) => {
      selectedPaths.value = paths
    },
  })
```

### template 区：`.card-area` 加 ref + 事件

```html
<!-- 修改前 -->
<div class="card-area custom-scroll">

<!-- 修改后 -->
<div
  ref="cardAreaRef"
  class="card-area custom-scroll"
  @mousedown="onContainerMouseDown"
  @scroll="onContainerScroll"
>
```

### template 区：框选覆盖层（与 ScalePage 相同）

---

## Task 9：更新 CODE_INDEX.md

**Files:**
- Modify: `CODE_INDEX.md`

### Composables 表格新增一行

```
| `useRubberBandSelect.ts` | ~75 | `useRubberBandSelect()` | 框选多选逻辑。mousedown（空白区域）→ mousemove（视口矩形 + data-path 碰撞）→ onSelect 回调。`justFinished` ref 屏蔽框选后 click 事件。`onContainerScroll` 终止框选防止起点失效 |
```

### 各页面描述更新

- `NormalCard.vue`：追加 `multiSelect?, checked? props + data-path + card-checkbox-shared`
- `TaskPage.vue`：追加 `useRubberBandSelect 集成（isEnabled=isMultiSelect）`
- `GameIntroPage.vue`：追加 `多选开关 + 全选 + useRubberBandSelect（isEnabled=isMultiSelect）+ 多选批量拖拽`
- `MaterialsPage.vue`：追加 `多选开关 + 全选（跨 group/subGroup 收集 allFiles）+ useRubberBandSelect`
- `ScalePage.vue`：追加 `useRubberBandSelect（isEnabled=ref(true)，始终开启）`
- `ConvertPage.vue`：追加 `useRubberBandSelect（isEnabled=ref(true)，始终开启）`

---

## ✅ 验收清单

### 通用（所有页面）
- [ ] 框选矩形视觉符合 design-system 变量（无硬编码颜色）
- [ ] 框选过程中松开鼠标矩形消失，卡片选中状态保留
- [ ] 框选途中触发滚动 → 框选立即终止，不出现残影
- [ ] 卡片 mousedown 不触发框选（由卡片自己处理）

### TaskPage
- [ ] 非多选模式：空白拖拽无任何反应
- [ ] 多选模式：空白拖拽出现框选矩形，命中卡片实时高亮
- [ ] 框选后点击空白不关闭侧边栏

### GameIntroPage / MaterialsPage
- [ ] 小标题栏出现「多选」按钮（与 TaskPage 视觉一致）
- [ ] 进入多选后侧边栏关闭
- [ ] 多选模式点击卡片切换选中，不打开侧边栏
- [ ] 多选模式拖拽任意选中卡片 → 全部选中文件一起拖出
- [ ] MaterialsPage 框选可跨 section 选中多个分组的卡片

### ScalePage / ConvertPage
- [ ] 直接可框选，无需点击任何开关按钮
- [ ] 框选和点击切换选中行为一致（均写入同一 `selectedPaths`）
