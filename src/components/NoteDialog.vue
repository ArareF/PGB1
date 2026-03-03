<script setup lang="ts">
import { ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { toggleCheckbox } from '../composables/useNotes'
import NoteEditor from './NoteEditor.vue'

const props = defineProps<{
  show: boolean
  title: string
  note: string
}>()

const emit = defineEmits<{
  save: [text: string]
  /** checkbox 切换：静默保存，不关闭弹窗 */
  update: [text: string]
  cancel: []
}>()

const { t } = useI18n()

const editText = ref('')
const editorRef = ref<InstanceType<typeof NoteEditor> | null>(null)

watch(() => props.show, (v) => {
  if (v) editText.value = props.note ?? ''
})

function handleSave() {
  emit('save', editText.value)
}

function handleToggleCheckbox(lineIndex: number) {
  editText.value = toggleCheckbox(editText.value, lineIndex)
  // checkbox 切换：静默保存数据，不关闭弹窗
  emit('update', editText.value)
}

/** 编辑器当前是否处于编辑模式 */
function isEditMode(): boolean {
  return editorRef.value?.mode === 'edit'
}
</script>

<template>
  <Teleport to="body">
    <Transition name="dialog">
      <div v-if="show" class="dialog-overlay" @mousedown.self.prevent>
        <div class="dialog-content glass-strong">
          <p class="dialog-title">{{ title }}</p>

          <div class="dialog-body">
            <NoteEditor
              ref="editorRef"
              v-model="editText"
              :save-on-blur="false"
              @save="handleSave"
              @toggle-checkbox="handleToggleCheckbox"
            />
          </div>

          <div class="dialog-actions">
            <template v-if="isEditMode()">
              <button
                class="dialog-btn dialog-btn-primary"
                @click="handleSave"
              >
                {{ t('common.save') }}
              </button>
              <button
                class="dialog-btn dialog-btn-secondary"
                @click="$emit('cancel')"
              >
                {{ t('common.cancel') }}
              </button>
            </template>
            <template v-else>
              <button
                class="dialog-btn dialog-btn-secondary"
                @click="$emit('cancel')"
              >
                {{ t('note.close') }}
              </button>
            </template>
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
  min-width: 380px;
  max-width: 520px;
  width: 100%;
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

.dialog-actions {
  display: flex;
  justify-content: flex-end;
  gap: var(--spacing-3);
}

.dialog-btn {
  display: inline-flex;
  align-items: center;
  height: var(--button-md-height);
  padding: 0 var(--spacing-5);
  font-size: var(--text-base);
  font-weight: var(--font-weight-heading);
  border-radius: var(--radius-md);
  border: none;
  cursor: pointer;
  font-family: inherit;
  transition: all var(--duration-fast);
}

.dialog-btn-primary {
  background: color-mix(in srgb, var(--color-primary-500) 75%, transparent);
  backdrop-filter: blur(var(--glass-subtle-blur));
  -webkit-backdrop-filter: blur(var(--glass-subtle-blur));
  color: var(--color-neutral-0);
}
.dialog-btn-primary:hover {
  background: color-mix(in srgb, var(--color-primary-500) 90%, transparent);
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
