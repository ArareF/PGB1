<script setup lang="ts">
import { ref, computed, watch, nextTick, onMounted, onBeforeUnmount } from 'vue'
import PinItem from './PinItem.vue'
import type { PinInfo, PinAnnotation, PinboardViewport } from '../composables/usePinboard'

const props = defineProps<{
  pins: PinInfo[]
  canvasAnnotations: PinAnnotation[]
  viewport: PinboardViewport
  activeTool: string
  activeColor: string
  strokeSize: number
  fontSize: number
  selectedPinId: string | null
  getImageUrl: (pin: PinInfo) => string
}>()

const emit = defineEmits<{
  'select-pin': [pinId: string]
  'deselect': []
  'update-pin': [pinId: string, updates: Partial<PinInfo>]
  'delete-pin': [pinId: string]
  'add-annotation': [pinId: string, annotation: PinAnnotation]
  'remove-annotation': [pinId: string, index: number]
  'add-canvas-annotation': [annotation: PinAnnotation]
  'remove-canvas-annotation': [index: number]
  'update-viewport': [viewport: PinboardViewport]
}>()

const canvasRef = ref<HTMLElement | null>(null)
const annotationCanvasRef = ref<HTMLCanvasElement | null>(null)
const canvasTextInputRef = ref<HTMLInputElement | null>(null)

// ─── 排序（zIndex 升序，高的在上） ─────────────────────
const sortedPins = computed(() =>
  [...props.pins].sort((a, b) => a.zIndex - b.zIndex)
)

// ─── 内层 transform ────────────────────────────────────
const innerStyle = computed(() => ({
  transform: `translate(${props.viewport.panX}px, ${props.viewport.panY}px) scale(${props.viewport.zoom})`,
  transformOrigin: '0 0',
}))

// ─── 滚轮缩放 ─────────────────────────────────────────
const MIN_ZOOM = 0.1
const MAX_ZOOM = 1.0

function onWheel(e: WheelEvent) {
  const rect = canvasRef.value!.getBoundingClientRect()
  const mouseX = e.clientX - rect.left
  const mouseY = e.clientY - rect.top

  const oldZoom = props.viewport.zoom
  const factor = e.deltaY < 0 ? 1.1 : 1 / 1.1
  const newZoom = Math.max(MIN_ZOOM, Math.min(MAX_ZOOM, oldZoom * factor))

  // 以鼠标位置为中心缩放
  const newPanX = mouseX - (mouseX - props.viewport.panX) * (newZoom / oldZoom)
  const newPanY = mouseY - (mouseY - props.viewport.panY) * (newZoom / oldZoom)

  emit('update-viewport', { panX: newPanX, panY: newPanY, zoom: newZoom })
}

// ─── 平移（中键 / 空格+左键） ─────────────────────────
let isPanning = false
let panStartX = 0
let panStartY = 0
let panStartPanX = 0
let panStartPanY = 0
const spaceHeld = ref(false)
const cursorClass = ref('')

function onKeyDown(e: KeyboardEvent) {
  if (e.code === 'Space' && !spaceHeld.value) {
    spaceHeld.value = true
    cursorClass.value = 'cursor-grab'
  }
}

function onKeyUp(e: KeyboardEvent) {
  if (e.code === 'Space') {
    spaceHeld.value = false
    if (!isPanning) cursorClass.value = ''
  }
}

function onMouseDown(e: MouseEvent) {
  // 中键拖拽
  if (e.button === 1) {
    e.preventDefault()
    startPan(e)
    return
  }

  // 空格 + 左键拖拽
  if (e.button === 0 && spaceHeld.value) {
    e.preventDefault()
    startPan(e)
    return
  }

  // 左键点击空白 → deselect
  if (e.button === 0) {
    emit('deselect')
  }
}

function startPan(e: MouseEvent) {
  isPanning = true
  panStartX = e.clientX
  panStartY = e.clientY
  panStartPanX = props.viewport.panX
  panStartPanY = props.viewport.panY
  cursorClass.value = 'cursor-grabbing'

  window.addEventListener('mousemove', onPanMove)
  window.addEventListener('mouseup', onPanEnd)
}

function onPanMove(e: MouseEvent) {
  if (!isPanning) return
  const dx = e.clientX - panStartX
  const dy = e.clientY - panStartY
  emit('update-viewport', {
    ...props.viewport,
    panX: panStartPanX + dx,
    panY: panStartPanY + dy,
  })
}

function onPanEnd() {
  isPanning = false
  cursorClass.value = spaceHeld.value ? 'cursor-grab' : ''
  window.removeEventListener('mousemove', onPanMove)
  window.removeEventListener('mouseup', onPanEnd)
}

// ─── 笔刷/橡皮擦光标指示器 ──────────────────────────────
const cursorMouseX = ref(0)
const cursorMouseY = ref(0)
const mouseInCanvas = ref(false)

const showBrushCursor = computed(() =>
  mouseInCanvas.value && canvasDrawingActive.value
  && (props.activeTool === 'pen' || props.activeTool === 'eraser')
)

const brushCursorStyle = computed(() => {
  const size = Math.max(props.strokeSize * props.viewport.zoom, 4)
  return {
    left: cursorMouseX.value - size / 2 + 'px',
    top: cursorMouseY.value - size / 2 + 'px',
    width: size + 'px',
    height: size + 'px',
  }
})

function onTrackMouse(e: MouseEvent) {
  const rect = canvasRef.value?.getBoundingClientRect()
  if (!rect) return
  cursorMouseX.value = e.clientX - rect.left
  cursorMouseY.value = e.clientY - rect.top
}

// ─── 画布标注：坐标转换 ─────────────────────────────────
function toWorldCoords(e: MouseEvent): [number, number] {
  const rect = canvasRef.value!.getBoundingClientRect()
  const screenX = e.clientX - rect.left
  const screenY = e.clientY - rect.top
  return [
    (screenX - props.viewport.panX) / props.viewport.zoom,
    (screenY - props.viewport.panY) / props.viewport.zoom,
  ]
}

// ─── 画布标注：绘制状态 ─────────────────────────────────
const canvasStroke = ref<PinAnnotation | null>(null)
let isCanvasDrawing = false

// 画布标注是否激活（绘制工具 + 无选中贴图）
const canvasDrawingActive = computed(() =>
  props.activeTool !== 'select' && !props.selectedPinId
)

// 文字输入
const canvasTextVisible = ref(false)
const canvasTextX = ref(0)
const canvasTextY = ref(0)
const canvasTextValue = ref('')
let canvasTextWorldPos: [number, number] = [0, 0]

// ─── 画布标注：鼠标事件 ─────────────────────────────────
function onOverlayMouseDown(e: MouseEvent) {
  if (e.button !== 0) return
  e.stopPropagation()

  const tool = props.activeTool
  if (tool === 'select' || tool === 'text') return

  isCanvasDrawing = true
  const pos = toWorldCoords(e)

  if (tool === 'eraser') {
    canvasStroke.value = { type: 'eraser', color: 'rgba(0,0,0,1)', strokeWidth: props.strokeSize, points: [pos] }
  } else if (tool === 'pen') {
    canvasStroke.value = { type: 'pen', color: props.activeColor, strokeWidth: props.strokeSize, points: [pos] }
  } else if (tool === 'arrow') {
    canvasStroke.value = { type: 'arrow', color: props.activeColor, strokeWidth: props.strokeSize, start: pos, end: pos }
  } else if (tool === 'rect') {
    canvasStroke.value = { type: 'rect', color: props.activeColor, strokeWidth: props.strokeSize, start: pos, end: pos }
  } else if (tool === 'ellipse') {
    canvasStroke.value = { type: 'ellipse', color: props.activeColor, strokeWidth: props.strokeSize, start: pos, end: pos }
  }

  renderCanvasAnnotations()
  window.addEventListener('mousemove', onGlobalDrawMove)
  window.addEventListener('mouseup', onGlobalDrawUp)
}

function onGlobalDrawMove(e: MouseEvent) {
  if (!isCanvasDrawing || !canvasStroke.value) return
  const pos = toWorldCoords(e)
  if (canvasStroke.value.type === 'pen' || canvasStroke.value.type === 'eraser') {
    canvasStroke.value.points!.push(pos)
  } else {
    canvasStroke.value.end = pos
  }
  renderCanvasAnnotations()
}

function onGlobalDrawUp() {
  if (!isCanvasDrawing || !canvasStroke.value) return
  isCanvasDrawing = false

  const ann = canvasStroke.value
  let valid = true
  if ((ann.type === 'pen' || ann.type === 'eraser') && (!ann.points || ann.points.length < 2)) valid = false
  if (ann.type !== 'pen' && ann.type !== 'eraser' && ann.start && ann.end) {
    const dx = Math.abs(ann.end[0] - ann.start[0])
    const dy = Math.abs(ann.end[1] - ann.start[1])
    if (dx < 3 && dy < 3) valid = false
  }

  if (valid) {
    emit('add-canvas-annotation', { ...ann })
  }

  canvasStroke.value = null
  renderCanvasAnnotations()
  window.removeEventListener('mousemove', onGlobalDrawMove)
  window.removeEventListener('mouseup', onGlobalDrawUp)
}

function onOverlayMouseMove(_e: MouseEvent) {
  // 橡皮擦现在是笔触模式，由 onGlobalDrawMove 处理
}

function onOverlayClick(e: MouseEvent) {
  if (props.activeTool !== 'text') return
  e.stopPropagation()
  const pos = toWorldCoords(e)
  canvasTextWorldPos = pos
  const rect = canvasRef.value!.getBoundingClientRect()
  canvasTextX.value = e.clientX - rect.left
  canvasTextY.value = e.clientY - rect.top
  canvasTextValue.value = ''
  canvasTextVisible.value = true
  nextTick(() => canvasTextInputRef.value?.focus())
}

function onCanvasTextConfirm() {
  if (canvasTextValue.value.trim()) {
    emit('add-canvas-annotation', {
      type: 'text',
      color: props.activeColor,
      strokeWidth: 1,
      text: canvasTextValue.value.trim(),
      position: canvasTextWorldPos,
      fontSize: props.fontSize,
    })
  }
  canvasTextVisible.value = false
  canvasTextValue.value = ''
}

function onCanvasTextKeydown(e: KeyboardEvent) {
  e.stopPropagation()
  if (e.key === 'Enter') onCanvasTextConfirm()
  else if (e.key === 'Escape') {
    canvasTextVisible.value = false
    canvasTextValue.value = ''
  }
}

// ─── 画布标注：碰撞检测 ─────────────────────────────────
function canvasHitTest(ann: PinAnnotation, pos: [number, number], radius: number): boolean {
  const [px, py] = pos
  if (ann.type === 'pen' && ann.points) {
    for (const pt of ann.points) {
      if (Math.hypot(pt[0] - px, pt[1] - py) < radius) return true
    }
  } else if (ann.type === 'text' && ann.position) {
    if (Math.hypot(ann.position[0] - px, ann.position[1] - py) < radius * 2) return true
  } else if (ann.start && ann.end) {
    const midX = (ann.start[0] + ann.end[0]) / 2
    const midY = (ann.start[1] + ann.end[1]) / 2
    if (Math.hypot(ann.start[0] - px, ann.start[1] - py) < radius) return true
    if (Math.hypot(ann.end[0] - px, ann.end[1] - py) < radius) return true
    if (Math.hypot(midX - px, midY - py) < radius) return true
    if (ann.type === 'rect' || ann.type === 'ellipse') {
      const edges: [number, number][] = [
        [ann.start[0], midY], [ann.end[0], midY],
        [midX, ann.start[1]], [midX, ann.end[1]],
      ]
      for (const ep of edges) {
        if (Math.hypot(ep[0] - px, ep[1] - py) < radius) return true
      }
    }
  }
  return false
}

// ─── 画布标注：渲染 ─────────────────────────────────────
function renderCanvasAnnotations() {
  const canvas = annotationCanvasRef.value
  const container = canvasRef.value
  if (!canvas || !container) return
  const ctx = canvas.getContext('2d')
  if (!ctx) return

  const w = container.clientWidth
  const h = container.clientHeight
  if (canvas.width !== w || canvas.height !== h) {
    canvas.width = w
    canvas.height = h
  }

  ctx.clearRect(0, 0, w, h)
  ctx.save()
  ctx.setTransform(
    props.viewport.zoom, 0,
    0, props.viewport.zoom,
    props.viewport.panX, props.viewport.panY,
  )

  const zoom = props.viewport.zoom
  for (const ann of props.canvasAnnotations) {
    drawWorldAnnotation(ctx, ann, zoom)
  }
  if (canvasStroke.value) {
    drawWorldAnnotation(ctx, canvasStroke.value, zoom)
  }

  ctx.restore()
}

function drawWorldAnnotation(ctx: CanvasRenderingContext2D, ann: PinAnnotation, zoom: number) {
  if (ann.type === 'eraser') {
    if (!ann.points || ann.points.length < 2) return
    ctx.save()
    ctx.globalCompositeOperation = 'destination-out'
    ctx.strokeStyle = 'rgba(0,0,0,1)'
    ctx.lineWidth = ann.strokeWidth / zoom
    ctx.lineCap = 'round'
    ctx.lineJoin = 'round'
    // 多次绘制消除抗锯齿半透明边缘残留
    for (let pass = 0; pass < 3; pass++) {
      ctx.beginPath()
      ctx.moveTo(ann.points[0][0], ann.points[0][1])
      for (let i = 1; i < ann.points.length; i++) {
        ctx.lineTo(ann.points[i][0], ann.points[i][1])
      }
      ctx.stroke()
    }
    ctx.restore()
    return
  }

  ctx.strokeStyle = ann.color
  ctx.fillStyle = ann.color
  ctx.lineWidth = ann.strokeWidth / zoom
  ctx.lineCap = 'round'
  ctx.lineJoin = 'round'

  switch (ann.type) {
    case 'pen': {
      if (!ann.points || ann.points.length < 2) return
      ctx.beginPath()
      ctx.moveTo(ann.points[0][0], ann.points[0][1])
      for (let i = 1; i < ann.points.length; i++) {
        ctx.lineTo(ann.points[i][0], ann.points[i][1])
      }
      ctx.stroke()
      break
    }
    case 'arrow': {
      if (!ann.start || !ann.end) return
      const [x1, y1] = ann.start
      const [x2, y2] = ann.end
      ctx.beginPath()
      ctx.moveTo(x1, y1)
      ctx.lineTo(x2, y2)
      ctx.stroke()
      const angle = Math.atan2(y2 - y1, x2 - x1)
      const headLen = 14 / zoom
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
      ctx.strokeRect(ann.start[0], ann.start[1], ann.end[0] - ann.start[0], ann.end[1] - ann.start[1])
      break
    }
    case 'ellipse': {
      if (!ann.start || !ann.end) return
      const cx = (ann.start[0] + ann.end[0]) / 2
      const cy = (ann.start[1] + ann.end[1]) / 2
      const erx = Math.abs(ann.end[0] - ann.start[0]) / 2
      const ery = Math.abs(ann.end[1] - ann.start[1]) / 2
      ctx.beginPath()
      ctx.ellipse(cx, cy, Math.max(erx, 1), Math.max(ery, 1), 0, 0, Math.PI * 2)
      ctx.stroke()
      break
    }
    case 'text': {
      if (!ann.position || !ann.text) return
      ctx.font = `${(ann.fontSize ?? 16) / zoom}px sans-serif`
      ctx.fillText(ann.text, ann.position[0], ann.position[1])
      break
    }
  }
}

// ─── 画布标注：重绘触发 ─────────────────────────────────
watch(() => [props.viewport.panX, props.viewport.panY, props.viewport.zoom], renderCanvasAnnotations)
watch(() => props.canvasAnnotations, renderCanvasAnnotations, { deep: true })

let resizeObserver: ResizeObserver | null = null
onMounted(() => {
  if (canvasRef.value) {
    resizeObserver = new ResizeObserver(renderCanvasAnnotations)
    resizeObserver.observe(canvasRef.value)
  }
  nextTick(renderCanvasAnnotations)
})

onBeforeUnmount(() => {
  resizeObserver?.disconnect()
  window.removeEventListener('mousemove', onGlobalDrawMove)
  window.removeEventListener('mouseup', onGlobalDrawUp)
})

// ─── 键盘事件绑定 ──────────────────────────────────────
// 由父组件（PinboardPage）管理全局键盘事件并传递给 canvas
defineExpose({
  onKeyDown,
  onKeyUp,
})
</script>

<template>
  <div
    ref="canvasRef"
    class="pb-canvas"
    :class="[cursorClass, { 'hide-cursor': showBrushCursor }]"
    @wheel.prevent="onWheel"
    @mousedown="onMouseDown"
    @mousemove="onTrackMouse"
    @mouseenter="mouseInCanvas = true"
    @mouseleave="mouseInCanvas = false"
    @contextmenu.prevent
  >
    <div class="pb-canvas-inner" :style="innerStyle">
      <PinItem
        v-for="pin in sortedPins"
        :key="pin.id"
        :pin="pin"
        :image-url="getImageUrl(pin)"
        :is-selected="pin.id === selectedPinId"
        :active-tool="(activeTool as any)"
        :active-color="activeColor"
        :canvas-zoom="viewport.zoom"
        :style="{ zIndex: pin.zIndex }"
        @select="$emit('select-pin', pin.id)"
        @update-position="(x, y) => $emit('update-pin', pin.id, { x, y })"
        @update-size="(w, h) => $emit('update-pin', pin.id, { width: w, height: h })"
        @delete="$emit('delete-pin', pin.id)"
        @add-annotation="(ann) => $emit('add-annotation', pin.id, ann)"
        @remove-annotation="(idx) => $emit('remove-annotation', pin.id, idx)"
      />
    </div>

    <!-- 画布标注覆盖层 -->
    <canvas
      ref="annotationCanvasRef"
      class="pb-annotation-overlay"
      :class="{ 'drawing-active': canvasDrawingActive }"
      @mousedown="onOverlayMouseDown"
      @mousemove="onOverlayMouseMove"
      @click="onOverlayClick"
    />

    <!-- 笔刷/橡皮擦大小指示器 -->
    <div
      v-if="showBrushCursor"
      class="pb-brush-cursor"
      :style="brushCursorStyle"
    />

    <!-- 画布文字输入 -->
    <input
      v-if="canvasTextVisible"
      ref="canvasTextInputRef"
      v-model="canvasTextValue"
      class="pb-canvas-text-input"
      :style="{ left: canvasTextX + 'px', top: canvasTextY + 'px', fontSize: props.fontSize + 'px' }"
      @keydown="onCanvasTextKeydown"
      @blur="onCanvasTextConfirm"
    />
  </div>
</template>

<style scoped>
.pb-canvas {
  position: absolute;
  inset: 0;
  overflow: hidden;
  background: var(--canvas-bg, var(--bg-secondary));
  cursor: default;
}

.pb-canvas.cursor-grab {
  cursor: grab;
}

.pb-canvas.cursor-grabbing {
  cursor: grabbing;
}

.pb-canvas-inner {
  position: absolute;
  min-width: 10000px;
  min-height: 10000px;
}

/* ─── 画布标注覆盖层 ─── */
.pb-annotation-overlay {
  position: absolute;
  inset: 0;
  z-index: 5;
  pointer-events: none;
}

.pb-annotation-overlay.drawing-active {
  pointer-events: auto;
  cursor: crosshair;
}

/* ─── 笔刷/橡皮擦光标指示器 ─── */
.pb-canvas.hide-cursor,
.pb-canvas.hide-cursor .pb-annotation-overlay.drawing-active {
  cursor: none;
}

.pb-brush-cursor {
  position: absolute;
  border: 1.5px solid rgba(255, 255, 255, 0.85);
  box-shadow: 0 0 0 1px rgba(0, 0, 0, 0.4);
  border-radius: 50%;
  pointer-events: none;
  z-index: 15;
}

/* ─── 画布文字输入 ─── */
.pb-canvas-text-input {
  position: absolute;
  z-index: 10;
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
}
</style>
