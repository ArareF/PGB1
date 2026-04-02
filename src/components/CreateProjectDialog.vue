<script setup lang="ts">
import { ref, computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { invoke } from '@tauri-apps/api/core'
import { normalizeDeadline } from '../utils/format'
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

.error-text {
  font-size: var(--text-sm);
  color: var(--color-danger);
}
</style>
