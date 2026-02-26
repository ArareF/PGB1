<script setup lang="ts">
import { convertFileSrc } from '@tauri-apps/api/core'
import { useI18n } from 'vue-i18n'
import { useSettings } from '../composables/useSettings'
import type { MaterialInfo } from '../composables/useMaterials'
import SequencePreview from './SequencePreview.vue'

const { t } = useI18n()

const { settings } = useSettings()

defineProps<{
  material: MaterialInfo
  multiSelect?: boolean
  checked?: boolean
  scaleLabel?: string   // 缩放标注，有值时替换右下角 size-tag
}>()

defineEmits<{
  click: [material: MaterialInfo]
}>()

/** 文件大小格式化 */
function formatSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(1)} GB`
}

/** 进度状态映射 */
function progressLabel(progress: string): string {
  const map: Record<string, string> = {
    none: t('materialCard.notStarted'),
    original: t('materialCard.original'),
    scaled: t('materialCard.scaled'),
    done: t('materialCard.done'),
    uploaded: t('materialCard.uploaded'),
    broken: t('materialCard.broken'),
  }
  return map[progress] ?? progress
}
</script>

<template>
  <button
    class="material-card glass-subtle"
    :data-path="material.path"
    @click="$emit('click', material)"
  >
    <!-- 多选复选框 -->
    <span v-if="multiSelect" class="card-checkbox" :class="{ checked }">
      <svg v-if="checked" width="14" height="14" viewBox="0 0 24 24" fill="currentColor">
        <path d="M9 16.17L4.83 12l-1.42 1.41L9 19 21 7l-1.41-1.41z" />
      </svg>
    </span>

    <!-- 预览区域 -->
    <div class="preview-wrapper">
      <div class="card-preview">
        <!-- 序列帧：Canvas 动画预览 -->
        <SequencePreview
          v-if="material.material_type === 'sequence'"
          :key="`${material.path}-${material.name}-${material.fps ?? settings?.preview.defaultFps ?? 24}`"
          :folder-path="material.path"
          :base-name="material.name"
          :fps="material.fps ?? settings?.preview.defaultFps ?? 24"
          :max-width="200"
          :transparent="settings?.preview.backgroundTransparent ?? false"
        />
        <!-- 静帧/其他：图片预览 -->
        <img
          v-else-if="material.preview_path"
          :src="convertFileSrc(material.preview_path)"
          :alt="material.name"
          class="preview-img"
          loading="lazy"
        />
        <!-- 无预览时的占位图标 -->
        <svg v-else class="type-icon" width="40" height="40" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
          <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z" />
          <polyline points="14 2 14 8 20 8" />
        </svg>
      </div>

      <!-- 缩放标注角标（ScalePage 使用，优先于 fps badge） -->
      <span v-if="scaleLabel" class="frame-badge scale-badge">{{ scaleLabel }}</span>
      <!-- 序列帧帧率角标（转换后才显示） -->
      <span v-else-if="material.material_type === 'sequence' && material.fps" class="frame-badge">
        {{ material.fps }}fps
      </span>
    </div>

    <!-- 文件信息 -->
    <div class="card-info">
      <span class="card-name" :title="material.file_name">{{ material.name.includes('/') ? material.name.split('/')[1] : material.name }}</span>
      <div class="card-tags">
        <span class="progress-tag" :class="`progress-${material.progress}`">
          {{ progressLabel(material.progress) }}
        </span>
        <span class="size-tag">{{ formatSize(material.size_bytes) }}</span>
      </div>
    </div>
  </button>
</template>

<style scoped>
.material-card {
  position: relative;
  width: var(--card-material-width);
  display: flex;
  flex-direction: column;
  padding: var(--card-material-padding);
  border-radius: var(--card-border-radius);
  border: none;
  cursor: pointer;
  transition: var(--transition-card-hover);
  text-align: left;
  overflow: hidden;
}

.material-card:hover {
  transform: translateY(var(--card-hover-lift));
  box-shadow: var(--card-shadow-hover);
}

.preview-wrapper {
  position: relative;
}

.card-preview {
  width: 100%;
  aspect-ratio: 1 / 1;
  display: flex;
  align-items: center;
  justify-content: center;
  background: var(--bg-hover);
  border-radius: var(--radius-md);
  overflow: hidden;
  color: var(--text-tertiary);
}

.preview-img {
  width: 100%;
  height: 100%;
  object-fit: cover;
}

.type-icon {
  opacity: 0.5;
}

.frame-badge {
  position: absolute;
  bottom: 0;
  right: 0;
  display: inline-flex;
  align-items: center;
  height: 32px;
  padding: 0 var(--spacing-3);
  font-size: var(--text-base);
  font-weight: var(--tag-font-weight);
  border-radius: var(--radius-md) 0 0 var(--radius-md);
  background: var(--tag-format-bg);
  color: var(--tag-format-text);
  border: 1px solid var(--tag-format-border);
  border-right: none;
  border-bottom: none;
  backdrop-filter: blur(var(--glass-subtle-blur));
}

.scale-badge {
  background: var(--color-primary-500);
  color: white;
  border-color: var(--color-primary-600);
}

.card-info {
  display: flex;
  flex-direction: column;
  justify-content: space-between;
  padding-top: var(--card-material-gap);
  min-width: 0;
  min-height: 56px;
}

.card-name {
  font-size: var(--text-lg);
  font-weight: var(--font-weight-heading);
  color: var(--text-primary);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.card-tags {
  display: flex;
  align-items: flex-end;
  gap: var(--spacing-2);
}

.progress-tag {
  display: inline-flex;
  align-items: center;
  height: var(--tag-height);
  padding: 0 var(--tag-padding-x);
  font-size: var(--text-sm);
  font-weight: var(--tag-font-weight);
  border-radius: var(--tag-border-radius);
  color: var(--tag-status-text);
}

.progress-none     { background: var(--tag-progress-none-bg); }
.progress-original { background: var(--tag-progress-original-bg); color: var(--color-neutral-700); }
.progress-scaled   { background: var(--tag-progress-scaled-bg); }
.progress-done     { background: var(--tag-progress-done-bg); }
.progress-uploaded { background: var(--tag-progress-uploaded-bg); }
.progress-broken {
  background: var(--color-danger-500, #ef4444);
  color: white;
  animation: broken-pulse 2s ease-in-out infinite;
}

@keyframes broken-pulse {
  0%, 100% { opacity: 1; }
  50% { opacity: 0.7; }
}

.size-tag {
  font-size: var(--text-sm);
  color: var(--text-tertiary);
}

/* 多选复选框 */
.card-checkbox {
  position: absolute;
  top: var(--spacing-2);
  left: var(--spacing-2);
  width: 24px;
  height: 24px;
  border-radius: var(--radius-sm);
  border: 2px solid var(--border-heavy);
  background: var(--glass-subtle-bg);
  backdrop-filter: blur(var(--glass-subtle-blur));
  box-shadow: var(--shadow-sm);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 2;
  transition: all var(--transition-fast);
}

.card-checkbox.checked {
  background: var(--color-primary);
  border-color: var(--color-primary);
  color: var(--color-neutral-0);
}

/* 多选选中态 */
.material-card.multi-checked {
  outline: 2px solid var(--color-primary);
  outline-offset: -2px;
}


</style>
