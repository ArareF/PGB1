<script setup lang="ts">
import { ref, watch, nextTick, onMounted, onBeforeUnmount } from 'vue'

const props = withDefaults(defineProps<{
  show: boolean
  title: string
  widthPercent?: number
  teleportTarget?: string
  teleportDisabled?: boolean
  hasActions?: boolean
}>(), {
  widthPercent: 30,
  teleportTarget: '#content-row',
  teleportDisabled: false,
  hasActions: false,
})

const emit = defineEmits<{
  'update:widthPercent': [value: number]
}>()

// ─── 全屏 ──────────────────────────────────────────
const isFullscreen = ref(false)
const fsLeft = ref('0px')
const sidebarEl = ref<HTMLElement | null>(null)

async function toggleFullscreen() {
  const el = sidebarEl.value
  if (!isFullscreen.value) {
    const mainEl = document.querySelector('.main-content') as HTMLElement | null
    const crEl = document.getElementById('content-row') as HTMLElement | null
    if (mainEl && crEl) {
      fsLeft.value = `${mainEl.getBoundingClientRect().left - crEl.getBoundingClientRect().left}px`
    } else {
      fsLeft.value = '0px'
    }
  }
  const startRect = el?.getBoundingClientRect()
  isFullscreen.value = !isFullscreen.value
  await nextTick()
  if (!el || !startRect) return
  const endRect = el.getBoundingClientRect()
  const dx = startRect.left - endRect.left
  const dy = startRect.top - endRect.top
  const scaleX = startRect.width / endRect.width
  const scaleY = startRect.height / endRect.height
  el.style.transformOrigin = 'top left'
  el.style.transform = `translate(${dx}px, ${dy}px) scale(${scaleX}, ${scaleY})`
  el.style.transition = 'none'
  void el.offsetWidth
  el.style.transition = `transform var(--duration-normal) var(--ease-out)`
  el.style.transform = ''
  el.addEventListener('transitionend', () => {
    el.style.transform = ''
    el.style.transition = ''
    el.style.transformOrigin = ''
  }, { once: true })
}

function onGlobalKeyDown(e: KeyboardEvent) {
  if (e.key === 'Escape' && isFullscreen.value) {
    isFullscreen.value = false
  }
}

onMounted(() => window.addEventListener('keydown', onGlobalKeyDown))
onBeforeUnmount(() => {
  window.removeEventListener('keydown', onGlobalKeyDown)
  resizeCleanup?.()
})

// ─── 拖拽调整宽度 ────────────────────────────────────
const isResizing = ref(false)
const SIDEBAR_WIDTH_KEY = 'pgb1-sidebar-width'
const savedWidth = parseFloat(localStorage.getItem(SIDEBAR_WIDTH_KEY) || '')
const currentWidth = ref(isFinite(savedWidth) ? savedWidth : (props.widthPercent ?? 30))

watch(() => props.widthPercent, (v) => {
  if (v != null) currentWidth.value = v
})

let resizeCleanup: (() => void) | null = null

function startResize(e: MouseEvent) {
  e.preventDefault()
  isResizing.value = true
  const startX = e.clientX
  const startWidth = currentWidth.value
  function onMouseMove(ev: MouseEvent) {
    const windowWidth = window.innerWidth
    const deltaPercent = ((startX - ev.clientX) / windowWidth) * 100
    currentWidth.value = Math.min(60, Math.max(20, startWidth + deltaPercent))
    emit('update:widthPercent', currentWidth.value)
  }
  function onMouseUp() {
    isResizing.value = false
    localStorage.setItem(SIDEBAR_WIDTH_KEY, String(currentWidth.value))
    window.removeEventListener('mousemove', onMouseMove)
    window.removeEventListener('mouseup', onMouseUp)
    resizeCleanup = null
  }
  window.addEventListener('mousemove', onMouseMove)
  window.addEventListener('mouseup', onMouseUp)
  resizeCleanup = onMouseUp
}

defineExpose({ isFullscreen, toggleFullscreen })
</script>

<template>
  <Teleport :to="teleportTarget" :disabled="teleportDisabled">
    <Transition name="sidebar-shell">
      <div
        v-if="show"
        ref="sidebarEl"
        class="sidebar-shell"
        :class="{ 'is-resizing': isResizing, 'is-fullscreen': isFullscreen }"
        :style="isFullscreen ? { left: fsLeft } : { width: currentWidth + '%' }"
      >
        <div class="sidebar-shell__resize" @mousedown="startResize" />

        <div class="sidebar-shell__header">
          <span class="sidebar-shell__title">{{ title }}</span>
        </div>

        <div class="sidebar-shell__body" :class="{ 'has-actions': hasActions }">
          <slot :is-fullscreen="isFullscreen" :toggle-fullscreen="toggleFullscreen" />
        </div>

        <div v-if="$slots.actions" class="sidebar-actions">
          <slot name="actions" />
        </div>

        <slot name="overlay" />
      </div>
    </Transition>
  </Teleport>
</template>

<style>
/* ─── 容器 ─── */
.sidebar-shell {
  position: relative;
  min-height: 0;
  display: flex;
  flex-direction: column;
  border-radius: var(--floating-main-radius);
  padding: var(--floating-main-padding);
  overflow: hidden;
  flex-shrink: 0;
  /* 手动复刻 glass-strong，不用 backdrop-filter（兄弟冲突） */
  background: var(--glass-strong-bg);
  border: var(--glass-strong-border);
  box-shadow: var(--glass-strong-shadow);
}

.sidebar-shell.is-resizing {
  user-select: none;
}

/* ─── 拖拽把手 ─── */
.sidebar-shell__resize {
  position: absolute;
  top: 0;
  left: 0;
  width: 4px;
  height: 100%;
  cursor: col-resize;
  z-index: 10;
}

.sidebar-shell__resize:hover,
.sidebar-shell__resize:active {
  background: var(--color-primary);
  opacity: 0.5;
}

/* ─── 进出场动画 ─── */
.sidebar-shell-enter-active {
  transition: transform var(--duration-normal) var(--ease-slide-in),
              width var(--duration-normal) var(--ease-slide-in);
  overflow: hidden;
}
.sidebar-shell-leave-active {
  transition: transform var(--duration-fast) var(--ease-slide-out),
              width var(--duration-fast) var(--ease-slide-out);
  overflow: hidden;
}
.sidebar-shell-enter-from,
.sidebar-shell-leave-to {
  transform: translateX(100%);
  width: 0 !important;
}

/* ─── 标题 ─── */
.sidebar-shell__header {
  display: flex;
  align-items: center;
  padding-bottom: var(--spacing-3);
  border-bottom: 1px solid var(--border-medium);
  flex-shrink: 0;
}

.sidebar-shell__title {
  font-size: var(--text-2xl);
  font-weight: var(--font-weight-heading);
  color: var(--text-primary);
}

/* ─── body 滚动区 ─── */
.sidebar-shell__body {
  flex: 1;
  overflow-y: auto;
  padding-top: var(--spacing-4);
  display: flex;
  flex-direction: column;
  gap: var(--spacing-4);
  min-height: 0;
}

/* 有底部操作按钮时留出空间 */
.sidebar-shell__body.has-actions {
  padding-bottom: calc(var(--button-md-height) + var(--spacing-4) * 2);
}

/* ─── 页面内全屏模式 ─── */
.sidebar-shell.is-fullscreen {
  position: absolute;
  top: 0;
  right: 0;
  bottom: 0;
  width: auto !important;
  z-index: 1;
  border-radius: var(--floating-main-radius);
  box-shadow: none;
  padding: 0;
}

.sidebar-shell.is-fullscreen .sidebar-shell__resize,
.sidebar-shell.is-fullscreen .sidebar-shell__header,
.sidebar-shell.is-fullscreen .sidebar-actions {
  display: none;
}

.sidebar-shell.is-fullscreen .sidebar-shell__body {
  display: contents;
}

/* ─── 侧边栏内通用信息展示样式 ─── */
.sidebar-shell .sidebar-section {
  display: flex;
  flex-direction: column;
  gap: var(--spacing-2);
}

.sidebar-shell .section-title {
  font-size: var(--text-base);
  font-weight: var(--font-weight-heading);
  color: var(--text-secondary);
}

.sidebar-shell .info-list {
  display: flex;
  flex-direction: column;
  gap: var(--spacing-2);
}

.sidebar-shell .info-row {
  display: flex;
  justify-content: space-between;
  align-items: flex-start;
  gap: var(--spacing-3);
}

.sidebar-shell .info-label {
  font-size: var(--text-sm);
  color: var(--text-tertiary);
  flex-shrink: 0;
}

.sidebar-shell .info-value {
  font-size: var(--text-sm);
  color: var(--text-primary);
  text-align: right;
  word-break: break-all;
}

/* ─── 版本列表 ─── */
.sidebar-shell .version-list {
  display: flex;
  flex-direction: column;
  gap: var(--spacing-2);
}

.sidebar-shell .version-card {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: var(--spacing-3);
  border-radius: var(--radius-md);
  border: 1px solid var(--border-light);
  background: transparent;
  cursor: pointer;
  transition: all var(--transition-fast);
}

.sidebar-shell .version-card:hover {
  background: var(--bg-hover);
  border-color: var(--border-medium);
}

.sidebar-shell .version-card.active {
  background: var(--bg-selected);
  border-color: var(--color-primary);
}

.sidebar-shell .version-card-left {
  display: flex;
  flex-direction: column;
  gap: var(--spacing-1);
  min-width: 0;
}

.sidebar-shell .version-name {
  font-size: var(--text-base);
  font-weight: var(--font-weight-heading);
  color: var(--text-primary);
}

.sidebar-shell .version-card.active .version-name {
  color: var(--color-primary);
}

.sidebar-shell .version-meta {
  font-size: var(--text-sm);
  color: var(--text-tertiary);
}

.sidebar-shell .version-card-right {
  display: flex;
  align-items: center;
  gap: var(--spacing-2);
  flex-shrink: 0;
}

.sidebar-shell .version-ext {
  font-size: var(--text-sm);
  font-weight: var(--font-medium);
  color: var(--text-secondary);
}

.sidebar-shell .version-folder-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 28px;
  height: 28px;
  border-radius: 50%;
  border: 1px solid var(--border-medium);
  background: transparent;
  color: var(--text-tertiary);
  cursor: pointer;
  transition: all var(--transition-fast);
}

.sidebar-shell .version-folder-btn:hover {
  background: var(--color-primary);
  color: var(--color-neutral-0);
  border-color: var(--color-primary);
}
</style>
