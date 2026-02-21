<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { readText, writeText } from '@tauri-apps/plugin-clipboard-manager'
import { useSettings } from '../composables/useSettings'

const { loadSettings } = useSettings()

const inputText = ref('')
const isTranslating = ref(false)
const copyFeedback = ref(false)
const langPair = ref('zh-en')
const lastOriginal = ref('')
const canUndo = ref(false)

onMounted(async () => {
  const settings = await loadSettings()
  if (settings) {
    const a = settings.translation.langA || 'zh-CN'
    const b = settings.translation.langB || 'en'
    // 映射到语言对
    if ((a === 'zh-CN' && b === 'en') || (a === 'en' && b === 'zh-CN')) langPair.value = 'zh-en'
    else if ((a === 'zh-CN' && b === 'ja') || (a === 'ja' && b === 'zh-CN')) langPair.value = 'zh-ja'
    else if ((a === 'en' && b === 'ja') || (a === 'ja' && b === 'en')) langPair.value = 'en-ja'
  }
})

function getLangPair() {
  const map: Record<string, [string, string]> = {
    'zh-en': ['zh-CN', 'en'],
    'zh-ja': ['zh-CN', 'ja'],
    'en-ja': ['en', 'ja'],
  }
  return map[langPair.value] || ['zh-CN', 'en']
}

async function handleTranslate() {
  if (isTranslating.value || !inputText.value.trim()) return

  const settings = await loadSettings()
  if (!settings) return

  isTranslating.value = true
  lastOriginal.value = inputText.value

  const [langA, langB] = getLangPair()

  try {
    const result = await invoke<string>('translate_text', {
      apiKey: settings.translation.apiKey,
      model: settings.translation.model,
      langA,
      langB,
      text: inputText.value,
    })
    inputText.value = result
    canUndo.value = true
  } catch (e) {
    console.error('翻译失败:', e)
  } finally {
    isTranslating.value = false
  }
}

function handleUndo() {
  if (!canUndo.value || !lastOriginal.value) return
  inputText.value = lastOriginal.value
  canUndo.value = false
}

async function handlePaste() {
  try {
    const text = await readText()
    if (text) {
      inputText.value = text
      canUndo.value = false
    }
  } catch (e) {
    console.error('读取剪贴板失败:', e)
  }
}

async function handleCopy() {
  if (!inputText.value) return
  try {
    await writeText(inputText.value)
    copyFeedback.value = true
    setTimeout(() => { copyFeedback.value = false }, 1500)
  } catch (e) {
    console.error('复制失败:', e)
  }
}

function handleKeydown(e: KeyboardEvent) {
  if (e.ctrlKey && e.key === 'Enter') {
    e.preventDefault()
    handleTranslate()
  }
}
</script>

<template>
  <div class="translator-window">
    <!-- 拖拽条 -->
    <div class="drag-handle" data-tauri-drag-region>
      <div class="drag-pill" data-tauri-drag-region />
    </div>

    <!-- 输入框容器 -->
    <div class="text-container">
      <textarea
        v-model="inputText"
        class="panel-textarea"
        placeholder="在此输入要翻译的文本..."
        @keydown="handleKeydown"
      />
      <!-- 粘贴/复制按钮 -->
      <div class="text-actions">
        <button class="action-btn" @click="handlePaste" title="粘贴">
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <path d="M16 4h2a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V6a2 2 0 0 1 2-2h2"/>
            <rect x="8" y="2" width="8" height="4" rx="1" ry="1"/>
          </svg>
          <span>粘贴</span>
        </button>
        <button class="action-btn" :class="{ 'action-btn-success': copyFeedback }" @click="handleCopy" title="复制">
          <svg v-if="!copyFeedback" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <rect x="9" y="9" width="13" height="13" rx="2" ry="2"/>
            <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"/>
          </svg>
          <svg v-else width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
            <polyline points="20 6 9 17 4 12"/>
          </svg>
          <span>{{ copyFeedback ? '已复制' : '复制' }}</span>
        </button>
      </div>
    </div>

    <!-- 操作行：语言对 + 翻译按钮 + 撤回 -->
    <div class="translator-action-row">
      <select v-model="langPair" class="lang-select">
        <option value="zh-en">中英</option>
        <option value="zh-ja">中日</option>
        <option value="en-ja">英日</option>
      </select>
      <button
        class="translate-btn"
        :disabled="isTranslating || !inputText.trim()"
        @click="handleTranslate"
      >
        <svg v-if="!isTranslating" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
          <polygon points="5 3 19 12 5 21 5 3"/>
        </svg>
        <svg v-else class="spin-icon" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round">
          <circle cx="12" cy="12" r="10" stroke-opacity="0.25"/>
          <path d="M12 2a10 10 0 0 1 10 10" stroke-opacity="1"/>
        </svg>
        {{ isTranslating ? '翻译中...' : '翻译' }}
      </button>
      <button
        class="undo-btn"
        :disabled="!canUndo"
        @click="handleUndo"
        title="撤回原文"
      >
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <polyline points="1 4 1 10 7 10"/>
          <path d="M3.51 15a9 9 0 1 0 2.13-9.36L1 10"/>
        </svg>
        <span>撤回</span>
      </button>
    </div>
  </div>
</template>

<style scoped>
/* ── 窗口容器 ── */
.translator-window {
  width: 100%;
  height: 100vh;
  display: flex;
  flex-direction: column;
  background: transparent;
  overflow: hidden;
  color: var(--text-primary);
  font-family: var(--font-family-base);
  user-select: none;
  padding: 10px;
  gap: var(--spacing-3);
}

/* ── 拖拽条 ── */
.drag-handle {
  display: flex;
  justify-content: center;
  padding: 2px 0;
  flex-shrink: 0;
  cursor: grab;
}
.drag-pill {
  width: 48px;
  height: 5px;
  border-radius: var(--radius-full);
  background: var(--border-medium);
}

/* ── 输入框容器 ── */
.text-container {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
  background: var(--glass-medium-bg);
  border: var(--glass-medium-border);
  border-radius: var(--radius-lg);
  overflow: hidden;
}
.panel-textarea {
  flex: 1;
  resize: none;
  border: none;
  background: transparent;
  color: var(--text-primary);
  font-family: var(--font-family-base);
  font-size: var(--text-sm);
  line-height: var(--leading-relaxed);
  padding: var(--spacing-3);
  box-sizing: border-box;
  outline: none;
  user-select: text;
}
.panel-textarea::placeholder {
  color: var(--text-tertiary);
}

/* ── 粘贴/复制按钮行 ── */
.text-actions {
  display: flex;
  justify-content: flex-end;
  gap: var(--spacing-2);
  padding: var(--spacing-2) var(--spacing-3);
}
.action-btn {
  display: flex;
  align-items: center;
  gap: var(--spacing-1);
  padding: var(--spacing-1) var(--spacing-2);
  border: none;
  border-radius: var(--radius-md);
  background: transparent;
  color: var(--text-secondary);
  font-family: var(--font-family-base);
  font-size: var(--text-xs);
  cursor: pointer;
  transition: var(--transition-all);
}
.action-btn:hover {
  background: var(--bg-hover);
  color: var(--text-primary);
}
.action-btn-success {
  color: var(--color-success);
}

/* ── 操作行 ── */
.translator-action-row {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: var(--spacing-4);
  flex-shrink: 0;
  padding-bottom: var(--spacing-1);
}
.translate-btn {
  display: flex;
  align-items: center;
  gap: var(--spacing-2);
  padding: 0 var(--spacing-6);
  height: var(--button-md-height);
  border: var(--glass-medium-border);
  border-radius: var(--radius-button);
  background: var(--glass-medium-bg);
  color: var(--text-primary);
  font-family: var(--font-family-base);
  font-size: var(--text-sm);
  font-weight: var(--font-semibold);
  cursor: pointer;
  transition: var(--transition-all);
}
.translate-btn:hover:not(:disabled) {
  background: var(--bg-hover);
}
.translate-btn:disabled {
  opacity: var(--button-disabled-opacity);
  cursor: not-allowed;
}

@keyframes spin {
  to { transform: rotate(360deg); }
}
.spin-icon {
  animation: spin 0.8s linear infinite;
}

/* ── 语言对选择器 ── */
.lang-select {
  height: var(--button-md-height);
  border: var(--glass-medium-border);
  border-radius: var(--radius-md);
  background: var(--glass-medium-bg);
  color: var(--text-primary);
  font-family: var(--font-family-base);
  font-size: var(--text-xs);
  padding: 0 var(--spacing-2);
  outline: none;
  cursor: pointer;
  transition: var(--transition-all);
}
.lang-select:focus {
  border-color: var(--border-focus);
}

/* ── 撤回按钮 ── */
.undo-btn {
  display: flex;
  align-items: center;
  gap: var(--spacing-1);
  height: var(--button-md-height);
  padding: 0 var(--spacing-3);
  border: var(--glass-medium-border);
  border-radius: var(--radius-md);
  background: var(--glass-medium-bg);
  color: var(--text-secondary);
  font-family: var(--font-family-base);
  font-size: var(--text-xs);
  cursor: pointer;
  transition: var(--transition-all);
}
.undo-btn:hover:not(:disabled) {
  background: var(--bg-hover);
  color: var(--text-primary);
}
.undo-btn:disabled {
  opacity: var(--button-disabled-opacity);
  cursor: not-allowed;
}
</style>
