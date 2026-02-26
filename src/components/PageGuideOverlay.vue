<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import type { GuideAnnotation } from '../config/onboarding'

defineProps<{
  show: boolean
  annotations: GuideAnnotation[]
}>()

const emit = defineEmits<{
  close: []
}>()

const { t } = useI18n()
</script>

<template>
  <Teleport to="body">
    <Transition name="guide">
      <div v-if="show" class="guide-overlay" @click="emit('close')">
        <p class="guide-close-hint">{{ t('pageGuide.closeHint') }}</p>
        <div
          v-for="ann in annotations"
          :key="ann.id"
          class="guide-bubble"
          :class="ann.arrowDirection ? `arrow-${ann.arrowDirection}` : ''"
          :style="{ top: ann.top, left: ann.left }"
        >
          {{ t(ann.labelKey) }}
        </div>
      </div>
    </Transition>
  </Teleport>
</template>

<style scoped>
.guide-overlay {
  position: fixed;
  inset: 0;
  z-index: var(--z-modal-backdrop);
  background: var(--overlay-backdrop);
  cursor: pointer;
}

.guide-close-hint {
  position: fixed;
  bottom: var(--spacing-6);
  left: 50%;
  transform: translateX(-50%);
  color: rgba(255, 255, 255, 0.6);
  font-size: var(--text-sm);
  pointer-events: none;
  z-index: var(--z-modal);
}

.guide-bubble {
  position: fixed;
  padding: var(--spacing-2) var(--spacing-4);
  background: var(--glass-strong-bg);
  border: var(--glass-strong-border);
  box-shadow: var(--glass-strong-shadow);
  border-radius: var(--radius-lg);
  color: var(--text-primary);
  font-size: var(--text-sm);
  line-height: 1.5;
  white-space: pre;
  text-align: left;
  pointer-events: none;
  z-index: var(--z-modal);
  transform: translate(-50%, -50%);
}

/* 箭头 — 伪元素三角 */
.guide-bubble::before {
  content: '';
  position: absolute;
  width: 0;
  height: 0;
  border: 6px solid transparent;
}

/* 箭头朝上 → 气泡在目标下方 */
.guide-bubble.arrow-up::before {
  bottom: 100%;
  left: 50%;
  transform: translateX(-50%);
  border-bottom-color: var(--border-medium);
}

/* 箭头朝下 → 气泡在目标上方 */
.guide-bubble.arrow-down::before {
  top: 100%;
  left: 50%;
  transform: translateX(-50%);
  border-top-color: var(--border-medium);
}

/* 箭头朝左 → 气泡在目标右方 */
.guide-bubble.arrow-left::before {
  right: 100%;
  top: 50%;
  transform: translateY(-50%);
  border-right-color: var(--border-medium);
}

/* 箭头朝右 → 气泡在目标左方 */
.guide-bubble.arrow-right::before {
  left: 100%;
  top: 50%;
  transform: translateY(-50%);
  border-left-color: var(--border-medium);
}

/* 进出场动画 */
.guide-enter-active {
  transition: opacity var(--transition-normal);
}
.guide-leave-active {
  transition: opacity var(--transition-fast);
}
.guide-enter-from,
.guide-leave-to {
  opacity: 0;
}
</style>
