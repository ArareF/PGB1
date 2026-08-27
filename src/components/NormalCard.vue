<script setup lang="ts">
import { ref, computed, watch, onMounted, onUnmounted } from 'vue'
import { convertFileSrc } from '@tauri-apps/api/core'
import type { FileEntry } from '../composables/useDirectoryFiles'
import { IMAGE_EXTS_BROWSE as IMAGE_EXTS, VIDEO_EXTS, PSD_EXTS, PDF_EXTS } from '../config/fileTypes'
import { getPsdThumbnail, invalidatePsdCache } from '../composables/usePsdThumbnail'
import { mediaVersion } from '../composables/useMediaCache'
import NoteTooltip from './NoteTooltip.vue'

const props = defineProps<{
  file: FileEntry
  multiSelect?: boolean
  checked?: boolean
  hasNote?: boolean
  notePreview?: string
  /** 覆盖卡片显示名（素材系列合并时显示基础名而非完整文件名） */
  displayName?: string
  /** 名称下方的副标题（如「最新 260807」） */
  subLabel?: string
  /** 版本数，>1 时右上角显示角标 */
  versionCount?: number
  /** 覆盖右下角格式标签（如多格式时的「PSD·JPG」） */
  formatLabel?: string
  /** 覆盖 data-path（多选 / 框选的身份标识），默认取 file.path */
  selectionPath?: string
}>()

const cardRef = ref<HTMLElement | null>(null)

defineEmits<{
  click: [file: FileEntry]
}>()

const isImage = computed(() => !props.file.is_dir && IMAGE_EXTS.has(props.file.extension))
const isVideo = computed(() => !props.file.is_dir && VIDEO_EXTS.has(props.file.extension))
const isPsd   = computed(() => !props.file.is_dir && PSD_EXTS.has(props.file.extension))
const isPdf   = computed(() => !props.file.is_dir && PDF_EXTS.has(props.file.extension))

const videoThumbnail = ref<string | null>(null)
const psdThumbnail   = ref<string | null>(null)

// 缓存命中：scan 时 Rust 已提供路径，直接转为 asset URL，无需任何 IPC
const cachedPsdUrl = computed(() =>
  props.file.thumbnail_path ? convertFileSrc(props.file.thumbnail_path) : null
)

// PSD 懒加载观察器，仅在缓存未命中时使用
let psdObserver: IntersectionObserver | null = null

// 视频缩略图懒加载：进入视口后才 seek 首帧
let videoObserver: IntersectionObserver | null = null

function generateVideoThumbnail() {
  const video = document.createElement('video')
  video.crossOrigin = 'anonymous'
  video.preload = 'metadata'
  // 带刷新代次：同名覆盖的新视频若走 webview 缓存，截出来还是旧首帧
  video.src = `${convertFileSrc(props.file.path)}?v=${mediaVersion.value}`
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

/** 建立视频截帧 / PSD 缩略图的懒加载观察器（挂载 + 手动刷新后各跑一次） */
function setupThumbnailObservers() {
  videoObserver?.disconnect()
  videoObserver = null
  psdObserver?.disconnect()
  psdObserver = null

  if (isVideo.value && cardRef.value) {
    videoObserver = new IntersectionObserver((entries) => {
      if (entries[0].isIntersecting) {
        videoObserver?.disconnect()
        videoObserver = null
        generateVideoThumbnail()
      }
    }, { rootMargin: '100px' })
    videoObserver.observe(cardRef.value)
  }

  // PSD 缓存未命中：进入视口后才发起 IPC 请求（首次生成缩略图）
  if (isPsd.value && !cachedPsdUrl.value && cardRef.value) {
    psdObserver = new IntersectionObserver((entries) => {
      if (entries[0].isIntersecting) {
        psdObserver?.disconnect()
        psdObserver = null
        // thumbnail_path 为 null 意味着 Rust 磁盘缓存不命中（首次或文件已修改）
        // 清除 JS 缓存，确保不会拿到同 session 内的旧缩略图
        invalidatePsdCache(props.file.path, 256)
        getPsdThumbnail(props.file.path, 256).then(url => {
          psdThumbnail.value = url
        })
      }
    }, { threshold: 0 })
    psdObserver.observe(cardRef.value)
  }
}

onMounted(setupThumbnailObservers)

/**
 * 手动刷新（mediaVersion +1）→ 丢弃本实例的截帧/缩略图重新生成。
 *
 * 观察器是一次性的（触发即 disconnect），卡片 key 又是稳定的文件/系列标识，
 * 不在这里重建的话：视频永远停在旧首帧；PSD 改动后 Rust 返回 thumbnail_path=null，
 * 模板会退回到还握着旧 URL 的 psdThumbnail，且再也不会重新请求。
 */
watch(mediaVersion, () => {
  videoThumbnail.value = null
  psdThumbnail.value = null
  setupThumbnailObservers()
})

onUnmounted(() => {
  videoObserver?.disconnect()
  videoObserver = null
  psdObserver?.disconnect()
  psdObserver = null
})
</script>

<template>
  <button
    ref="cardRef"
    class="normal-card"
    :data-path="selectionPath ?? file.path"
    @click="$emit('click', file)"
  >
    <!-- 多选复选框 -->
    <span v-if="multiSelect" class="card-checkbox-shared" :class="{ checked }">
      <svg v-if="checked" width="14" height="14" viewBox="0 0 24 24" fill="currentColor">
        <path d="M9 16.17L4.83 12l-1.42 1.41L9 19 21 7l-1.41-1.41z" />
      </svg>
    </span>
    <!-- 版本数角标（素材系列合并时显示） -->
    <span v-if="versionCount && versionCount > 1" class="version-badge">{{ versionCount }}</span>
    <!-- 预览区域 -->
    <div class="preview-wrapper">
      <div class="card-preview">
        <!-- 图片预览 -->
        <img
          v-if="isImage"
          :src="`${convertFileSrc(file.path)}?v=${mediaVersion}`"
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
        <!-- PSD/PSB 缓存命中：等同于 PNG/JPG，浏览器原生懒加载管理 -->
        <img
          v-else-if="isPsd && cachedPsdUrl"
          :src="cachedPsdUrl"
          :alt="file.name"
          class="preview-img"
          loading="lazy"
          decoding="async"
        />
        <!-- PSD/PSB 缓存未命中：IPC 懒加载结果（首次生成后下次 scan 即命中） -->
        <img
          v-else-if="isPsd && psdThumbnail"
          :src="psdThumbnail"
          :alt="file.name"
          class="preview-img"
          decoding="async"
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
        {{ formatLabel ?? (file.extension ? file.extension.toUpperCase() : 'DIR') }}
      </span>
    </div>

    <!-- 文件信息 -->
    <div class="card-info">
      <div class="card-name-row">
        <span class="card-name">{{ displayName ?? file.name }}</span>
        <svg v-if="hasNote" class="note-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M12 20h9"/><path d="M16.5 3.5a2.121 2.121 0 0 1 3 3L7 19l-4 1 1-4L16.5 3.5z"/></svg>
      </div>
      <span v-if="subLabel" class="card-sub-label">{{ subLabel }}</span>
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
  cursor: pointer;
  transition: var(--transition-card-hover);
  text-align: left;
  overflow: hidden;
  /* 手动 glass-subtle：不用 backdrop-filter，避免每张卡片创建独立合成层 */
  background: var(--glass-subtle-bg);
  border: var(--glass-subtle-border);
  box-shadow: var(--glass-subtle-shadow);
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
  gap: var(--spacing-1);
  padding-top: var(--card-material-gap);
  min-width: 0;
}

/* 版本数角标（右上角，与左上角多选框对称） */
.version-badge {
  position: absolute;
  top: var(--spacing-2);
  right: var(--spacing-2);
  min-width: var(--card-version-badge-size);
  height: var(--card-version-badge-size);
  padding: 0 var(--spacing-1);
  border-radius: var(--radius-full);
  background: var(--card-version-badge-bg);
  color: var(--card-version-badge-text);
  font-size: var(--text-xs);
  font-weight: var(--font-weight-heading);
  display: flex;
  align-items: center;
  justify-content: center;
  box-shadow: var(--shadow-sm);
  z-index: 2;
}

/* 副标题（最新版本日期） */
.card-sub-label {
  font-size: var(--text-xs);
  color: var(--text-tertiary);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
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
