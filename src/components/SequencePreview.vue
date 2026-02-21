<script setup lang="ts">
import { ref, onMounted, onUnmounted } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { loadSequenceFrames } from '../composables/useFrameCache'

const props = defineProps<{
  folderPath: string
  fps?: number
  maxWidth?: number
  transparent?: boolean
}>()

const canvasRef = ref<HTMLCanvasElement | null>(null)
const loaded = ref(false)

let frames: HTMLImageElement[] = []
let currentFrame = 0
let animationId: number | null = null
let lastFrameTime = 0
const frameInterval = 1000 / (props.fps ?? 24)

async function init() {
  const filePaths = await invoke<string[]>('list_sequence_frames', { dirPath: props.folderPath })
  if (filePaths.length === 0) return

  frames = await loadSequenceFrames(props.folderPath, filePaths, props.maxWidth ?? 200)
  loaded.value = true
  play()
}

function drawFrame(index: number) {
  const canvas = canvasRef.value
  if (!canvas || frames.length === 0) return

  const ctx = canvas.getContext('2d')
  if (!ctx) return

  const img = frames[index]
  canvas.width = canvas.clientWidth
  canvas.height = canvas.clientHeight

  // 背景（透明或黑色）
  if (props.transparent) {
    ctx.clearRect(0, 0, canvas.width, canvas.height)
  } else {
    const rootStyle = getComputedStyle(document.documentElement)
    ctx.fillStyle = rootStyle.getPropertyValue('--canvas-bg').trim() || '#0C0D10'
    ctx.fillRect(0, 0, canvas.width, canvas.height)
  }

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

function stop() {
  if (animationId !== null) {
    cancelAnimationFrame(animationId)
    animationId = null
  }
}

onMounted(init)
onUnmounted(stop)
</script>

<template>
  <canvas
    ref="canvasRef"
    class="sequence-canvas"
    :class="{ 'sequence-canvas--transparent': transparent }"
  />
</template>

<style scoped>
.sequence-canvas {
  width: 100%;
  height: 100%;
  display: block;
}

.sequence-canvas--transparent {
  background-image:
    linear-gradient(45deg, #808080 25%, transparent 25%),
    linear-gradient(-45deg, #808080 25%, transparent 25%),
    linear-gradient(45deg, transparent 75%, #808080 75%),
    linear-gradient(-45deg, transparent 75%, #808080 75%);
  background-size: 16px 16px;
  background-position: 0 0, 0 8px, 8px -8px, -8px 0px;
  background-color: #b0b0b0;
}
</style>
