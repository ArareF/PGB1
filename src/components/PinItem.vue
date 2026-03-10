<script setup lang="ts">
import { ref, watch, nextTick, onMounted, onBeforeUnmount } from 'vue'
import type { PinInfo, PinAnnotation } from '../composables/usePinboard'

const props = defineProps<{
  pin: PinInfo
  imageUrl: string
  isSelected: boolean
  activeTool: 'select' | 'pen' | 'arrow' | 'rect' | 'ellipse' | 'text' | 'eraser'
  activeColor: string
  canvasZoom: number
}>()

const emit = defineEmits<{
  select: []
  'update-position': [x: number, y: number]
  'update-size': [width: number, height: number]
  delete: []
  'add-annotation': [annotation: PinAnnotation]
  'remove-annotation': [index: number]
}>()

const canvasRef = ref<HTMLCanvasElement | null>(null)
const textInputRef = ref<HTMLInputElement | null>(null)

// ─── 拖拽移动 ──────────────────────────────────────────
const isDragging = ref(false)
let dragStartX = 0
let dragStartY = 0
let dragPinStartX = 0
let dragPinStartY = 0

function onMouseDownMain(e: MouseEvent) {
  if (props.activeTool !== 'select') return
  if ((e.target as HTMLElement).classList.contains('pin-resize-handle')) return
  if ((e.target as HTMLElement).classList.contains('pin-delete-btn')) return

  e.stopPropagation()
  emit('select')

  isDragging.value = true
  dragStartX = e.clientX
  dragStartY = e.clientY
  dragPinStartX = props.pin.x
  dragPinStartY = props.pin.y

  window.addEventListener('mousemove', onDragMove)
  window.addEventListener('mouseup', onDragEnd)
}

function onDragMove(e: MouseEvent) {
  if (!isDragging.value) return
  const dx = (e.clientX - dragStartX) / props.canvasZoom
  const dy = (e.clientY - dragStartY) / props.canvasZoom
  emit('update-position', dragPinStartX + dx, dragPinStartY + dy)
}

function onDragEnd() {
  isDragging.value = false
  window.removeEventListener('mousemove', onDragMove)
  window.removeEventListener('mouseup', onDragEnd)
}

// ─── 8 方向缩放 ────────────────────────────────────────
let resizeEdge = ''
let resizeStartX = 0
let resizeStartY = 0
let resizeStartW = 0
let resizeStartH = 0
let resizeStartPinX = 0
let resizeStartPinY = 0
const isResizing = ref(false)

function onResizeStart(e: MouseEvent, edge: string) {
  e.preventDefault()
  e.stopPropagation()
  emit('select')

  isResizing.value = true
  resizeEdge = edge
  resizeStartX = e.clientX
  resizeStartY = e.clientY
  resizeStartW = props.pin.width
  resizeStartH = props.pin.height
  resizeStartPinX = props.pin.x
  resizeStartPinY = props.pin.y

  window.addEventListener('mousemove', onResizeMove)
  window.addEventListener('mouseup', onResizeEnd)
}

function onResizeMove(e: MouseEvent) {
  if (!isResizing.value) return
  const dx = (e.clientX - resizeStartX) / props.canvasZoom
  const dy = (e.clientY - resizeStartY) / props.canvasZoom
  const aspect = resizeStartW / resizeStartH

  let newW = resizeStartW
  let newH = resizeStartH
  let newX = resizeStartPinX
  let newY = resizeStartPinY
  const isCorner = resizeEdge.length === 2

  if (resizeEdge.includes('e')) newW = Math.max(50, resizeStartW + dx)
  if (resizeEdge.includes('w')) {
    newW = Math.max(50, resizeStartW - dx)
    newX = resizeStartPinX + (resizeStartW - newW)
  }
  if (resizeEdge.includes('s')) newH = Math.max(50, resizeStartH + dy)
  if (resizeEdge.includes('n')) {
    newH = Math.max(50, resizeStartH - dy)
    newY = resizeStartPinY + (resizeStartH - newH)
  }

  // 角拖拽保持宽高比
  if (isCorner) {
    const scaleW = newW / resizeStartW
    const scaleH = newH / resizeStartH
    const scale = Math.max(scaleW, scaleH)
    newW = Math.max(50, resizeStartW * scale)
    newH = Math.max(50, newW / aspect)
    if (resizeEdge.includes('w')) newX = resizeStartPinX + resizeStartW - newW
    if (resizeEdge.includes('n')) newY = resizeStartPinY + resizeStartH - newH
  }

  emit('update-size', Math.round(newW), Math.round(newH))
  if (newX !== resizeStartPinX || newY !== resizeStartPinY) {
    emit('update-position', newX, newY)
  }
}

function onResizeEnd() {
  isResizing.value = false
  window.removeEventListener('mousemove', onResizeMove)
  window.removeEventListener('mouseup', onResizeEnd)
}

// ─── Canvas 标注层 ─────────────────────────────────────
const currentStroke = ref<PinAnnotation | null>(null)
let isDrawing = false

// 文字输入
const textInputVisible = ref(false)
const textInputX = ref(0)
const textInputY = ref(0)
const textInputValue = ref('')
let textInputNormPos: [number, number] = [0, 0]

function toNormalized(e: MouseEvent): [number, number] {
  const canvas = canvasRef.value!
  const rect = canvas.getBoundingClientRect()
  const x = (e.clientX - rect.left) / rect.width
  const y = (e.clientY - rect.top) / rect.height
  return [Math.max(0, Math.min(1, x)), Math.max(0, Math.min(1, y))]
}

function onCanvasMouseDown(e: MouseEvent) {
  if (props.activeTool === 'select') return
  if (props.activeTool === 'text') return

  e.stopPropagation()
  emit('select')

  isDrawing = true
  const pos = toNormalized(e)

  if (props.activeTool === 'eraser') {
    currentStroke.value = {
      type: 'eraser',
      color: 'rgba(0,0,0,1)',
      strokeWidth: 20,
      points: [pos],
    }
  } else if (props.activeTool === 'pen') {
    currentStroke.value = {
      type: 'pen',
      color: props.activeColor,
      strokeWidth: 3,
      points: [pos],
    }
  } else if (props.activeTool === 'arrow') {
    currentStroke.value = {
      type: 'arrow',
      color: props.activeColor,
      strokeWidth: 3,
      start: pos,
      end: pos,
    }
  } else if (props.activeTool === 'rect') {
    currentStroke.value = {
      type: 'rect',
      color: props.activeColor,
      strokeWidth: 3,
      start: pos,
      end: pos,
    }
  } else if (props.activeTool === 'ellipse') {
    currentStroke.value = {
      type: 'ellipse',
      color: props.activeColor,
      strokeWidth: 3,
      start: pos,
      end: pos,
    }
  }

  renderAnnotations()
}

function onCanvasMouseMove(e: MouseEvent) {
  if (!isDrawing || !currentStroke.value) return
  const pos = toNormalized(e)

  if (currentStroke.value.type === 'pen' || currentStroke.value.type === 'eraser') {
    currentStroke.value.points!.push(pos)
  } else {
    currentStroke.value.end = pos
  }

  renderAnnotations()
}

function onCanvasMouseUp() {
  if (!isDrawing || !currentStroke.value) return
  isDrawing = false

  const ann = currentStroke.value
  let valid = true
  if ((ann.type === 'pen' || ann.type === 'eraser') && (!ann.points || ann.points.length < 2)) valid = false
  if (ann.type !== 'pen' && ann.type !== 'eraser' && ann.start && ann.end) {
    const dx = Math.abs(ann.end[0] - ann.start[0])
    const dy = Math.abs(ann.end[1] - ann.start[1])
    if (dx < 0.005 && dy < 0.005) valid = false
  }

  if (valid) {
    emit('add-annotation', { ...ann })
  }

  currentStroke.value = null
  renderAnnotations()
}

function onCanvasClick(e: MouseEvent) {
  emit('select')

  if (props.activeTool === 'text') {
    e.stopPropagation()
    const pos = toNormalized(e)
    textInputNormPos = pos
    const canvas = canvasRef.value!
    const rect = canvas.getBoundingClientRect()
    textInputX.value = e.clientX - rect.left
    textInputY.value = e.clientY - rect.top
    textInputValue.value = ''
    textInputVisible.value = true
    nextTick(() => textInputRef.value?.focus())
  }
}

function onTextInputConfirm() {
  if (textInputValue.value.trim()) {
    emit('add-annotation', {
      type: 'text',
      color: props.activeColor,
      strokeWidth: 1,
      text: textInputValue.value.trim(),
      position: textInputNormPos,
      fontSize: 16,
    })
  }
  textInputVisible.value = false
  textInputValue.value = ''
}

function onTextInputKeydown(e: KeyboardEvent) {
  e.stopPropagation()
  if (e.key === 'Enter') {
    onTextInputConfirm()
  } else if (e.key === 'Escape') {
    textInputVisible.value = false
    textInputValue.value = ''
  }
}



// ─── Canvas 渲染 ───────────────────────────────────────
function renderAnnotations() {
  const canvas = canvasRef.value
  if (!canvas) return
  const ctx = canvas.getContext('2d')
  if (!ctx) return
  const w = canvas.width
  const h = canvas.height
  ctx.clearRect(0, 0, w, h)

  for (const ann of props.pin.annotations) {
    drawAnnotation(ctx, ann, w, h)
  }
  if (currentStroke.value) {
    drawAnnotation(ctx, currentStroke.value, w, h)
  }
}

function drawAnnotation(ctx: CanvasRenderingContext2D, ann: PinAnnotation, w: number, h: number) {
  const isEraser = ann.type === 'eraser'
  if (isEraser) {
    ctx.save()
    ctx.globalCompositeOperation = 'destination-out'
    ctx.strokeStyle = 'rgba(0,0,0,1)'
    ctx.lineWidth = ann.strokeWidth * Math.min(w, h) / 500
    ctx.lineCap = 'round'
    ctx.lineJoin = 'round'
    if (ann.points && ann.points.length >= 2) {
      // 多次绘制消除抗锯齿半透明边缘残留
      for (let pass = 0; pass < 3; pass++) {
        ctx.beginPath()
        ctx.moveTo(ann.points[0][0] * w, ann.points[0][1] * h)
        for (let i = 1; i < ann.points.length; i++) {
          ctx.lineTo(ann.points[i][0] * w, ann.points[i][1] * h)
        }
        ctx.stroke()
      }
    }
    ctx.restore()
    return
  }

  ctx.strokeStyle = ann.color
  ctx.fillStyle = ann.color
  ctx.lineWidth = ann.strokeWidth * Math.min(w, h) / 500
  ctx.lineCap = 'round'
  ctx.lineJoin = 'round'

  switch (ann.type) {
    case 'pen': {
      if (!ann.points || ann.points.length < 2) return
      ctx.beginPath()
      ctx.moveTo(ann.points[0][0] * w, ann.points[0][1] * h)
      for (let i = 1; i < ann.points.length; i++) {
        ctx.lineTo(ann.points[i][0] * w, ann.points[i][1] * h)
      }
      ctx.stroke()
      break
    }
    case 'arrow': {
      if (!ann.start || !ann.end) return
      const [x1, y1] = [ann.start[0] * w, ann.start[1] * h]
      const [x2, y2] = [ann.end[0] * w, ann.end[1] * h]
      ctx.beginPath()
      ctx.moveTo(x1, y1)
      ctx.lineTo(x2, y2)
      ctx.stroke()
      const angle = Math.atan2(y2 - y1, x2 - x1)
      const headLen = 12
      ctx.beginPath()
      ctx.moveTo(x2, y2)
      ctx.lineTo(x2 - headLen * Math.cos(angle - Math.PI / 6), y2 - headLen * Math.sin(angle - Math.PI / 6))
      ctx.lineTo(x2 - headLen * Math.cos(angle + Math.PI / 6), y2 - headLen * Math.sin(angle + Math.PI / 6))
      ctx.closePath()
      ctx.fill()
      break
    }
    case 'rect': {
      if (!ann.start || !ann.end) return
      const rx = ann.start[0] * w, ry = ann.start[1] * h
      const rw = (ann.end[0] - ann.start[0]) * w, rh = (ann.end[1] - ann.start[1]) * h
      ctx.strokeRect(rx, ry, rw, rh)
      break
    }
    case 'ellipse': {
      if (!ann.start || !ann.end) return
      const cx = ((ann.start[0] + ann.end[0]) / 2) * w
      const cy = ((ann.start[1] + ann.end[1]) / 2) * h
      const erx = Math.abs(ann.end[0] - ann.start[0]) / 2 * w
      const ery = Math.abs(ann.end[1] - ann.start[1]) / 2 * h
      ctx.beginPath()
      ctx.ellipse(cx, cy, Math.max(erx, 1), Math.max(ery, 1), 0, 0, Math.PI * 2)
      ctx.stroke()
      break
    }
    case 'text': {
      if (!ann.position || !ann.text) return
      const fs = (ann.fontSize ?? 16) * h / 500
      ctx.font = `${fs}px sans-serif`
      ctx.fillText(ann.text, ann.position[0] * w, ann.position[1] * h)
      break
    }
  }
}

// ─── 同步 Canvas 尺寸 + 重绘 ──────────────────────────
function syncCanvasSize() {
  const canvas = canvasRef.value
  if (!canvas) return
  canvas.width = props.pin.width
  canvas.height = props.pin.height
  renderAnnotations()
}

watch(() => [props.pin.width, props.pin.height], syncCanvasSize)
watch(() => props.pin.annotations, renderAnnotations, { deep: true })

onMounted(() => {
  nextTick(syncCanvasSize)
})

// ─── Cursor 样式 ──────────────────────────────────────
const TOOL_CURSORS: Record<string, string> = {
  select: 'move',
  pen: 'crosshair',
  arrow: 'crosshair',
  rect: 'crosshair',
  ellipse: 'crosshair',
  text: 'text',
  eraser: 'cell',
}

// ─── 清理 ──────────────────────────────────────────────
onBeforeUnmount(() => {
  window.removeEventListener('mousemove', onDragMove)
  window.removeEventListener('mouseup', onDragEnd)
  window.removeEventListener('mousemove', onResizeMove)
  window.removeEventListener('mouseup', onResizeEnd)
})
</script>

<template>
  <div
    class="pin-item"
    :class="{ 'pin-selected': isSelected }"
    :style="{
      left: pin.x + 'px',
      top: pin.y + 'px',
      width: pin.width + 'px',
      height: pin.height + 'px',
      cursor: TOOL_CURSORS[activeTool] ?? 'default',
    }"
    @mousedown="onMouseDownMain"
  >
    <img
      class="pin-image"
      :src="imageUrl"
      :width="pin.width"
      :height="pin.height"
      draggable="false"
      alt=""
    />

    <canvas
      ref="canvasRef"
      class="pin-canvas"
      @mousedown="onCanvasMouseDown"
      @mousemove="onCanvasMouseMove"
      @mouseup="onCanvasMouseUp"
      @click="onCanvasClick"
    />

    <!-- 文字输入框 -->
    <input
      v-if="textInputVisible"
      ref="textInputRef"
      v-model="textInputValue"
      class="pin-text-input"
      :style="{ left: textInputX + 'px', top: textInputY + 'px' }"
      @keydown="onTextInputKeydown"
      @blur="onTextInputConfirm"
    />

    <!-- 选中态 UI -->
    <template v-if="isSelected">
      <!-- 8 方向缩放手柄 -->
      <div class="pin-resize-handle handle-n" @mousedown="onResizeStart($event, 'n')" />
      <div class="pin-resize-handle handle-s" @mousedown="onResizeStart($event, 's')" />
      <div class="pin-resize-handle handle-e" @mousedown="onResizeStart($event, 'e')" />
      <div class="pin-resize-handle handle-w" @mousedown="onResizeStart($event, 'w')" />
      <div class="pin-resize-handle handle-ne" @mousedown="onResizeStart($event, 'ne')" />
      <div class="pin-resize-handle handle-nw" @mousedown="onResizeStart($event, 'nw')" />
      <div class="pin-resize-handle handle-se" @mousedown="onResizeStart($event, 'se')" />
      <div class="pin-resize-handle handle-sw" @mousedown="onResizeStart($event, 'sw')" />

      <!-- 删除按钮（DOM 顺序在手柄之后，确保覆盖 handle-ne） -->
      <button class="pin-delete-btn" @mousedown.stop @click.stop="$emit('delete')">
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3" stroke-linecap="round" stroke-linejoin="round">
          <line x1="18" y1="6" x2="6" y2="18" /><line x1="6" y1="6" x2="18" y2="18" />
        </svg>
      </button>
    </template>
  </div>
</template>

<style scoped>
.pin-item {
  position: absolute;
  user-select: none;
}

.pin-image {
  display: block;
  width: 100%;
  height: 100%;
  object-fit: contain;
  pointer-events: none;
  border-radius: var(--radius-sm);
}

.pin-canvas {
  position: absolute;
  inset: 0;
  width: 100%;
  height: 100%;
  background: transparent;
  border-radius: var(--radius-sm);
  z-index: 1;
}

/* ─── 选中态 ─── */
.pin-selected {
  outline: 2px solid var(--color-primary);
  outline-offset: 1px;
  border-radius: var(--radius-sm);
}

/* ─── 删除按钮 ─── */
.pin-delete-btn {
  position: absolute;
  top: -14px;
  right: -14px;
  width: 28px;
  height: 28px;
  border-radius: 50%;
  background: var(--color-danger);
  color: var(--color-neutral-0);
  border: none;
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 20;
  transition: transform var(--duration-fast);
}

.pin-delete-btn:hover {
  transform: scale(1.15);
}

/* ─── 缩放手柄 ─── */
.pin-resize-handle {
  position: absolute;
  width: 8px;
  height: 8px;
  background: var(--color-primary);
  border: 1px solid var(--color-neutral-0);
  z-index: 20;
}

.handle-n  { top: -3px; left: 50%; transform: translateX(-50%); cursor: n-resize; }
.handle-s  { bottom: -3px; left: 50%; transform: translateX(-50%); cursor: s-resize; }
.handle-e  { right: -3px; top: 50%; transform: translateY(-50%); cursor: e-resize; }
.handle-w  { left: -3px; top: 50%; transform: translateY(-50%); cursor: w-resize; }
.handle-ne { top: -3px; right: -3px; cursor: ne-resize; }
.handle-nw { top: -3px; left: -3px; cursor: nw-resize; }
.handle-se { bottom: -3px; right: -3px; cursor: se-resize; }
.handle-sw { bottom: -3px; left: -3px; cursor: sw-resize; }

/* ─── 文字输入框 ─── */
.pin-text-input {
  position: absolute;
  min-width: 60px;
  max-width: 200px;
  padding: var(--spacing-1) var(--spacing-2);
  font-size: var(--text-sm);
  font-family: inherit;
  border: 1px solid var(--color-primary);
  border-radius: var(--radius-sm);
  background: var(--bg-primary);
  color: var(--text-primary);
  outline: none;
  z-index: 20;
}
</style>
