<script setup lang="ts">
import { ref, computed, watch, nextTick } from 'vue'
import type { NavAction } from '../composables/useNavigation'
import { useNavigation } from '../composables/useNavigation'
import StatusBar from './StatusBar.vue'

const { title, showBackButton, actions, goBack, routeDirection } = useNavigation()

// 与页面切换同向：前进从右，返回从左
const navTransition = computed(() =>
  routeDirection.value === 'back' ? 'nav-back' : 'nav-forward'
)
// actions 用于 key，方向变化时整组替换
const actionsKey = computed(() => actions.value.map(a => a.id).join(','))

// 容器宽度 FLIP 动画
const leftIslandRef = ref<HTMLElement>()
const centerIslandRef = ref<HTMLElement>()

// 导航状态合并 key（title / back / actions 任一变化即触发）
const navStateKey = computed(() =>
  `${title.value}|${showBackButton.value}|${actionsKey.value}`
)

let savedLeft = 0
let savedCenter = 0

// pre：DOM 更新前记录旧宽度
watch(navStateKey, () => {
  savedLeft = leftIslandRef.value?.offsetWidth ?? 0
  savedCenter = centerIslandRef.value?.offsetWidth ?? 0
}, { flush: 'pre' })

// post：DOM 更新后执行 FLIP
watch(navStateKey, async () => {
  await nextTick()
  flipWidth(leftIslandRef.value, savedLeft)
  flipWidth(centerIslandRef.value, savedCenter)
}, { flush: 'post' })

function flipWidth(el: HTMLElement | undefined, fromWidth: number) {
  if (!el || fromWidth === 0) return

  // 先清除上一次 FLIP 可能残留的内联样式，确保读到元素的真实自然宽度
  // 若不清除：上次动画未结束时 el.style.width 仍为旧值，offsetWidth 读到错误值
  // 导致 fromWidth ≈ toWidth，提前 return，岛宽永久卡死
  el.style.transition = 'none'
  el.style.width = ''
  el.style.overflow = ''

  const toWidth = el.offsetWidth   // offsetWidth 强制 layout，此时读到正确自然宽度
  if (Math.abs(fromWidth - toWidth) < 1) return

  // 锁定旧宽度，下一帧过渡到新宽度
  el.style.width = `${fromWidth}px`
  el.style.overflow = 'hidden'

  requestAnimationFrame(() => {
    requestAnimationFrame(() => {
      el.style.transition = `width var(--duration-dialog) var(--ease-out)`
      el.style.width = `${toWidth}px`
    })
  })

  el.addEventListener('transitionend', (e) => {
    if ((e as TransitionEvent).propertyName === 'width') {
      el.style.width = ''
      el.style.transition = ''
      el.style.overflow = ''
    }
  }, { once: true })
}

const LONG_PRESS_MS = 500

let longPressTimer: ReturnType<typeof setTimeout> | null = null
let longPressTriggered = false

function onActionPointerDown(e: PointerEvent, action: NavAction) {
  if (!action.onLongPress || e.button !== 0) return
  const btnEl = e.currentTarget as HTMLElement
  longPressTriggered = false
  longPressTimer = setTimeout(() => {
    longPressTriggered = true
    action.onLongPress!(btnEl.getBoundingClientRect())
  }, LONG_PRESS_MS)
}

function onActionPointerUp() {
  if (longPressTimer !== null) {
    clearTimeout(longPressTimer)
    longPressTimer = null
  }
}

function onActionClick(e: MouseEvent, action: NavAction) {
  if (action.onLongPress && longPressTriggered) {
    // 长按已触发，吞掉这次 click
    e.stopPropagation()
    return
  }
  action.handler()
}

function onActionsWheel(e: WheelEvent) {
  const el = e.currentTarget as HTMLElement
  el.scrollLeft += e.deltaY || e.deltaX
}
</script>

<template>
  <header class="title-bar" data-tauri-drag-region>
    <!-- 左侧悬浮岛：返回按钮 + 标题 -->
    <div ref="leftIslandRef" class="title-bar-left glass-medium" :class="{ 'has-back': showBackButton }">
      <Transition :name="navTransition">
        <button
          v-if="showBackButton"
          class="back-btn"
          title="返回"
          @click="goBack"
        >
          <svg width="40" height="40" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <polyline points="15 18 9 12 15 6" />
          </svg>
        </button>
      </Transition>
      <Transition :name="navTransition">
        <span :key="title" class="title-text">{{ title }}</span>
      </Transition>
    </div>

    <!-- 中部悬浮岛：状态栏 + 快捷功能区 -->
    <div ref="centerIslandRef" class="title-bar-center glass-medium">
      <StatusBar />
      <Transition :name="navTransition">
        <div v-if="actions.length > 0" :key="actionsKey" class="title-bar-actions-wrap">
          <div class="title-bar-divider" />
          <div class="title-bar-actions" @wheel.prevent="onActionsWheel">
            <button
              v-for="action in actions"
              :key="action.id"
              class="action-btn"
              :class="{ 'action-btn--active': action.active }"
              :title="action.onLongPress ? action.label + '（长按选择版本）' : action.label"
              :disabled="action.disabled"
              @pointerdown="onActionPointerDown($event, action)"
              @pointerup="onActionPointerUp"
              @pointerleave="onActionPointerUp"
              @click="onActionClick($event, action)"
            >
              {{ action.label }}
            </button>
          </div>
        </div>
      </Transition>
    </div>
  </header>
</template>

<style scoped>
.title-bar {
  display: flex;
  align-items: flex-end;
  gap: var(--spacing-8);
  padding: 0;
  width: 100%;
  /* 允许拖拽移动窗口 */
  -webkit-app-region: drag;
}

/* 悬浮岛内的交互元素不参与拖拽 */
.title-bar-left,
.title-bar-center {
  -webkit-app-region: no-drag;
}

.title-bar-left {
  display: flex;
  align-items: center;
  gap: 0;
  padding: 0 var(--spacing-8) 0 var(--spacing-8);
  border-radius: var(--floating-navbar-radius);
  flex-shrink: 0;
  height: var(--floating-navbar-height);
  overflow: hidden;
  margin-top: var(--spacing-3);
  position: relative;
}

/* 有返回按钮时去掉左侧 padding，按钮紧贴边缘 */
.title-bar-left.has-back {
  padding-left: 0;
}

.title-text {
  font-size: var(--text-4xl);
  font-weight: var(--font-weight-body);
  color: var(--text-primary);
  white-space: nowrap;
}

.back-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  align-self: stretch;
  aspect-ratio: 1;
  border: none;
  background: var(--bg-hover);
  color: var(--text-primary);
  border-radius: var(--floating-navbar-radius) 0 0 var(--floating-navbar-radius);
  cursor: pointer;
  transition: var(--transition-all);
  flex-shrink: 0;
  margin-right: var(--spacing-4);
}

.back-btn:hover {
  background: var(--glass-subtle-bg);
}

.title-bar-center {
  display: flex;
  align-items: center;
  gap: var(--spacing-3);
  padding: var(--spacing-3) var(--spacing-4);
  border-radius: var(--floating-navbar-radius);
  flex-shrink: 1;
  min-width: 0;
  margin-left: auto;
  height: var(--floating-navbar-center-height);
  margin-top: var(--spacing-3);
  /* glass-medium 的 overflow:hidden 会裁切 action-btn hover 时的 translateY(-2px)，覆盖为 visible */
  overflow: visible;
}

.title-bar-actions {
  display: flex;
  align-items: center;
  gap: var(--spacing-3);
  overflow-x: auto;
  flex-shrink: 1;
  min-width: 0;
  /* overflow-x:auto 强制 overflow-y:auto，会裁切 translateY(-2px) 的绘制区域。
     padding-block 在 border edge 和内容之间建立缓冲区，hover 上浮后仍在 padding 内不被裁切。
     顶底对称确保按钮视觉居中不变。 */
  padding-block: 4px;
}

.title-bar-actions::-webkit-scrollbar {
  display: none;
}

/* 隐藏快捷功能区的滚动条（保持简洁） */
.title-bar-center::-webkit-scrollbar {
  display: none;
}

.title-bar-divider {
  width: 1px;
  align-self: stretch;
  background: var(--border-light);
  flex-shrink: 0;
  margin: var(--spacing-2) 0;
}

.action-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  padding: var(--spacing-2) var(--spacing-4);
  border: 1px solid transparent;
  background: var(--bg-hover);
  color: var(--text-primary);
  font-size: var(--text-2xl);
  border-radius: var(--radius-button);
  cursor: pointer;
  transition: background var(--duration-fast) var(--ease-out),
              border-color var(--duration-fast) var(--ease-out),
              transform var(--duration-fast) var(--ease-out),
              box-shadow var(--duration-fast) var(--ease-out),
              color var(--duration-fast) var(--ease-out);
  white-space: nowrap;
  flex-shrink: 0;
}

.action-btn:hover:not(:disabled) {
  background: var(--bg-active);
  color: var(--text-primary);
  border-color: var(--border-medium);
  transform: translateY(-2px);
  box-shadow: 0 3px 10px rgba(0, 0, 0, 0.30);
}

.action-btn:active:not(:disabled) {
  transform: translateY(0);
  box-shadow: none;
}

.action-btn:disabled {
  opacity: var(--button-disabled-opacity);
  cursor: not-allowed;
}

.action-btn--active {
  background: color-mix(in srgb, var(--color-primary-500) 15%, transparent);
  color: var(--color-primary-500);
  box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--color-primary-500) 40%, transparent);
}

.action-btn--active:hover:not(:disabled) {
  background: color-mix(in srgb, var(--color-primary-500) 25%, transparent);
  color: var(--color-primary-500);
  border-color: color-mix(in srgb, var(--color-primary-500) 50%, transparent);
  box-shadow: 0 3px 10px rgba(0, 0, 0, 0.25),
              inset 0 0 0 1px color-mix(in srgb, var(--color-primary-500) 40%, transparent);
  transform: translateY(-2px);
}

.title-bar-actions-wrap {
  display: flex;
  align-items: center;
  gap: var(--spacing-3);
  min-width: 0;
  flex-shrink: 1;
}

/* 返回按钮 leave 时保持原始尺寸/位置，
   防止 absolute 脱离 flex 后 align-self:stretch 失效导致向上跳动 */
.nav-back-leave-active.back-btn,
.nav-forward-leave-active.back-btn {
  top: 0;
  bottom: 0;
}

/* 导航动画 — 前进：从右滑入 */
.nav-forward-enter-active {
  transition: transform var(--duration-dialog) var(--ease-out),
              opacity var(--duration-dialog) var(--ease-out);
}
.nav-forward-leave-active {
  transition: transform var(--duration-dialog) var(--ease-out),
              opacity var(--duration-dialog) var(--ease-out);
  position: absolute;
  pointer-events: none;
}
.nav-forward-enter-from {
  transform: translateX(20px);
  opacity: 0;
}
.nav-forward-leave-to {
  transform: translateX(-20px);
  opacity: 0;
}

/* 导航动画 — 返回：从左滑入 */
.nav-back-enter-active {
  transition: transform var(--duration-dialog) var(--ease-out),
              opacity var(--duration-dialog) var(--ease-out);
}
.nav-back-leave-active {
  transition: transform var(--duration-dialog) var(--ease-out),
              opacity var(--duration-dialog) var(--ease-out);
  position: absolute;
  pointer-events: none;
}
.nav-back-enter-from {
  transform: translateX(-20px);
  opacity: 0;
}
.nav-back-leave-to {
  transform: translateX(20px);
  opacity: 0;
}
</style>
