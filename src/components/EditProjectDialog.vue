<script setup lang="ts">
import { ref, computed } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import type { ProjectInfo } from '../composables/useProjects'

const props = defineProps<{
  project: ProjectInfo
  mode: 'rename' | 'deadline' | 'delete'
}>()

const emit = defineEmits<{
  updated: [project: ProjectInfo]
  deleted: [projectPath: string]
  cancel: []
}>()

const inputValue = ref(
  props.mode === 'rename' ? props.project.name :
  props.mode === 'deadline' ? (props.project.deadline ?? '') : ''
)
const errorMsg = ref('')
const loading = ref(false)

const title = computed(() => {
  if (props.mode === 'rename') return '重命名项目'
  if (props.mode === 'deadline') return '修改截止日期'
  return '删除项目'
})

const confirmLabel = computed(() => {
  if (props.mode === 'delete') return loading.value ? '删除中...' : '确认删除'
  return loading.value ? '保存中...' : '保存'
})

/** 将用户输入的日期标准化为 YYYY-MM-DD */
function normalizeDeadline(raw: string): string | null {
  const trimmed = raw.trim()
  if (!trimmed) return null
  if (/^\d{8}$/.test(trimmed)) {
    return `${trimmed.slice(0, 4)}-${trimmed.slice(4, 6)}-${trimmed.slice(6, 8)}`
  }
  if (/^\d{4}[-/]\d{1,2}[-/]\d{1,2}$/.test(trimmed)) {
    const parts = trimmed.split(/[-/]/)
    return `${parts[0]}-${parts[1].padStart(2, '0')}-${parts[2].padStart(2, '0')}`
  }
  return trimmed
}

async function handleConfirm() {
  errorMsg.value = ''
  loading.value = true
  try {
    if (props.mode === 'rename') {
      const updated = await invoke<ProjectInfo>('rename_project', {
        projectPath: props.project.path,
        newName: inputValue.value.trim(),
      })
      emit('updated', updated)
    } else if (props.mode === 'deadline') {
      await invoke('update_project_deadline', {
        projectPath: props.project.path,
        deadline: normalizeDeadline(inputValue.value),
      })
      emit('updated', { ...props.project, deadline: normalizeDeadline(inputValue.value) })
    } else {
      await invoke('delete_project', {
        projectPath: props.project.path,
      })
      emit('deleted', props.project.path)
    }
  } catch (e) {
    errorMsg.value = String(e)
  } finally {
    loading.value = false
  }
}
</script>

<template>
  <Teleport to="body">
    <Transition name="dialog" appear>
    <div class="dialog-overlay" @click.self="$emit('cancel')">
      <div class="dialog-content glass-strong">
        <p class="dialog-title">{{ title }}</p>

        <div class="dialog-body">
          <!-- 重命名 / 截止日期：输入框 -->
          <template v-if="mode !== 'delete'">
            <label class="field-label">
              {{ mode === 'rename' ? '新项目名称' : '截止日期' }}
            </label>
            <input
              v-model="inputValue"
              class="field-input"
              type="text"
              :placeholder="mode === 'rename' ? '如 218_NewGame' : 'YYYY-MM-DD'"
              autofocus
              @keydown.enter="handleConfirm"
              @keydown.esc="$emit('cancel')"
            />
          </template>

          <!-- 删除：警告文案 -->
          <template v-else>
            <p class="delete-warning">
              确定要删除项目 <strong>{{ project.name }}</strong> 吗？
            </p>
            <p class="delete-danger">项目目录将移入回收站，可从回收站恢复。</p>
          </template>

          <p v-if="errorMsg" class="error-text">{{ errorMsg }}</p>
        </div>

        <div class="dialog-actions">
          <button
            class="dialog-btn"
            :class="mode === 'delete' ? 'dialog-btn-danger' : 'dialog-btn-primary'"
            :disabled="loading"
            @click="handleConfirm"
          >
            {{ confirmLabel }}
          </button>
          <button class="dialog-btn dialog-btn-secondary" @click="$emit('cancel')">
            取消
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

.delete-warning {
  font-size: var(--text-base);
  color: var(--text-primary);
}

.delete-danger {
  font-size: var(--text-sm);
  color: var(--color-danger);
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

.dialog-btn:disabled {
  opacity: 0.5;
  cursor: not-allowed;
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

.dialog-btn-danger {
  background: color-mix(in srgb, var(--color-danger) 75%, transparent);
  backdrop-filter: blur(var(--glass-subtle-blur));
  -webkit-backdrop-filter: blur(var(--glass-subtle-blur));
  color: var(--color-neutral-0);
}

.dialog-btn-danger:hover:not(:disabled) {
  background: color-mix(in srgb, var(--color-danger) 90%, transparent);
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
