<script setup lang="ts">
import { ref, computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { invoke } from '@tauri-apps/api/core'
import { normalizeDeadline } from '../utils/format'
import type { ProjectInfo } from '../composables/useProjects'

const { t } = useI18n()

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
  if (props.mode === 'rename') return t('editProject.renameTitle')
  if (props.mode === 'deadline') return t('editProject.deadlineTitle')
  return t('editProject.deleteTitle')
})

const confirmLabel = computed(() => {
  if (props.mode === 'delete') return loading.value ? t('editProject.deleting') : t('editProject.confirmDelete')
  return loading.value ? t('editProject.saving') : t('editProject.save')
})

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
    console.error('[EditProjectDialog] 保存/删除项目失败:', e)
    errorMsg.value = String(e)
  } finally {
    loading.value = false
  }
}
</script>

<template>
  <Teleport to="body">
    <Transition name="dialog" appear>
    <div class="dialog-overlay">
      <div class="dialog-content glass-strong">
        <p class="dialog-title">{{ title }}</p>

        <div class="dialog-body">
          <!-- 重命名 / 截止日期：输入框 -->
          <template v-if="mode !== 'delete'">
            <label class="field-label">
              {{ mode === 'rename' ? $t('editProject.newName') : $t('editProject.deadlineLabel') }}
            </label>
            <input
              v-model="inputValue"
              class="field-input"
              type="text"
              :placeholder="mode === 'rename' ? $t('createProject.projectNamePlaceholder') : 'YYYY-MM-DD'"
              autofocus
              @keydown.enter="handleConfirm"
              @keydown.esc="$emit('cancel')"
            />
          </template>

          <!-- 删除：警告文案 -->
          <template v-else>
            <p class="delete-warning">
              {{ $t('editProject.deleteWarning', { name: project.name }) }}
            </p>
            <p class="delete-danger">{{ $t('editProject.deleteDanger') }}</p>
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
            {{ $t('common.cancel') }}
          </button>
        </div>
      </div>
    </div>
    </Transition>
  </Teleport>
</template>

<style scoped>
.dialog-content {
  min-width: 320px;
  max-width: 400px;
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
</style>
