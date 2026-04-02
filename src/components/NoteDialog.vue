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
.dialog-content {
  min-width: 380px;
  max-width: 520px;
  width: 100%;
}
</style>
