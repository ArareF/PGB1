<script setup lang="ts">
import { ref } from 'vue'
import { useI18n } from 'vue-i18n'

const props = defineProps<{
  show: boolean
  enabledSubtasks: string[]
  completedSubtasks: Set<string>
  autoPrompt: boolean
  revertPrompt: boolean
  /** 自动触发弹窗时，需要有变更才能确认 */
  hasChanges: boolean
}>()

const emit = defineEmits<{
  close: []
  toggle: [subtaskKey: string]
  skip: []
}>()

const { t } = useI18n()

/** 是否处于自动触发状态（禁止直接关闭） */
const isAutoTriggered = () => props.autoPrompt || props.revertPrompt

/* ─── 弹窗内部交互 ─── */
const dialogShaking = ref(false)
let skipPressTimer: ReturnType<typeof setTimeout> | null = null

/** 关闭弹窗（自动触发时摇晃提示） */
function closeDialog() {
  if (isAutoTriggered() && !props.hasChanges) {
    dialogShaking.value = true
    setTimeout(() => { dialogShaking.value = false }, 300)
    return
  }
  emit('close')
}

/** 跳过按钮：长按期间持续抖动，满 1.5s 触发跳过 */
function onSkipMouseDown() {
  dialogShaking.value = true
  skipPressTimer = setTimeout(() => {
    skipPressTimer = null
    dialogShaking.value = false
    emit('skip')
  }, 1500)
}

function stopSkipPress() {
  if (skipPressTimer) {
    clearTimeout(skipPressTimer)
    skipPressTimer = null
  }
  dialogShaking.value = false
}
</script>

<template>
  <Teleport to="body">
    <div v-if="show" class="subtask-overlay">
      <div class="subtask-dialog glass-strong" :class="{ 'dialog-shake': dialogShaking }">
        <p class="subtask-title">{{ t('task.subtaskProgress') }}</p>
        <p v-if="autoPrompt" class="subtask-hint">{{ t('task.allUploadedHint') }}</p>
        <p v-else-if="revertPrompt" class="subtask-hint">{{ t('task.partialUploadHint') }}</p>
        <div class="subtask-list">
          <label
            v-for="key in enabledSubtasks"
            :key="key"
            class="subtask-row"
            @click.prevent="emit('toggle', key)"
          >
            <span
              class="subtask-checkbox"
              :class="{ checked: completedSubtasks.has(key) }"
            />
            <span class="subtask-name">{{ key.split('/')[1] }}</span>
          </label>
        </div>
        <div class="subtask-actions">
          <span
            v-if="isAutoTriggered()"
            class="subtask-skip-btn"
            @mousedown.prevent="onSkipMouseDown"
            @mouseup="stopSkipPress"
            @mouseleave="stopSkipPress"
          >{{ t('common.skip') }}</span>
          <button
            class="subtask-close-btn"
            :disabled="isAutoTriggered() && !hasChanges"
            @click="closeDialog"
          >
            {{ isAutoTriggered() ? t('common.confirm') : t('common.close') }}
          </button>
        </div>
      </div>
    </div>
  </Teleport>
</template>

<style scoped>
/* ─── 子任务弹窗 ─── */
.subtask-overlay {
  position: fixed;
  inset: 0;
  z-index: var(--z-modal, 1000);
  display: flex;
  align-items: center;
  justify-content: center;
  background: var(--overlay-backdrop);
  backdrop-filter: blur(var(--glass-light-blur));
}

.subtask-dialog {
  min-width: 320px;
  max-width: 320px;
  border-radius: var(--floating-navbar-radius);
  padding: var(--spacing-6);
  display: flex;
  flex-direction: column;
  gap: var(--spacing-5);
}

.subtask-title {
  font-size: var(--text-2xl);
  font-weight: var(--font-weight-heading);
  color: var(--text-primary);
}

.subtask-hint {
  font-size: var(--text-base);
  color: var(--text-secondary);
}

.subtask-list {
  display: flex;
  flex-direction: column;
  gap: var(--spacing-2);
}

.subtask-row {
  display: flex;
  align-items: center;
  gap: var(--spacing-3);
  padding: var(--spacing-2) var(--spacing-3);
  border-radius: var(--radius-md);
  cursor: pointer;
  transition: background var(--transition-fast);
}

.subtask-row:hover {
  background: var(--bg-hover);
}

.subtask-checkbox {
  width: 18px;
  height: 18px;
  min-width: 18px;
  border-radius: var(--radius-sm);
  border: 2px solid var(--border-medium);
  flex-shrink: 0;
  position: relative;
  transition: all var(--transition-fast);
}

.subtask-checkbox.checked {
  background: color-mix(in srgb, var(--color-primary-500) 75%, transparent);
  border-color: color-mix(in srgb, var(--color-primary-500) 75%, transparent);
  backdrop-filter: blur(var(--glass-light-blur));
  -webkit-backdrop-filter: blur(var(--glass-light-blur));
}

.subtask-checkbox.checked::after {
  content: '';
  position: absolute;
  top: 2px;
  left: 5px;
  width: 4px;
  height: 8px;
  border: solid var(--color-neutral-0);
  border-width: 0 2px 2px 0;
  transform: rotate(45deg);
}

.subtask-name {
  font-size: var(--text-base);
  color: var(--text-primary);
}

.subtask-actions {
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.subtask-skip-btn {
  font-size: var(--text-xs);
  color: var(--text-quaternary, var(--text-tertiary));
  opacity: 0.5;
  cursor: pointer;
  user-select: none;
  transition: opacity var(--transition-fast);
}

.subtask-skip-btn:hover {
  opacity: 0.7;
}

.subtask-skip-btn:active {
  opacity: 1;
}

@keyframes dialog-shake {
  0%, 100% { transform: translate(0, 0); }
  15% { transform: translate(-4px, 0); }
  30% { transform: translate(4px, 0); }
  45% { transform: translate(-3px, 0); }
  60% { transform: translate(3px, 0); }
  75% { transform: translate(-2px, 0); }
  90% { transform: translate(2px, 0); }
}

.dialog-shake {
  animation: dialog-shake 0.3s ease infinite;
}

.subtask-close-btn {
  display: inline-flex;
  align-items: center;
  height: var(--button-height);
  padding: 0 var(--spacing-5);
  font-size: var(--text-base);
  font-weight: var(--font-weight-heading);
  border-radius: var(--radius-md);
  border: 1px solid var(--border-medium);
  background: transparent;
  color: var(--text-secondary);
  cursor: pointer;
  transition: all var(--transition-fast);
}

.subtask-close-btn:hover:not(:disabled) {
  background: var(--bg-hover);
  color: var(--text-primary);
}

.subtask-close-btn:disabled {
  opacity: 0.35;
  cursor: not-allowed;
}
</style>
