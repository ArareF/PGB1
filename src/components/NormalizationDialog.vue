<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { invoke } from '@tauri-apps/api/core'

interface NormalizePreviewItem {
  original_path: string
  original_name: string
  target_name: string
  action_type: 'rename' | 'move_to_folder'
  is_sequence: boolean
}

const props = defineProps<{
  taskPath: string
  show?: boolean
}>()

const emit = defineEmits<{
  close: []
  success: []
}>()

const loading = ref(true)
const executing = ref(false)
const items = ref<NormalizePreviewItem[]>([])
const error = ref<string | null>(null)

/** 选中的项目索引集合 */
const selectedIndexes = ref<Set<number>>(new Set())

async function fetchPreview() {
  loading.value = true
  error.value = null
  try {
    items.value = await invoke<NormalizePreviewItem[]>('preview_normalize', {
      taskPath: props.taskPath,
    })
    // 默认全选
    selectedIndexes.value = new Set(items.value.keys())
  } catch (e) {
    error.value = String(e)
    console.error('获取规范化预览失败:', e)
  } finally {
    loading.value = false
  }
}

function toggleSelectAll() {
  if (selectedIndexes.value.size === items.value.length) {
    selectedIndexes.value = new Set()
  } else {
    selectedIndexes.value = new Set(items.value.keys())
  }
}

function toggleItem(index: number) {
  const newSet = new Set(selectedIndexes.value)
  if (newSet.has(index)) {
    newSet.delete(index)
  } else {
    newSet.add(index)
  }
  selectedIndexes.value = newSet
}

async function handleExecute() {
  if (selectedIndexes.value.size === 0) return

  executing.value = true
  try {
    const selectedItems = items.value.filter((_, i) => selectedIndexes.value.has(i))
    await invoke('execute_normalize', { items: selectedItems })
    emit('success')
    emit('close')
  } catch (e) {
    error.value = String(e)
    console.error('执行规范化失败:', e)
  } finally {
    executing.value = false
  }
}

onMounted(fetchPreview)
</script>

<template>
  <Teleport to="body">
    <Transition name="dialog">
    <div v-if="props.show !== false" class="dialog-overlay" @click.self="!executing && emit('close')">
      <div class="dialog-content glass-strong">
        <div class="dialog-header">
          <p class="dialog-title">规范化预览</p>
          <p class="dialog-subtitle">识别静帧与序列帧，清理文件名后缀</p>
        </div>

        <div class="dialog-body">
          <div v-if="loading" class="loading-state">
            <span class="spinner"></span>
            扫描中...
          </div>
          
          <div v-else-if="error" class="error-state">
            <p>{{ error }}</p>
            <button class="retry-btn" @click="fetchPreview">重试</button>
          </div>

          <div v-else-if="items.length === 0" class="empty-state">
            <p>00_original 目录已规范化，无需操作。</p>
          </div>

          <div v-else class="preview-container">
            <div class="list-header">
              <label class="select-all" @click="toggleSelectAll">
                <span class="checkbox" :class="{ checked: selectedIndexes.size === items.length }" />
                全选 ({{ selectedIndexes.size }}/{{ items.length }})
              </label>
              <div class="header-labels">
                <span>原始文件</span>
                <span></span>
                <span>目标</span>
              </div>
            </div>

            <div class="preview-list">
              <div
                v-for="(item, index) in items"
                :key="item.original_path"
                class="preview-row"
                @click="toggleItem(index)"
              >
                <span class="checkbox" :class="{ checked: selectedIndexes.has(index) }" />
                
                <div class="row-content">
                  <div class="original-info">
                    <span class="file-name" :title="item.original_name">{{ item.original_name }}</span>
                  </div>

                  <div class="action-arrow">
                    <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                      <path d="M5 12h14M12 5l7 7-7 7" />
                    </svg>
                    <span class="action-label">{{ item.action_type === 'rename' ? '去后缀' : '归类' }}</span>
                  </div>

                  <div class="target-info">
                    <template v-if="item.action_type === 'rename'">
                      <span class="target-name" :title="item.target_name">{{ item.target_name }}</span>
                    </template>
                    <template v-else>
                      <div class="folder-badge">
                        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                          <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z" />
                        </svg>
                        <span class="folder-name">{{ item.target_name }}/</span>
                      </div>
                    </template>
                  </div>
                </div>
              </div>
            </div>
          </div>
        </div>

        <div class="dialog-actions">
          <button class="dialog-btn secondary" :disabled="executing" @click="emit('close')">
            取消
          </button>
          <button
            class="dialog-btn primary"
            :disabled="executing || items.length === 0 || selectedIndexes.size === 0"
            @click="handleExecute"
          >
            {{ executing ? '执行中...' : '执行规范化' }}
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
  z-index: var(--z-modal);
  display: flex;
  align-items: center;
  justify-content: center;
  background: var(--overlay-backdrop);
  backdrop-filter: blur(var(--glass-light-blur));
}

.dialog-content {
  width: 460px;
  max-height: 80vh;
  border-radius: var(--radius-2xl);
  display: flex;
  flex-direction: column;
  overflow: hidden;
  box-shadow: var(--shadow-modal);
}

.dialog-header {
  padding: var(--spacing-6) var(--spacing-6) var(--spacing-4);
  flex-shrink: 0;
}

.dialog-title {
  font-size: var(--text-2xl);
  font-weight: var(--font-weight-bold);
  color: var(--text-primary);
  margin-bottom: var(--spacing-1);
}

.dialog-subtitle {
  font-size: var(--text-sm);
  color: var(--text-tertiary);
}

.dialog-body {
  flex: 1;
  overflow: hidden;
  padding: 0 var(--spacing-6);
  min-height: 200px;
  display: flex;
  flex-direction: column;
}

.preview-container {
  flex: 1;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  border: 1px solid var(--border-medium);
  border-radius: var(--radius-lg);
  background: var(--bg-tertiary);
}

.list-header {
  display: flex;
  align-items: center;
  padding: var(--spacing-3);
  background: var(--bg-secondary);
  border-bottom: 1px solid var(--border-medium);
  font-size: var(--text-sm);
  font-weight: var(--font-bold);
  color: var(--text-secondary);
}

.select-all {
  display: flex;
  align-items: center;
  gap: var(--spacing-3);
  cursor: pointer;
  width: 120px;
  flex-shrink: 0;
}

.header-labels {
  flex: 1;
  display: grid;
  grid-template-columns: 1fr 80px 1fr;
  padding-left: var(--spacing-4);
}

.preview-list {
  flex: 1;
  overflow-y: auto;
}

.preview-row {
  display: flex;
  align-items: center;
  padding: var(--spacing-3);
  border-bottom: 1px solid var(--border-light);
  cursor: pointer;
  transition: background var(--duration-fast);
}

.preview-row:hover {
  background: var(--bg-hover);
}

.preview-row:last-child {
  border-bottom: none;
}

.row-content {
  flex: 1;
  display: grid;
  grid-template-columns: 1fr 80px 1fr;
  align-items: center;
  gap: var(--spacing-4);
  padding-left: var(--spacing-4);
  min-width: 0;
}

.original-info, .target-info {
  min-width: 0;
  display: flex;
  align-items: center;
}

.file-name, .target-name {
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  font-size: var(--text-sm);
}

.action-arrow {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 2px;
  color: var(--text-tertiary);
}

.action-label {
  font-size: var(--text-2xs);
  text-transform: uppercase;
  letter-spacing: 0.05em;
}

.folder-badge {
  display: inline-flex;
  align-items: center;
  gap: var(--spacing-1);
  background: var(--color-primary-50);
  color: var(--color-primary-700);
  padding: 2px 6px;
  border-radius: var(--radius-sm);
  max-width: 100%;
}

.folder-name {
  font-size: var(--text-xs);
  font-weight: var(--font-bold);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

/* Checkbox 样式 */
.checkbox {
  width: 18px;
  height: 18px;
  border: 2px solid var(--border-heavy);
  border-radius: var(--radius-sm);
  position: relative;
  transition: all var(--duration-fast);
  flex-shrink: 0;
}

.checkbox.checked {
  background: var(--color-primary-500);
  border-color: var(--color-primary-500);
}

.checkbox.checked::after {
  content: '';
  position: absolute;
  top: 2px;
  left: 5px;
  width: 4px;
  height: 8px;
  border: solid white;
  border-width: 0 2px 2px 0;
  transform: rotate(45deg);
}

.dialog-actions {
  padding: var(--spacing-6);
  display: flex;
  justify-content: flex-end;
  gap: var(--spacing-3);
  flex-shrink: 0;
}

.dialog-btn {
  height: var(--button-md-height);
  padding: 0 var(--spacing-6);
  border-radius: var(--radius-lg);
  font-weight: var(--font-bold);
  cursor: pointer;
  transition: all var(--duration-fast);
  border: none;
}

.dialog-btn.primary {
  background: var(--color-success);
  color: white;
}

.dialog-btn.primary:hover:not(:disabled) {
  background: var(--color-success-dark);
}

.dialog-btn.secondary {
  background: transparent;
  color: var(--text-secondary);
  border: 1px solid var(--border-medium);
}

.dialog-btn.secondary:hover:not(:disabled) {
  background: var(--bg-hover);
  color: var(--text-primary);
}

.dialog-btn:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.loading-state, .empty-state, .error-state {
  flex: 1;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: var(--spacing-4);
  color: var(--text-tertiary);
}

.spinner {
  width: 32px;
  height: 32px;
  border: 3px solid var(--border-light);
  border-top-color: var(--color-primary-500);
  border-radius: 50%;
  animation: spin 1s linear infinite;
}

@keyframes spin {
  to { transform: rotate(360deg); }
}

.retry-btn {
  background: var(--color-primary-500);
  color: white;
  border: none;
  padding: 4px 12px;
  border-radius: var(--radius-sm);
  cursor: pointer;
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
