<script setup lang="ts">
import { ref, computed, watch, nextTick } from 'vue'
import { useI18n } from 'vue-i18n'
import { NOTE_PREVIEW_LIMIT, stripMarkdown } from '../composables/useNotes'
import NoteRenderer from './NoteRenderer.vue'

const props = withDefaults(defineProps<{
  modelValue: string
  /** blur 时自动保存并切回渲染模式（sidebar 场景用；弹窗场景应传 false） */
  saveOnBlur?: boolean
}>(), {
  saveOnBlur: true,
})

const emit = defineEmits<{
  'update:modelValue': [value: string]
  'toggle-checkbox': [lineIndex: number]
  save: []
}>()

const { t } = useI18n()

const mode = ref<'render' | 'edit'>(props.modelValue ? 'render' : 'edit')
const textareaRef = ref<HTMLTextAreaElement | null>(null)

// 空内容自动进编辑模式
watch(() => props.modelValue, (val, oldVal) => {
  if (!val && oldVal) {
    // 内容被清空，保持编辑模式
  } else if (!val) {
    mode.value = 'edit'
  }
})

const progress = computed(() => {
  const len = stripMarkdown(props.modelValue).length
  return {
    current: Math.min(len, NOTE_PREVIEW_LIMIT),
    percent: Math.min((len / NOTE_PREVIEW_LIMIT) * 100, 100),
  }
})

function onInput(e: Event) {
  emit('update:modelValue', (e.target as HTMLTextAreaElement).value)
}

function switchToEdit() {
  mode.value = 'edit'
  nextTick(() => {
    textareaRef.value?.focus()
  })
}

function onBlur(e: FocusEvent) {
  // 如果焦点移到工具栏按钮，不触发保存
  const related = e.relatedTarget as HTMLElement | null
  if (related?.closest('.note-toolbar')) return
  if (!props.saveOnBlur) return
  emit('save')
  if (props.modelValue) {
    mode.value = 'render'
  }
}

function onCheckboxToggle(lineIndex: number) {
  emit('toggle-checkbox', lineIndex)
}

// ─── 工具栏插入逻辑 ──────────────────────────────────────

function insertAtCursor(before: string, after: string, placeholder: string) {
  const ta = textareaRef.value
  if (!ta) return
  const start = ta.selectionStart
  const end = ta.selectionEnd
  const text = props.modelValue
  const selected = text.slice(start, end)

  if (selected) {
    // 有选中 → 包裹
    const newText = text.slice(0, start) + before + selected + after + text.slice(end)
    emit('update:modelValue', newText)
    nextTick(() => {
      ta.focus()
      ta.setSelectionRange(start + before.length, end + before.length)
    })
  } else {
    // 无选中 → 插入占位文字并选中
    const newText = text.slice(0, start) + before + placeholder + after + text.slice(end)
    emit('update:modelValue', newText)
    nextTick(() => {
      ta.focus()
      ta.setSelectionRange(start + before.length, start + before.length + placeholder.length)
    })
  }
}

function insertBold() {
  insertAtCursor('**', '**', t('note.toolbar.bold'))
}

function insertItalic() {
  insertAtCursor('*', '*', t('note.toolbar.italic'))
}

function insertLink() {
  const ta = textareaRef.value
  if (!ta) return
  const start = ta.selectionStart
  const end = ta.selectionEnd
  const text = props.modelValue
  const selected = text.slice(start, end)
  const placeholder = t('note.toolbar.linkName')

  if (selected) {
    // 有选中文字 → 用选中文字作为链接名称
    const wrapped = '[' + selected + '](https://)'
    const newText = text.slice(0, start) + wrapped + text.slice(end)
    emit('update:modelValue', newText)
    // 选中 https:// 让用户粘贴 URL
    const urlStart = start + selected.length + 3 // [selected](
    nextTick(() => {
      ta.focus()
      ta.setSelectionRange(urlStart, urlStart + 8)
    })
  } else {
    // 无选中 → 插入 [名称](https://)，选中"名称"让用户修改
    const link = '[' + placeholder + '](https://)'
    const newText = text.slice(0, start) + link + text.slice(start)
    emit('update:modelValue', newText)
    nextTick(() => {
      ta.focus()
      ta.setSelectionRange(start + 1, start + 1 + placeholder.length)
    })
  }
}

function insertChecklist() {
  const ta = textareaRef.value
  if (!ta) return
  const text = props.modelValue
  const start = ta.selectionStart
  // 找到当前行首位置
  const lineStart = text.lastIndexOf('\n', start - 1) + 1
  const prefix = '- [ ] '
  const newText = text.slice(0, lineStart) + prefix + text.slice(lineStart)
  emit('update:modelValue', newText)
  nextTick(() => {
    ta.focus()
    // 光标放在行末
    ta.setSelectionRange(start + prefix.length, start + prefix.length)
  })
}

/** 暴露 mode 供父组件读取 */
defineExpose({ mode })
</script>

<template>
  <div class="note-editor">
    <!-- 渲染模式 -->
    <template v-if="mode === 'render' && modelValue">
      <div class="note-render-header">
        <button
          class="note-edit-btn"
          @click="switchToEdit"
        >
          {{ t('note.edit') }}
        </button>
      </div>
      <NoteRenderer
        :text="modelValue"
        @toggle-checkbox="onCheckboxToggle"
      />
    </template>

    <!-- 编辑模式 -->
    <template v-else>
      <div class="note-toolbar">
        <button class="note-toolbar-btn" :title="t('note.toolbar.bold')" @mousedown.prevent="insertBold">
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3" stroke-linecap="round" stroke-linejoin="round"><path d="M6 4h8a4 4 0 0 1 0 8H6z"/><path d="M6 12h9a4 4 0 0 1 0 8H6z"/></svg>
        </button>
        <button class="note-toolbar-btn" :title="t('note.toolbar.italic')" @mousedown.prevent="insertItalic">
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><line x1="19" y1="4" x2="10" y2="4"/><line x1="14" y1="20" x2="5" y2="20"/><line x1="15" y1="4" x2="9" y2="20"/></svg>
        </button>
        <button class="note-toolbar-btn" :title="t('note.toolbar.link')" @mousedown.prevent="insertLink">
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><path d="M10 13a5 5 0 0 0 7.54.54l3-3a5 5 0 0 0-7.07-7.07l-1.72 1.71"/><path d="M14 11a5 5 0 0 0-7.54-.54l-3 3a5 5 0 0 0 7.07 7.07l1.71-1.71"/></svg>
        </button>
        <button class="note-toolbar-btn" :title="t('note.toolbar.checklist')" @mousedown.prevent="insertChecklist">
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><polyline points="9 11 12 14 22 4"/><path d="M21 12v7a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h11"/></svg>
        </button>
      </div>
      <textarea
        ref="textareaRef"
        class="note-textarea"
        :value="modelValue"
        :placeholder="t('note.placeholder')"
        rows="6"
        @input="onInput"
        @blur="onBlur"
      />
      <div class="note-progress-row">
        <div class="note-progress-bar">
          <div class="note-progress-fill" :style="{ width: progress.percent + '%' }" />
        </div>
        <span class="note-progress-text">
          {{ t('note.hoverPreview') }} {{ progress.current }}/{{ NOTE_PREVIEW_LIMIT }}
        </span>
      </div>
    </template>
  </div>
</template>

<style scoped>
.note-editor {
  display: flex;
  flex-direction: column;
  gap: var(--spacing-1);
}

.note-render-header {
  display: flex;
  justify-content: flex-end;
}

.note-textarea {
  width: 100%;
  padding: var(--spacing-2) var(--spacing-3);
  border-radius: var(--radius-sm);
  border: 1px solid var(--border-medium);
  background: var(--input-bg);
  color: var(--text-primary);
  font-size: var(--text-sm);
  font-family: inherit;
  line-height: var(--leading-normal);
  resize: vertical;
  outline: none;
  box-sizing: border-box;
  transition: border-color var(--duration-fast) var(--ease-out);
}
.note-textarea:focus {
  border-color: var(--border-focus);
}
.note-textarea::placeholder {
  color: var(--text-tertiary);
}

.note-progress-row {
  display: flex;
  align-items: center;
  gap: var(--spacing-2);
}

.note-progress-bar {
  flex: 1;
  height: 3px;
  border-radius: var(--radius-full);
  background: var(--border-light);
  overflow: hidden;
}

.note-progress-fill {
  height: 100%;
  border-radius: var(--radius-full);
  background: var(--color-primary);
  transition: width var(--duration-fast) var(--ease-out);
}

.note-progress-text {
  font-size: var(--text-2xs);
  color: var(--text-tertiary);
  white-space: nowrap;
}
</style>
