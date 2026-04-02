<script setup lang="ts">
import { computed } from 'vue'
import { usePdfTranslate } from '../composables/usePdfTranslate'

const props = defineProps<{
  /** PDF 文件路径 */
  filePath: string
  /** 是否全屏模式 */
  isFullscreen?: boolean
}>()

const emit = defineEmits<{
  'toggle-fullscreen': []
}>()

// ─── PDF 翻译（全局状态 composable，侧边栏关闭不中断翻译） ───

const pdfFilePath = computed(() => props.filePath)
const {
  state: pdfTranslateState,
  progress: pdfTranslateProgress,
  error: pdfTranslateError,
  showingTranslated,
  retryInfo: pdfRetryInfo,
  activePdfSrc,
  start: handleTranslatePdf,
  toggleView: togglePdfView,
  reset: resetPdfTranslate,
} = usePdfTranslate(pdfFilePath)
</script>

<template>
  <!-- PDF 预览 -->
  <div class="preview-pdf-wrap">
    <iframe
      :key="activePdfSrc"
      :src="activePdfSrc"
      class="preview-pdf-frame"
      frameborder="0"
    />
    <button class="preview-fullscreen-btn" :title="isFullscreen ? $t('common.exitFullscreen') : $t('common.fullscreen')" @click="emit('toggle-fullscreen')">
      <svg v-if="!isFullscreen" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round">
        <polyline points="15 3 21 3 21 9" /><polyline points="9 21 3 21 3 15" />
        <line x1="21" y1="3" x2="14" y2="10" /><line x1="3" y1="21" x2="10" y2="14" />
      </svg>
      <svg v-else width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round">
        <polyline points="4 14 10 14 10 20" /><polyline points="20 10 14 10 14 4" />
        <line x1="10" y1="14" x2="3" y2="21" /><line x1="21" y1="3" x2="14" y2="10" />
      </svg>
    </button>
  </div>

  <!-- PDF 翻译区块 -->
  <div class="pdf-translate-section">
    <!-- idle：显示翻译按钮 -->
    <template v-if="pdfTranslateState === 'idle'">
      <button class="pdf-translate-btn" @click="handleTranslatePdf">
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <circle cx="12" cy="12" r="10"/>
          <path d="M12 2a15.3 15.3 0 0 1 4 10 15.3 15.3 0 0 1-4 10 15.3 15.3 0 0 1-4-10 15.3 15.3 0 0 1 4-10z"/>
          <line x1="2" y1="12" x2="22" y2="12"/>
        </svg>
        {{ $t('fileDetail.translatePdf') }}
      </button>
    </template>

    <!-- 进行中 -->
    <template v-else-if="pdfTranslateState === 'extracting' || pdfTranslateState === 'translating' || pdfTranslateState === 'building'">
      <div class="pdf-translate-progress">
        <span class="pdf-translate-spinner"/>
        <span v-if="pdfRetryInfo">
          {{ $t('fileDetail.translatePdfRetrying', { page: pdfRetryInfo.page + 1, attempt: pdfRetryInfo.attempt, max: pdfRetryInfo.maxRetries }) }}
        </span>
        <span v-else-if="pdfTranslateState === 'extracting'">{{ $t('fileDetail.loading') }}</span>
        <span v-else-if="pdfTranslateState === 'translating'">
          {{ $t('fileDetail.translatePdfProgress', pdfTranslateProgress) }}
        </span>
        <span v-else>{{ $t('fileDetail.translatePdfBuilding') }}</span>
      </div>
    </template>

    <!-- 完成 -->
    <template v-else-if="pdfTranslateState === 'done'">
      <div class="pdf-translate-done">
        <button class="pdf-translate-btn" @click="togglePdfView">
          {{ showingTranslated ? $t('fileDetail.translatePdfViewOriginal') : $t('fileDetail.translatePdfViewTranslated') }}
        </button>
        <button class="pdf-translate-reset-btn" :title="$t('fileDetail.translatePdfRetranslate')" @click="resetPdfTranslate">↺</button>
      </div>
    </template>

    <!-- 出错 -->
    <template v-else-if="pdfTranslateState === 'error'">
      <div class="pdf-translate-error">
        <span class="pdf-translate-error-msg">{{ pdfTranslateError }}</span>
        <button class="pdf-translate-reset-btn" :title="$t('fileDetail.translatePdfRetry')" @click="resetPdfTranslate">↺</button>
      </div>
    </template>
  </div>
</template>

<style>
/* 非 scoped — 与 FileDetailSidebar 全局样式一致 */

/* ─── PDF 预览 ─── */
.preview-pdf-wrap {
  width: 100%;
  flex: 1;
  min-height: 400px;
  border-radius: var(--radius-lg);
  overflow: hidden;
  flex-shrink: 0;
}

.preview-pdf-frame {
  width: 100%;
  height: 100%;
  min-height: 400px;
  border: none;
  display: block;
  border-radius: var(--radius-lg);
}

/* ─── PDF 翻译区块 ─── */
.pdf-translate-section {
  display: flex;
  flex-direction: column;
  padding: var(--spacing-2) var(--spacing-3);
  border-top: var(--glass-border);
  flex-shrink: 0;
}

.pdf-translate-btn {
  display: flex;
  align-items: center;
  gap: var(--spacing-2);
  width: 100%;
  padding: var(--spacing-2) var(--spacing-3);
  background: transparent;
  border: 1px solid var(--border-light);
  border-radius: var(--radius-md);
  color: var(--text-secondary);
  font-size: var(--font-size-sm);
  cursor: pointer;
  transition: background var(--duration-fast) var(--ease-out),
              color var(--duration-fast) var(--ease-out),
              border-color var(--duration-fast) var(--ease-out);
}

.pdf-translate-btn:hover {
  background: var(--bg-hover);
  color: var(--text-primary);
  border-color: var(--border-medium);
}

.pdf-translate-progress {
  display: flex;
  align-items: center;
  gap: var(--spacing-2);
  padding: var(--spacing-2) 0;
  font-size: var(--font-size-sm);
  color: var(--text-secondary);
}

.pdf-translate-spinner {
  display: inline-block;
  width: 12px;
  height: 12px;
  border: 2px solid var(--border-medium);
  border-top-color: var(--color-primary);
  border-radius: 50%;
  animation: pdf-spin 0.8s linear infinite;
  flex-shrink: 0;
}

@keyframes pdf-spin {
  to { transform: rotate(360deg); }
}

.pdf-translate-done {
  display: flex;
  align-items: center;
  gap: var(--spacing-2);
  padding: var(--spacing-2) 0;
  font-size: var(--font-size-sm);
}

.pdf-translate-done-label {
  color: var(--color-success);
  flex: 1;
}

.pdf-translate-open-btn {
  padding: var(--spacing-1) var(--spacing-3);
  background: var(--color-primary);
  color: #fff;
  border: none;
  border-radius: var(--radius-sm);
  font-size: var(--font-size-sm);
  cursor: pointer;
  transition: opacity var(--duration-fast);
  white-space: nowrap;
}

.pdf-translate-open-btn:hover { opacity: 0.85; }

.pdf-translate-reset-btn {
  padding: var(--spacing-1) var(--spacing-2);
  background: transparent;
  border: 1px solid var(--border-light);
  border-radius: var(--radius-sm);
  color: var(--text-tertiary);
  font-size: var(--font-size-sm);
  cursor: pointer;
  transition: color var(--duration-fast);
  flex-shrink: 0;
}

.pdf-translate-reset-btn:hover { color: var(--text-primary); }

.pdf-translate-error {
  display: flex;
  align-items: flex-start;
  gap: var(--spacing-2);
  padding: var(--spacing-2) 0;
  font-size: var(--font-size-sm);
}

.pdf-translate-error-msg {
  color: var(--color-danger);
  flex: 1;
  line-height: 1.4;
  word-break: break-word;
}
</style>
