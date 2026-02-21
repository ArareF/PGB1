<script setup lang="ts">
import { ref, watch } from 'vue'

const props = defineProps<{
  src: string
  alt?: string
}>()

const scale   = ref(1)
const offsetX = ref(0)
const offsetY = ref(0)
const isDragging = ref(false)

// 切换图片时重置
watch(() => props.src, () => {
  scale.value   = 1
  offsetX.value = 0
  offsetY.value = 0
})

function onWheel(e: WheelEvent) {
  e.preventDefault()
  const delta = e.deltaY > 0 ? 0.9 : 1.1
  scale.value = Math.min(10, Math.max(0.1, scale.value * delta))
}

function onMouseDown(e: MouseEvent) {
  if (e.button !== 0) return
  isDragging.value = true
  const startX = e.clientX - offsetX.value
  const startY = e.clientY - offsetY.value

  function onMove(ev: MouseEvent) {
    offsetX.value = ev.clientX - startX
    offsetY.value = ev.clientY - startY
  }
  function onUp() {
    isDragging.value = false
    window.removeEventListener('mousemove', onMove)
    window.removeEventListener('mouseup', onUp)
  }
  window.addEventListener('mousemove', onMove)
  window.addEventListener('mouseup', onUp)
}

function reset() {
  scale.value   = 1
  offsetX.value = 0
  offsetY.value = 0
}

const isTransformed = () => scale.value !== 1 || offsetX.value !== 0 || offsetY.value !== 0
</script>

<template>
  <div
    class="image-viewer"
    :class="{ dragging: isDragging }"
    @wheel.prevent="onWheel"
    @mousedown="onMouseDown"
  >
    <img
      :src="src"
      :alt="alt"
      class="image-viewer-img"
      :style="{ transform: `translate(${offsetX}px, ${offsetY}px) scale(${scale})` }"
      draggable="false"
    />
    <button
      v-if="isTransformed()"
      class="image-viewer-reset"
      @click.stop="reset"
    >
      重置
    </button>
  </div>
</template>

<style scoped>
.image-viewer {
  position: relative;
  width: 100%;
  height: 100%;
  display: flex;
  align-items: center;
  justify-content: center;
  overflow: hidden;
  cursor: grab;
  user-select: none;
}

.image-viewer.dragging {
  cursor: grabbing;
}

.image-viewer-img {
  max-width: 100%;
  max-height: 100%;
  object-fit: contain;
  transform-origin: center center;
  pointer-events: none;
}

.image-viewer-reset {
  position: absolute;
  bottom: var(--spacing-2);
  right: var(--spacing-2);
  padding: var(--spacing-1) var(--spacing-3);
  border-radius: var(--radius-md);
  border: 1px solid var(--border-medium);
  background: var(--glass-medium-bg);
  backdrop-filter: blur(var(--glass-subtle-blur));
  -webkit-backdrop-filter: blur(var(--glass-subtle-blur));
  color: var(--text-secondary);
  font-size: var(--text-xs);
  font-family: inherit;
  cursor: pointer;
  transition: all var(--transition-fast);
}

.image-viewer-reset:hover {
  color: var(--text-primary);
}
</style>
