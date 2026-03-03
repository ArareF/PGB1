<script setup lang="ts">
import { ref, computed, onMounted } from 'vue'
import { convertFileSrc } from '@tauri-apps/api/core'
import type { FileEntry } from '../composables/useDirectoryFiles'
import { getPsdThumbnail } from '../composables/usePsdThumbnail'
import NoteTooltip from './NoteTooltip.vue'

const props = defineProps<{
  file: FileEntry
  multiSelect?: boolean
  checked?: boolean
  hasNote?: boolean
  notePreview?: string
}>()

const cardRef = ref<HTMLElement | null>(null)

defineEmits<{
  click: [file: FileEntry]
}>()

const IMAGE_EXTS = new Set(['png', 'jpg', 'jpeg', 'gif', 'bmp', 'webp', 'svg', 'ico'])
const VIDEO_EXTS = new Set(['mp4', 'mov', 'avi', 'mkv', 'webm', 'flv'])
const PSD_EXTS   = new Set(['psd', 'psb'])
const PDF_EXTS   = new Set(['pdf'])

const isImage = computed(() => !props.file.is_dir && IMAGE_EXTS.has(props.file.extension))
const isVideo = computed(() => !props.file.is_dir && VIDEO_EXTS.has(props.file.extension))
const isPsd   = computed(() => !props.file.is_dir && PSD_EXTS.has(props.file.extension))
const isPdf   = computed(() => !props.file.is_dir && PDF_EXTS.has(props.file.extension))

const videoThumbnail = ref<string | null>(null)
const psdThumbnail   = ref<string | null>(null)

onMounted(async () => {
  if (isVideo.value) {
    const video = document.createElement('video')
    video.crossOrigin = 'anonymous'
    video.preload = 'metadata'
    video.src = convertFileSrc(props.file.path)
    video.currentTime = 0.1

    video.addEventListener('seeked', () => {
      const canvas = document.createElement('canvas')
      canvas.width = video.videoWidth || 200
      canvas.height = video.videoHeight || 150
      const ctx = canvas.getContext('2d')
      if (ctx) {
        ctx.drawImage(video, 0, 0, canvas.width, canvas.height)
        videoThumbnail.value = canvas.toDataURL('image/jpeg', 0.7)
      }
      video.src = ''
    }, { once: true })

    video.addEventListener('error', () => {
      video.src = ''
    }, { once: true })
  }

  if (isPsd.value) {
    psdThumbnail.value = await getPsdThumbnail(props.file.path, 256)
  }
})
</script>

<template>
  <button
    ref="cardRef"
    class="normal-card glass-subtle"
    :data-path="file.path"
    @click="$emit('click', file)"
  >
    <!-- 多选复选框 -->
    <span v-if="multiSelect" class="card-checkbox-shared" :class="{ checked }">
      <svg v-if="checked" width="14" height="14" viewBox="0 0 24 24" fill="currentColor">
        <path d="M9 16.17L4.83 12l-1.42 1.41L9 19 21 7l-1.41-1.41z" />
      </svg>
    </span>
    <!-- 预览区域 -->
    <div class="preview-wrapper">
      <div class="card-preview">
        <!-- 图片预览 -->
        <img
          v-if="isImage"
          :src="convertFileSrc(file.path)"
          :alt="file.name"
          class="preview-img"
          loading="lazy"
        />
        <!-- 视频：有截帧则显示截帧图，否则显示播放图标 -->
        <img
          v-else-if="isVideo && videoThumbnail"
          :src="videoThumbnail"
          :alt="file.name"
          class="preview-img"
        />
        <div v-else-if="isVideo" class="video-placeholder">
          <svg width="32" height="32" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
            <polygon points="5,3 19,12 5,21" fill="currentColor" stroke="none" opacity="0.6" />
          </svg>
        </div>
        <!-- PSD/PSB：有内嵌缩略图则显示，否则降级为 PS 图标 -->
        <img
          v-else-if="isPsd && psdThumbnail"
          :src="psdThumbnail"
          :alt="file.name"
          class="preview-img"
        />
        <div v-else-if="isPsd" class="psd-icon">
          <svg width="36" height="36" viewBox="0 0 36 36" fill="none">
            <rect width="36" height="36" rx="6" fill="#001E36"/>
            <text x="18" y="25" font-family="sans-serif" font-size="13" font-weight="700" fill="#31A8FF" text-anchor="middle">Ps</text>
          </svg>
        </div>
        <!-- PDF：红色图标 -->
        <div v-else-if="isPdf" class="pdf-icon">
          <svg width="36" height="36" viewBox="0 0 36 36" fill="none">
            <rect width="36" height="36" rx="6" fill="#CC0000"/>
            <text x="18" y="25" font-family="sans-serif" font-size="12" font-weight="700" fill="#FFFFFF" text-anchor="middle">PDF</text>
          </svg>
        </div>
        <!-- 文件夹图标 -->
        <svg v-else-if="file.is_dir" class="type-icon" width="40" height="40" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
          <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z" />
        </svg>
        <!-- 其他文件图标 -->
        <svg v-else class="type-icon" width="40" height="40" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
          <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z" />
          <polyline points="14 2 14 8 20 8" />
        </svg>
      </div>

      <!-- 格式标签（右下角，独立于预览容器） -->
      <span class="format-tag">
        {{ file.extension ? file.extension.toUpperCase() : 'DIR' }}
      </span>
    </div>

    <!-- 文件信息 -->
    <div class="card-info">
      <div class="card-name-row">
        <span class="card-name">{{ file.name }}</span>
        <svg v-if="hasNote" class="note-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M12 20h9"/><path d="M16.5 3.5a2.121 2.121 0 0 1 3 3L7 19l-4 1 1-4L16.5 3.5z"/></svg>
      </div>
    </div>

    <NoteTooltip
      v-if="hasNote"
      :target="cardRef"
      :text="notePreview ?? ''"
    />
  </button>
</template>

<style scoped>
.normal-card {
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

.normal-card:hover {
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

.format-tag {
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

.preview-img {
  width: 100%;
  height: 100%;
  object-fit: cover;
}

.type-icon {
  opacity: 0.5;
}

.card-info {
  display: flex;
  flex-direction: column;
  gap: var(--spacing-3);
  padding-top: var(--card-material-gap);
  min-width: 0;
}

.card-name-row {
  display: flex;
  align-items: center;
  gap: var(--spacing-1);
  min-width: 0;
}

.card-name {
  font-size: var(--text-lg);
  font-weight: var(--font-weight-heading);
  color: var(--text-primary);
  display: -webkit-box;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
  overflow: hidden;
  word-break: break-all;
}

.video-placeholder {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 100%;
  height: 100%;
  color: var(--text-tertiary);
}

.psd-icon {
  display: flex;
  align-items: center;
  justify-content: center;
  opacity: 0.85;
}

.pdf-icon {
  display: flex;
  align-items: center;
  justify-content: center;
  opacity: 0.85;
}

/* 多选选中态描边 */
.normal-card.multi-checked {
  outline: 2px solid var(--color-primary);
  outline-offset: -2px;
}
</style>
