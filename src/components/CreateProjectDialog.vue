<script setup lang="ts">
import { ref, computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { invoke } from '@tauri-apps/api/core'
import { useSettings } from '../composables/useSettings'

useI18n()

const props = defineProps<{ show?: boolean }>()

const emit = defineEmits<{
  created: [projectName: string]
  cancel: []
}>()

const { loadSettings } = useSettings()

const projectName = ref('')
const deadline = ref('')
const errorMsg = ref('')
const creating = ref(false)

const canCreate = computed(() => projectName.value.trim().length > 0 && !creating.value)

/** 将用户输入的日期标准化为 YYYY-MM-DD，支持 20260616 和 2026-06-16 */
function normalizeDeadline(raw: string): string | null {
  const trimmed = raw.trim()
  if (!trimmed) return null

  // 纯 8 位数字：20260616 → 2026-06-16
  if (/^\d{8}$/.test(trimmed)) {
    return `${trimmed.slice(0, 4)}-${trimmed.slice(4, 6)}-${trimmed.slice(6, 8)}`
  }
  // 已带分隔符：2026-06-16 或 2026/06/16
  if (/^\d{4}[-/]\d{1,2}[-/]\d{1,2}$/.test(trimmed)) {
    const parts = trimmed.split(/[-/]/)
    return `${parts[0]}-${parts[1].padStart(2, '0')}-${parts[2].padStart(2, '0')}`
  }
  return trimmed
}

async function handleCreate() {
  if (!canCreate.value) return

  errorMsg.value = ''
  creating.value = true

  try {
    const s = await loadSettings()
    await invoke('create_project', {
      rootDir: s?.general.projectRootDir ?? '',
      projectName: projectName.value.trim(),
      deadline: normalizeDeadline(deadline.value),
    })
    emit('created', projectName.value.trim())
  } catch (e) {
    errorMsg.value = String(e)
  } finally {
    creating.value = false
  }
}
</script>

<template>
  <Teleport to="body">
    <Transition name="dialog">
    <div v-if="props.show !== false" class="dialog-overlay">
      <div class="dialog-content glass-strong">
        <p class="dialog-title">{{ $t('createProject.title') }}</p>

        <div class="dialog-body">
          <label class="field-label">{{ $t('createProject.projectName') }}</label>
          <input
            v-model="projectName"
            class="field-input"
            type="text"
            :placeholder="$t('createProject.projectNamePlaceholder')"
            @keydown.enter="handleCreate"
          />

          <label class="field-label">{{ $t('createProject.deadline') }}</label>
          <input
            v-model="deadline"
            class="field-input"
            type="text"
            placeholder="YYYY-MM-DD"
            @keydown.enter="handleCreate"
          />

          <p v-if="errorMsg" class="error-text">{{ errorMsg }}</p>
        </div>

        <div class="dialog-actions">
          <button
            class="dialog-btn dialog-btn-primary"
            :disabled="!canCreate"
            @click="handleCreate"
          >
            {{ creating ? $t('createProject.creating') : $t('createProject.create') }}
          </button>
          <button class="dialog-btn dialog-btn-secondary" @click="$emit('cancel')">
            {{ $t('common.cancel') }}
          </button>
        </div>
      </div>
    </div>
    </Transition>
  </Teleport>
</template>

<style scoped>
.dialog-overlay {
  position: fixed;
  inset: 0;
  z-index: var(--z-modal, 1000);
  display: flex;
  align-items: center;
  justify-content: center;
  background: var(--overlay-backdrop);
  backdrop-filter: blur(var(--glass-light-blur));
}

.dialog-content {
  min-width: 320px;
  max-width: 400px;
  border-radius: var(--floating-navbar-radius);
  padding: var(--spacing-6);
  display: flex;
  flex-direction: column;
  gap: var(--spacing-5);
}

.dialog-title {
  font-size: var(--text-2xl);
  font-weight: var(--font-weight-heading);
  color: var(--text-primary);
}

.dialog-body {
  display: flex;
  flex-direction: column;
  gap: var(--spacing-3);
}

.field-label {
  font-size: var(--text-base);
  color: var(--text-secondary);
}

.field-input {
  height: var(--button-height);
  padding: 0 var(--spacing-3);
  font-size: var(--text-base);
  color: var(--text-primary);
  background: var(--bg-elevated);
  border: 1px solid var(--border-medium);
  border-radius: var(--radius-md);
  outline: none;
  transition: border-color var(--transition-fast);
}

.field-input:focus {
  border-color: var(--color-primary);
}

.field-input::placeholder {
  color: var(--text-tertiary);
}

.error-text {
  font-size: var(--text-sm);
  color: var(--color-danger);
}

.dialog-actions {
  display: flex;
  justify-content: flex-end;
  gap: var(--spacing-3);
}

.dialog-btn {
  display: inline-flex;
  align-items: center;
  height: var(--button-height);
  padding: 0 var(--spacing-5);
  font-size: var(--text-base);
  font-weight: var(--font-weight-heading);
  border-radius: var(--radius-md);
  border: none;
  cursor: pointer;
  transition: all var(--transition-fast);
}

.dialog-btn-primary {
  background: color-mix(in srgb, var(--color-primary-500) 75%, transparent);
  backdrop-filter: blur(var(--glass-subtle-blur));
  -webkit-backdrop-filter: blur(var(--glass-subtle-blur));
  color: var(--color-neutral-0);
}

.dialog-btn-primary:hover:not(:disabled) {
  background: color-mix(in srgb, var(--color-primary-500) 90%, transparent);
}

.dialog-btn-primary:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.dialog-btn-secondary {
  background: transparent;
  color: var(--text-secondary);
  border: 1px solid var(--border-medium);
}

.dialog-btn-secondary:hover {
  background: var(--bg-hover);
  color: var(--text-primary);
}
</style>

<style>
/* 弹窗进出动画 */
/* 根元素 .dialog-overlay 必须有 transition，Vue 以此计算等待时长 */
.dialog-enter-active {
  transition: opacity var(--duration-dialog) var(--ease-out);
}
.dialog-leave-active {
  transition: opacity var(--duration-dialog) var(--ease-in);
}
.dialog-enter-from,
.dialog-leave-to {
  opacity: 0;
}
/* 内容区额外的 transform 动画 */
.dialog-enter-active .dialog-content {
  transition: transform var(--duration-dialog) var(--ease-out);
}
.dialog-leave-active .dialog-content {
  transition: transform var(--duration-dialog) var(--ease-in);
}
.dialog-enter-from .dialog-content {
  transform: translateY(16px) scale(0.97);
}
.dialog-leave-to .dialog-content {
  transform: translateY(8px) scale(0.97);
}
</style>
