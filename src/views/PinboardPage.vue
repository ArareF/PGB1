<script setup lang="ts">
import { ref, computed, onMounted, onBeforeUnmount, nextTick } from 'vue'
import { useI18n } from 'vue-i18n'
import { listen } from '@tauri-apps/api/event'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { usePinboard } from '../composables/usePinboard'
import PinboardCanvas from '../components/PinboardCanvas.vue'
import type { PinInfo, PinAnnotation, PinboardViewport } from '../composables/usePinboard'

const { t } = useI18n()

// ─── 标签系统 ────────────────────────────────────────────
interface PinboardTab {
  dirPath: string
  canvasKey: string
  title: string
}

const tabs = ref<PinboardTab[]>([])
const activeTabIndex = ref(-1)

const activeTab = computed(() =>
  activeTabIndex.value >= 0 && activeTabIndex.value < tabs.value.length
    ? tabs.value[activeTabIndex.value]
    : null
)

function openTab(tab: PinboardTab) {
  // 已存在则切换
  const existingIndex = tabs.value.findIndex(
    t => t.dirPath === tab.dirPath && t.canvasKey === tab.canvasKey
  )
  if (existingIndex >= 0) {
    switchTab(existingIndex)
    return
  }
  // 新增标签
  tabs.value.push(tab)
  switchTab(tabs.value.length - 1)
}

function closeTab(index: number) {
  // 切换前先保存当前画布
  if (index === activeTabIndex.value) {
    savePinboard()
  }
  tabs.value.splice(index, 1)
  if (tabs.value.length === 0) {
    activeTabIndex.value = -1
    // 全部关闭后关闭窗口
    getCurrentWindow().close()
    return
  }
  if (activeTabIndex.value >= tabs.value.length) {
    activeTabIndex.value = tabs.value.length - 1
  }
  // 加载新激活标签的数据
  nextTick(() => loadPinboard())
}

function switchTab(index: number) {
  if (index === activeTabIndex.value) return
  // 保存当前画布
  if (activeTabIndex.value >= 0) {
    savePinboard()
  }
  activeTabIndex.value = index
  // 重置状态
  selectedPinId.value = null
  activeTool.value = 'select'
  undoStacks.value.clear()
  redoStacks.value.clear()
  canvasUndoStack.value = []
  canvasRedoStack.value = []
  nextTick(() => loadPinboard())
}

// ─── 贴图板数据 ────────────────────────────────────────
const activeDirPath = computed(() => activeTab.value?.dirPath ?? '')
const activeCanvasKey = computed(() => activeTab.value?.canvasKey ?? '')

const {
  pins,
  canvasAnnotations,
  viewport,
  loadPinboard,
  savePinboard,
  pasteImage,
  deletePin,
  updatePin,
  bringToFront,
  getPinImageUrl,
} = usePinboard(activeDirPath, activeCanvasKey)

// ─── 工具 & 颜色 & 尺寸 ─────────────────────────────────
type ToolType = 'select' | 'pen' | 'arrow' | 'rect' | 'ellipse' | 'text' | 'eraser'
const activeTool = ref<ToolType>('select')
const COLORS = ['#FF3B30', '#007AFF', '#34C759', '#FF9500', '#FFFFFF'] as const
const activeColor = ref<string>(COLORS[0])
const selectedPinId = ref<string | null>(null)

// 各工具独立记忆的尺寸
const penSize = ref(3)
const eraserSize = ref(20)
const textSize = ref(16)

const TOOLS_WITH_SIZE = ['pen', 'arrow', 'rect', 'ellipse', 'eraser', 'text'] as const
const showSizeSlider = computed(() => TOOLS_WITH_SIZE.includes(activeTool.value as any))

const currentSizeValue = computed(() => {
  if (activeTool.value === 'eraser') return eraserSize.value
  if (activeTool.value === 'text') return textSize.value
  return penSize.value
})
const currentSizeMin = computed(() => activeTool.value === 'eraser' ? 5 : activeTool.value === 'text' ? 10 : 1)
const currentSizeMax = computed(() => activeTool.value === 'eraser' ? 50 : activeTool.value === 'text' ? 48 : 20)

function onSizeChange(e: Event) {
  const val = +(e.target as HTMLInputElement).value
  if (activeTool.value === 'eraser') eraserSize.value = val
  else if (activeTool.value === 'text') textSize.value = val
  else penSize.value = val
}

const currentStrokeSize = computed(() =>
  activeTool.value === 'eraser' ? eraserSize.value : penSize.value
)
const currentFontSize = computed(() => textSize.value)

const canvasRef = ref<InstanceType<typeof PinboardCanvas> | null>(null)

// ─── 撤销/重做（贴图级） ──────────────────────────────
const undoStacks = ref(new Map<string, PinAnnotation[][]>())
const redoStacks = ref(new Map<string, PinAnnotation[][]>())

function pushUndo(pinId: string) {
  const pin = pins.value.find(p => p.id === pinId)
  if (!pin) return
  const stack = undoStacks.value.get(pinId) ?? []
  stack.push(JSON.parse(JSON.stringify(pin.annotations)))
  undoStacks.value.set(pinId, stack)
  redoStacks.value.set(pinId, [])
}

function onAddAnnotation(pinId: string, ann: PinAnnotation) {
  pushUndo(pinId)
  const pin = pins.value.find(p => p.id === pinId)
  if (pin) {
    pin.annotations.push(ann)
    savePinboard()
  }
}

function onRemoveAnnotation(pinId: string, index: number) {
  pushUndo(pinId)
  const pin = pins.value.find(p => p.id === pinId)
  if (pin && index >= 0 && index < pin.annotations.length) {
    pin.annotations.splice(index, 1)
    savePinboard()
  }
}

// ─── 撤销/重做（画布级） ──────────────────────────────
const canvasUndoStack = ref<PinAnnotation[][]>([])
const canvasRedoStack = ref<PinAnnotation[][]>([])

function onAddCanvasAnnotation(ann: PinAnnotation) {
  canvasUndoStack.value.push(JSON.parse(JSON.stringify(canvasAnnotations.value)))
  canvasRedoStack.value = []
  canvasAnnotations.value.push(ann)
  savePinboard()
}

function onRemoveCanvasAnnotation(index: number) {
  canvasUndoStack.value.push(JSON.parse(JSON.stringify(canvasAnnotations.value)))
  canvasRedoStack.value = []
  canvasAnnotations.value.splice(index, 1)
  savePinboard()
}

// ─── 撤销/重做统一入口 ────────────────────────────────
function undo() {
  if (selectedPinId.value) {
    const pinId = selectedPinId.value
    const stack = undoStacks.value.get(pinId)
    if (!stack || stack.length === 0) return
    const pin = pins.value.find(p => p.id === pinId)
    if (!pin) return
    const redoStack = redoStacks.value.get(pinId) ?? []
    redoStack.push(JSON.parse(JSON.stringify(pin.annotations)))
    redoStacks.value.set(pinId, redoStack)
    pin.annotations = stack.pop()!
  } else {
    if (canvasUndoStack.value.length === 0) return
    canvasRedoStack.value.push(JSON.parse(JSON.stringify(canvasAnnotations.value)))
    canvasAnnotations.value = canvasUndoStack.value.pop()!
  }
  savePinboard()
}

function redo() {
  if (selectedPinId.value) {
    const pinId = selectedPinId.value
    const stack = redoStacks.value.get(pinId)
    if (!stack || stack.length === 0) return
    const pin = pins.value.find(p => p.id === pinId)
    if (!pin) return
    const undoStack = undoStacks.value.get(pinId) ?? []
    undoStack.push(JSON.parse(JSON.stringify(pin.annotations)))
    undoStacks.value.set(pinId, undoStack)
    pin.annotations = stack.pop()!
  } else {
    if (canvasRedoStack.value.length === 0) return
    canvasUndoStack.value.push(JSON.parse(JSON.stringify(canvasAnnotations.value)))
    canvasAnnotations.value = canvasRedoStack.value.pop()!
  }
  savePinboard()
}

const canUndo = computed(() => {
  if (selectedPinId.value) {
    const stack = undoStacks.value.get(selectedPinId.value)
    return !!stack && stack.length > 0
  }
  return canvasUndoStack.value.length > 0
})

const canRedo = computed(() => {
  if (selectedPinId.value) {
    const stack = redoStacks.value.get(selectedPinId.value)
    return !!stack && stack.length > 0
  }
  return canvasRedoStack.value.length > 0
})

// ─── Pin 操作 ─────────────────────────────────────────
function onSelectPin(pinId: string) {
  selectedPinId.value = pinId
  bringToFront(pinId)
}

function onUpdatePin(pinId: string, updates: Partial<PinInfo>) {
  updatePin(pinId, updates)
  savePinboard()
}

async function onDeletePin(pinId: string) {
  await deletePin(pinId)
  if (selectedPinId.value === pinId) {
    selectedPinId.value = null
  }
}

function onDeselect() {
  selectedPinId.value = null
}

function onUpdateViewport(vp: PinboardViewport) {
  viewport.value = vp
}

// ─── 粘贴 ─────────────────────────────────────────────
async function handlePaste() {
  if (!activeTab.value) return
  const pin = await pasteImage()
  if (pin) {
    selectedPinId.value = pin.id
  }
}

// ─── 缩放按钮 ─────────────────────────────────────────
function zoomIn() {
  viewport.value = {
    ...viewport.value,
    zoom: Math.min(5, viewport.value.zoom * 1.2),
  }
}

function zoomOut() {
  viewport.value = {
    ...viewport.value,
    zoom: Math.max(0.1, viewport.value.zoom / 1.2),
  }
}

const zoomLabel = computed(() => Math.round(viewport.value.zoom * 100) + '%')

// ─── 键盘快捷键 ───────────────────────────────────────
function onKeyDown(e: KeyboardEvent) {
  canvasRef.value?.onKeyDown(e)

  if (e.ctrlKey && e.key === 'v') {
    e.preventDefault()
    handlePaste()
  } else if (e.ctrlKey && (e.altKey || e.shiftKey) && e.key.toLowerCase() === 'z') {
    e.preventDefault()
    redo()
  } else if (e.ctrlKey && !e.altKey && !e.shiftKey && e.key === 'z') {
    e.preventDefault()
    undo()
  } else if (e.key === 'Delete' && selectedPinId.value) {
    e.preventDefault()
    onDeletePin(selectedPinId.value)
  }
}

function onKeyUp(e: KeyboardEvent) {
  canvasRef.value?.onKeyUp(e)
}

// ─── 窗口控制 ─────────────────────────────────────────
function minimizeWindow() {
  getCurrentWindow().minimize()
}

function closeWindow() {
  getCurrentWindow().close()
}

// ─── 事件监听 ─────────────────────────────────────────
let unlistenOpenTab: (() => void) | null = null

onMounted(async () => {
  window.addEventListener('keydown', onKeyDown)
  window.addEventListener('keyup', onKeyUp)

  // 从 URL query params 读取初始标签（首次创建窗口时 Rust 编码到 URL）
  const params = new URLSearchParams(window.location.search)
  const initDirPath = params.get('dirPath')
  const initCanvasKey = params.get('canvasKey')
  const initTitle = params.get('title')
  if (initDirPath && initCanvasKey && initTitle) {
    openTab({ dirPath: initDirPath, canvasKey: initCanvasKey, title: initTitle })
  }

  // 监听后续 open-tab 事件（窗口已存在时 Rust 用 win.emit 发送）
  unlistenOpenTab = await listen<PinboardTab>('pinboard-open-tab', (event) => {
    openTab(event.payload)
  })
})

onBeforeUnmount(() => {
  window.removeEventListener('keydown', onKeyDown)
  window.removeEventListener('keyup', onKeyUp)
  if (unlistenOpenTab) {
    unlistenOpenTab()
    unlistenOpenTab = null
  }
})
</script>

<template>
  <div class="pinboard-page">
    <!-- 标签栏 + 窗口控制 -->
    <div class="pb-tab-bar" data-tauri-drag-region>
      <div class="pb-tabs">
        <div
          v-for="(tab, i) in tabs"
          :key="tab.dirPath + ':' + tab.canvasKey"
          class="pb-tab"
          :class="{ active: i === activeTabIndex }"
          @mousedown="switchTab(i)"
        >
          <span class="pb-tab-title">{{ tab.title }}</span>
          <button
            class="pb-tab-close"
            :title="t('pinboard.closeTab')"
            @mousedown.stop="closeTab(i)"
          >
            <svg width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3" stroke-linecap="round">
              <line x1="18" y1="6" x2="6" y2="18" /><line x1="6" y1="6" x2="18" y2="18" />
            </svg>
          </button>
        </div>
      </div>
      <div class="pb-drag-spacer" data-tauri-drag-region />
      <div class="pb-window-controls">
        <button class="pb-win-btn" @click="minimizeWindow">
          <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round">
            <line x1="4" y1="12" x2="20" y2="12" />
          </svg>
        </button>
        <button class="pb-win-btn pb-win-close" @click="closeWindow">
          <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round">
            <line x1="18" y1="6" x2="6" y2="18" /><line x1="6" y1="6" x2="18" y2="18" />
          </svg>
        </button>
      </div>
    </div>

    <!-- 工具栏（有激活标签时显示） -->
    <div v-if="activeTab" class="pb-toolbar">
      <!-- 粘贴 -->
      <button class="pb-tool-btn" :title="t('pinboard.pasteHint')" @click="handlePaste">
        <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <path d="M16 4h2a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V6a2 2 0 0 1 2-2h2" />
          <rect x="8" y="2" width="8" height="4" rx="1" ry="1" />
        </svg>
      </button>

      <div class="pb-separator" />

      <!-- 工具组 -->
      <button
        class="pb-tool-btn"
        :class="{ active: activeTool === 'select' }"
        :title="t('pinboard.tools.select')"
        @click="activeTool = 'select'"
      >
        <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <path d="M3 3l7.07 16.97 2.51-7.39 7.39-2.51L3 3z" />
          <path d="M13 13l6 6" />
        </svg>
      </button>
      <button
        class="pb-tool-btn"
        :class="{ active: activeTool === 'pen' }"
        :title="t('pinboard.tools.pen')"
        @click="activeTool = 'pen'"
      >
        <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <path d="M17 3a2.83 2.83 0 1 1 4 4L7.5 20.5 2 22l1.5-5.5L17 3z" />
        </svg>
      </button>
      <button
        class="pb-tool-btn"
        :class="{ active: activeTool === 'arrow' }"
        :title="t('pinboard.tools.arrow')"
        @click="activeTool = 'arrow'"
      >
        <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <line x1="5" y1="19" x2="19" y2="5" />
          <polyline points="12 5 19 5 19 12" />
        </svg>
      </button>
      <button
        class="pb-tool-btn"
        :class="{ active: activeTool === 'rect' }"
        :title="t('pinboard.tools.rect')"
        @click="activeTool = 'rect'"
      >
        <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <rect x="3" y="3" width="18" height="18" rx="2" ry="2" />
        </svg>
      </button>
      <button
        class="pb-tool-btn"
        :class="{ active: activeTool === 'ellipse' }"
        :title="t('pinboard.tools.ellipse')"
        @click="activeTool = 'ellipse'"
      >
        <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <circle cx="12" cy="12" r="10" />
        </svg>
      </button>
      <button
        class="pb-tool-btn"
        :class="{ active: activeTool === 'text' }"
        :title="t('pinboard.tools.text')"
        @click="activeTool = 'text'"
      >
        <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <polyline points="4 7 4 4 20 4 20 7" />
          <line x1="12" y1="4" x2="12" y2="20" />
          <line x1="8" y1="20" x2="16" y2="20" />
        </svg>
      </button>
      <button
        class="pb-tool-btn"
        :class="{ active: activeTool === 'eraser' }"
        :title="t('pinboard.tools.eraser')"
        @click="activeTool = 'eraser'"
      >
        <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <path d="M20 20H7L3 16a1 1 0 0 1 0-1.41l9.59-9.59a2 2 0 0 1 2.82 0L20 9.59a2 2 0 0 1 0 2.82L11 21" />
          <line x1="18" y1="13" x2="11" y2="20" />
        </svg>
      </button>

      <!-- 大小调整 -->
      <template v-if="showSizeSlider">
        <div class="pb-separator" />
        <label class="pb-size-control">
          <span class="pb-size-label">{{ currentSizeValue }}</span>
          <input
            type="range"
            class="pb-size-slider"
            :min="currentSizeMin"
            :max="currentSizeMax"
            :value="currentSizeValue"
            @input="onSizeChange"
          />
        </label>
      </template>

      <div class="pb-separator" />

      <!-- 颜色选择 -->
      <button
        v-for="color in COLORS"
        :key="color"
        class="pb-color-dot"
        :class="{ active: activeColor === color }"
        :style="{ background: color }"
        @click="activeColor = color"
      />

      <div class="pb-separator" />

      <!-- 撤销/重做 -->
      <button
        class="pb-tool-btn"
        :title="t('pinboard.undo')"
        :disabled="!canUndo"
        @click="undo"
      >
        <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <polyline points="1 4 1 10 7 10" />
          <path d="M3.51 15a9 9 0 1 0 2.13-9.36L1 10" />
        </svg>
      </button>
      <button
        class="pb-tool-btn"
        :title="t('pinboard.redo')"
        :disabled="!canRedo"
        @click="redo"
      >
        <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <polyline points="23 4 23 10 17 10" />
          <path d="M20.49 15a9 9 0 1 1-2.13-9.36L23 10" />
        </svg>
      </button>

      <div class="pb-spacer" />

      <!-- 缩放 -->
      <button
        class="pb-tool-btn"
        :title="t('pinboard.zoomOut')"
        @click="zoomOut"
      >
        <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <line x1="5" y1="12" x2="19" y2="12" />
        </svg>
      </button>
      <span class="pb-zoom-label">{{ zoomLabel }}</span>
      <button
        class="pb-tool-btn"
        :title="t('pinboard.zoomIn')"
        @click="zoomIn"
      >
        <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <line x1="12" y1="5" x2="12" y2="19" />
          <line x1="5" y1="12" x2="19" y2="12" />
        </svg>
      </button>
    </div>

    <!-- 画布区域 -->
    <div class="pb-canvas-area">
      <PinboardCanvas
        v-if="activeTab"
        ref="canvasRef"
        :pins="pins"
        :canvas-annotations="canvasAnnotations"
        :viewport="viewport"
        :active-tool="activeTool"
        :active-color="activeColor"
        :stroke-size="currentStrokeSize"
        :font-size="currentFontSize"
        :selected-pin-id="selectedPinId"
        :get-image-url="getPinImageUrl"
        @select-pin="onSelectPin"
        @deselect="onDeselect"
        @update-pin="onUpdatePin"
        @delete-pin="onDeletePin"
        @add-annotation="onAddAnnotation"
        @remove-annotation="onRemoveAnnotation"
        @add-canvas-annotation="onAddCanvasAnnotation"
        @remove-canvas-annotation="onRemoveCanvasAnnotation"
        @update-viewport="onUpdateViewport"
      />

      <!-- 空状态：有标签但无贴图 -->
      <div v-if="activeTab && pins.length === 0" class="pb-empty">
        {{ t('pinboard.empty') }}
      </div>

      <!-- 空状态：无标签 -->
      <div v-if="!activeTab" class="pb-empty">
        {{ t('pinboard.noTabs') }}
      </div>
    </div>

    <!-- 状态栏 -->
    <div v-if="activeTab" class="pb-statusbar">
      {{ t('pinboard.pinCount', { count: pins.length }) }}
    </div>
  </div>
</template>

<style scoped>
.pinboard-page {
  display: flex;
  flex-direction: column;
  width: 100vw;
  height: 100vh;
  overflow: hidden;
  background: transparent;
}

/* ─── 标签栏 ─── */
.pb-tab-bar {
  display: flex;
  align-items: center;
  height: 36px;
  padding: 0 var(--spacing-1);
  border-bottom: 1px solid var(--border-light);
  flex-shrink: 0;
  user-select: none;
}

.pb-tabs {
  display: flex;
  min-width: 0;
  overflow-x: auto;
  gap: 1px;
}

.pb-tabs::-webkit-scrollbar {
  display: none;
}

.pb-tab {
  display: flex;
  align-items: center;
  gap: var(--spacing-1);
  padding: var(--spacing-1) var(--spacing-3);
  border-radius: var(--radius-md) var(--radius-md) 0 0;
  background: transparent;
  color: var(--text-tertiary);
  font-size: var(--text-xs);
  cursor: pointer;
  white-space: nowrap;
  max-width: 160px;
  transition: var(--transition-bg);
  flex-shrink: 0;
}

.pb-tab:hover {
  background: var(--bg-hover);
  color: var(--text-secondary);
}

.pb-tab.active {
  background: var(--bg-active);
  color: var(--text-primary);
}

.pb-tab-title {
  overflow: hidden;
  text-overflow: ellipsis;
}

.pb-tab-close {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 16px;
  height: 16px;
  border: none;
  border-radius: var(--radius-sm);
  background: transparent;
  color: var(--text-tertiary);
  cursor: pointer;
  flex-shrink: 0;
  opacity: 0;
  transition: var(--transition-bg);
}

.pb-tab:hover .pb-tab-close,
.pb-tab.active .pb-tab-close {
  opacity: 1;
}

.pb-tab-close:hover {
  background: var(--color-danger);
  color: var(--color-neutral-0);
}

.pb-drag-spacer {
  flex: 1;
  min-width: 32px;
  height: 100%;
}

/* ─── 窗口控制 ─── */
.pb-window-controls {
  display: flex;
  align-items: center;
  gap: 2px;
  margin-left: var(--spacing-2);
  -webkit-app-region: no-drag;
  flex-shrink: 0;
}

.pb-win-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 28px;
  height: 28px;
  border: none;
  border-radius: var(--radius-sm);
  background: transparent;
  color: var(--text-secondary);
  cursor: pointer;
  transition: var(--transition-bg);
}

.pb-win-btn:hover {
  background: var(--bg-hover);
  color: var(--text-primary);
}

.pb-win-close:hover {
  background: var(--color-danger) !important;
  color: var(--color-neutral-0) !important;
}

/* ─── 工具栏 ─── */
.pb-toolbar {
  display: flex;
  flex-direction: row;
  align-items: center;
  gap: var(--spacing-1);
  padding: var(--spacing-2) var(--spacing-3);
  border-bottom: 1px solid var(--border-light);
  flex-shrink: 0;
  position: relative;
  z-index: 21;
}

/* ─── 大小控制 ─── */
.pb-size-control {
  display: flex;
  align-items: center;
  gap: var(--spacing-1);
}

.pb-size-label {
  font-size: var(--text-xs);
  color: var(--text-secondary);
  min-width: 20px;
  text-align: right;
  user-select: none;
}

.pb-size-slider {
  -webkit-appearance: none;
  appearance: none;
  width: 80px;
  height: 14px;
  background: transparent;
  outline: none;
  cursor: pointer;
}

.pb-spacer {
  flex: 1;
}

.pb-zoom-label {
  font-size: var(--text-xs);
  color: var(--text-secondary);
  min-width: 40px;
  text-align: center;
  user-select: none;
}

/* ─── 画布区域 ─── */
.pb-canvas-area {
  flex: 1;
  overflow: hidden;
  position: relative;
  min-height: 0;
}

/* ─── 空状态 ─── */
.pb-empty {
  position: absolute;
  inset: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: var(--text-lg);
  color: var(--text-tertiary);
  pointer-events: none;
}

/* ─── 状态栏 ─── */
.pb-statusbar {
  padding: var(--spacing-2) var(--spacing-4);
  font-size: var(--text-xs);
  color: var(--text-tertiary);
  border-top: 1px solid var(--border-light);
  flex-shrink: 0;
}
</style>

<style>
/* 非 scoped：WebView2 伪元素无法被 scoped 属性选择器穿透 */
.pb-size-slider::-webkit-slider-runnable-track {
  width: 100%;
  height: 4px;
  background: var(--border-medium);
  border-radius: 2px;
  border: none;
}

.pb-size-slider::-webkit-slider-thumb {
  -webkit-appearance: none;
  appearance: none;
  width: 12px;
  height: 12px;
  border-radius: 50%;
  background: var(--color-primary);
  border: none;
  margin-top: -4px;
  cursor: pointer;
  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.3);
}
</style>
