<script setup lang="ts">
import { ref, watch, onUnmounted } from 'vue'
import { toggleCheckbox } from '../composables/useNotes'
import NoteRenderer from './NoteRenderer.vue'

const props = defineProps<{
  target: HTMLElement | null
  text: string
}>()

const emit = defineEmits<{
  /** checkbox 切换后保存新文本 */
  save: [text: string]
}>()

/** 本地副本，支持 checkbox 即时切换 */
const localText = ref(props.text)
watch(() => props.text, (val) => { localText.value = val })

function onCheckboxToggle(lineIndex: number) {
  localText.value = toggleCheckbox(localText.value, lineIndex)
  emit('save', localText.value)
}

const visible = ref(false)
const posX = ref(0)
const posY = ref(0)
let showTimer: ReturnType<typeof setTimeout> | null = null
let hideTimer: ReturnType<typeof setTimeout> | null = null

/** 悬停桥接延迟（ms）：鼠标从卡片移到 tooltip 的缓冲时间 */
const BRIDGE_DELAY = 150

function clearTimers() {
  if (showTimer) { clearTimeout(showTimer); showTimer = null }
  if (hideTimer) { clearTimeout(hideTimer); hideTimer = null }
}

function onCardEnter() {
  if (!props.text) return
  // 取消待执行的隐藏
  if (hideTimer) { clearTimeout(hideTimer); hideTimer = null }
  showTimer = setTimeout(() => {
    updatePosition()
    visible.value = true
  }, 300)
}

function onCardLeave() {
  if (showTimer) { clearTimeout(showTimer); showTimer = null }
  // 延迟隐藏，允许鼠标桥接到 tooltip
  hideTimer = setTimeout(() => {
    visible.value = false
  }, BRIDGE_DELAY)
}

function onTooltipEnter() {
  if (hideTimer) { clearTimeout(hideTimer); hideTimer = null }
}

function onTooltipLeave() {
  visible.value = false
}

function updatePosition() {
  if (!props.target) return
  const rect = props.target.getBoundingClientRect()
  posX.value = rect.left + rect.width / 2
  // 定位在卡片上方
  posY.value = rect.top - 8
}

// 监听 target 变化，绑定/解绑事件
let prevTarget: HTMLElement | null = null
watch(() => props.target, (el, oldEl) => {
  if (oldEl) {
    oldEl.removeEventListener('mouseenter', onCardEnter)
    oldEl.removeEventListener('mouseleave', onCardLeave)
  }
  if (el) {
    el.addEventListener('mouseenter', onCardEnter)
    el.addEventListener('mouseleave', onCardLeave)
  }
  prevTarget = el
}, { immediate: true })

onUnmounted(() => {
  clearTimers()
  if (prevTarget) {
    prevTarget.removeEventListener('mouseenter', onCardEnter)
    prevTarget.removeEventListener('mouseleave', onCardLeave)
  }
})
</script>

<template>
  <Teleport to="body">
    <Transition name="tooltip">
      <div
        v-if="visible && text"
        class="note-tooltip"
        :style="{
          left: posX + 'px',
          top: posY + 'px',
        }"
        @mouseenter="onTooltipEnter"
        @mouseleave="onTooltipLeave"
      >
        <NoteRenderer :text="localText" @toggle-checkbox="onCheckboxToggle" />
      </div>
    </Transition>
  </Teleport>
</template>

<style>
.note-tooltip {
  position: fixed;
  transform: translateX(-50%) translateY(-100%);
  max-width: 320px;
  max-height: 200px;
  overflow-y: auto;
  padding: var(--spacing-2) var(--spacing-3);
  border-radius: var(--radius-md);
  background: var(--glass-strong-bg);
  border: var(--glass-strong-border);
  box-shadow: var(--glass-strong-shadow);
  backdrop-filter: blur(var(--glass-strong-blur));
  -webkit-backdrop-filter: blur(var(--glass-strong-blur));
  color: var(--text-primary);
  font-size: var(--text-xs);
  line-height: var(--leading-normal);
  z-index: var(--z-tooltip);
}

.tooltip-enter-active {
  transition: opacity var(--duration-fast) var(--ease-out),
              transform var(--duration-fast) var(--ease-out);
}
.tooltip-leave-active {
  transition: opacity var(--duration-fast) var(--ease-in),
              transform var(--duration-fast) var(--ease-in);
}
.tooltip-enter-from,
.tooltip-leave-to {
  opacity: 0;
  transform: translateX(-50%) translateY(calc(-100% + 4px));
}
</style>
