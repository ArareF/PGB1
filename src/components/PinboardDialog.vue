<script setup lang="ts">
import { ref, computed, watch, onMounted, onBeforeUnmount, nextTick, toRef } from 'vue'
import { useI18n } from 'vue-i18n'
import { usePinboard } from '../composables/usePinboard'
import PinboardCanvas from './PinboardCanvas.vue'
import type { PinInfo, PinAnnotation, PinboardViewport } from '../composables/usePinboard'

const props = defineProps<{
  show: boolean
  dirPath: string
  canvasKey: string
}>()

defineEmits<{
  close: []
}>()

const { t } = useI18n()

// ─── 贴图板数据 ────────────────────────────────────────
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
} = usePinboard(toRef(props, 'dirPath'), toRef(props, 'canvasKey'))

// ─── 工具 & 颜色 ──────────────────────────────────────
type ToolType = 'select' | 'pen' | 'arrow' | 'rect' | 'ellipse' | 'text' | 'eraser'
const activeTool = ref<ToolType>('select')
const COLORS = ['#FF3B30', '#007AFF', '#34C759', '#FF9500', '#FFFFFF'] as const
const activeColor = ref<string>(COLORS[0])
const selectedPinId = ref<string | null>(null)

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
    // 贴图级撤销
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
    // 画布级撤销
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

// ─── 弹窗尺寸 ─────────────────────────────────────────
const STORAGE_KEY = 'pgb1-pinboard-size'
const MIN_PCT = 40
const MAX_PCT = 95

function clamp(v: number) {
  return Math.max(MIN_PCT, Math.min(MAX_PCT, v))
}

function loadSavedSize(): { w: number; h: number } {
  try {
    const raw = localStorage.getItem(STORAGE_KEY)
    if (raw) {
      const parsed = JSON.parse(raw)
      return { w: clamp(parsed.w), h: clamp(parsed.h) }
    }
  } catch { /* 忽略 */ }
  return { w: 75, h: 80 }
}

const saved = loadSavedSize()
const dialogWidth = ref(saved.w)
const dialogHeight = ref(saved.h)

function saveSize() {
  localStorage.setItem(STORAGE_KEY, JSON.stringify({ w: dialogWidth.value, h: dialogHeight.value }))
}

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

// ─── 键盘快捷键 ───────────────────────────────────────
function onKeyDown(e: KeyboardEvent) {
  if (!props.show) return

  // 传递给 Canvas（空格平移）
  canvasRef.value?.onKeyDown(e)

  if (e.ctrlKey && e.key === 'v') {
    e.preventDefault()
    handlePaste()
  } else if (e.ctrlKey && e.key === 'z') {
    e.preventDefault()
    undo()
  } else if (e.ctrlKey && e.key === 'y') {
    e.preventDefault()
    redo()
  } else if (e.key === 'Delete' && selectedPinId.value) {
    e.preventDefault()
    onDeletePin(selectedPinId.value)
  }
}

function onKeyUp(e: KeyboardEvent) {
  canvasRef.value?.onKeyUp(e)
}

onMounted(() => {
  window.addEventListener('keydown', onKeyDown)
  window.addEventListener('keyup', onKeyUp)
})

onBeforeUnmount(() => {
  window.removeEventListener('keydown', onKeyDown)
  window.removeEventListener('keyup', onKeyUp)
})

// ─── show 变化时加载 ──────────────────────────────────
watch(() => props.show, (v) => {
  if (v) {
    selectedPinId.value = null
    activeTool.value = 'select'
    undoStacks.value.clear()
    redoStacks.value.clear()
    canvasUndoStack.value = []
    canvasRedoStack.value = []
    nextTick(() => loadPinboard())
  }
})
</script>

<template>
  <Teleport to="body">
    <Transition name="dialog">
      <div v-if="show" class="pb-overlay" @mousedown.self="$emit('close')">
        <div
          class="pb-dialog glass-strong"
          :style="{ width: dialogWidth + 'vw', height: dialogHeight + 'vh' }"
        >
          <!-- 8 方向 resize 手柄 -->
          <div class="pb-resize pb-resize-n" @mousedown="onResizeStart($event, 'n')" />
          <div class="pb-resize pb-resize-s" @mousedown="onResizeStart($event, 's')" />
          <div class="pb-resize pb-resize-e" @mousedown="onResizeStart($event, 'e')" />
          <div class="pb-resize pb-resize-w" @mousedown="onResizeStart($event, 'w')" />
          <div class="pb-resize pb-resize-ne" @mousedown="onResizeStart($event, 'ne')" />
          <div class="pb-resize pb-resize-nw" @mousedown="onResizeStart($event, 'nw')" />
          <div class="pb-resize pb-resize-se" @mousedown="onResizeStart($event, 'se')" />
          <div class="pb-resize pb-resize-sw" @mousedown="onResizeStart($event, 'sw')" />

          <!-- 工具栏 -->
          <div class="pb-toolbar">
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

            <div class="pb-separator" />

            <!-- 关闭 -->
            <button class="pb-tool-btn pb-close-btn" @click="$emit('close')">
              <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                <line x1="18" y1="6" x2="6" y2="18" /><line x1="6" y1="6" x2="18" y2="18" />
              </svg>
            </button>
          </div>

          <!-- 画布区域 -->
          <div class="pb-canvas-area">
            <PinboardCanvas
              ref="canvasRef"
              :pins="pins"
              :canvas-annotations="canvasAnnotations"
              :viewport="viewport"
              :active-tool="activeTool"
              :active-color="activeColor"
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

            <!-- 空状态 -->
            <div v-if="pins.length === 0" class="pb-empty">
              {{ t('pinboard.empty') }}
            </div>
          </div>

          <!-- 状态栏 -->
          <div class="pb-statusbar">
            {{ t('pinboard.pinCount', { count: pins.length }) }}
          </div>
        </div>
      </div>
    </Transition>
  </Teleport>
</template>

<style scoped>
/* ─── 遮罩层 ─── */
.pb-overlay {
  position: fixed;
  inset: 0;
  z-index: var(--z-modal, 1000);
  display: flex;
  align-items: center;
  justify-content: center;
  background: var(--overlay-backdrop);
  backdrop-filter: blur(var(--glass-light-blur));
}

/* ─── 弹窗主体 ─── */
.pb-dialog {
  position: relative;
  display: flex;
  flex-direction: column;
  border-radius: var(--floating-navbar-radius);
  overflow: hidden;
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

.pb-close-btn:hover {
  background: var(--color-danger) !important;
  color: var(--color-neutral-0) !important;
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

/* ─── 拖拽调整尺寸手柄 ─── */
.pb-resize {
  position: absolute;
  z-index: 20;
}

.pb-resize-n  { top: 0; left: 8px; right: 8px; height: 5px; cursor: n-resize; }
.pb-resize-s  { bottom: 0; left: 8px; right: 8px; height: 5px; cursor: s-resize; }
.pb-resize-e  { top: 8px; right: 0; bottom: 8px; width: 5px; cursor: e-resize; }
.pb-resize-w  { top: 8px; left: 0; bottom: 8px; width: 5px; cursor: w-resize; }
.pb-resize-ne { top: 0; right: 0; width: 10px; height: 10px; cursor: ne-resize; }
.pb-resize-nw { top: 0; left: 0; width: 10px; height: 10px; cursor: nw-resize; }
.pb-resize-se { bottom: 0; right: 0; width: 10px; height: 10px; cursor: se-resize; }
.pb-resize-sw { bottom: 0; left: 0; width: 10px; height: 10px; cursor: sw-resize; }
</style>
