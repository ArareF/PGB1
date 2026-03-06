<script setup lang="ts">
import { ref, computed, watch, nextTick } from 'vue'
import { useI18n } from 'vue-i18n'
import { convertFileSrc, invoke } from '@tauri-apps/api/core'
import { getPsdThumbnail } from '../composables/usePsdThumbnail'
import type { FileEntry } from '../composables/useDirectoryFiles'
import { useDirectoryFiles } from '../composables/useDirectoryFiles'
import { toggleCheckbox } from '../composables/useNotes'
import NoteEditor from './NoteEditor.vue'
import ImageViewer from './ImageViewer.vue'

// ─── 视频播放控制 ─────────────────────────────────────

const videoRef = ref<HTMLVideoElement | null>(null)
const isPlaying = ref(false)
const currentTime = ref(0)
const duration = ref(0)
const isSeeking = ref(false)

function onVideoTimeUpdate() {
  if (!isSeeking.value && videoRef.value) {
    currentTime.value = videoRef.value.currentTime
  }
}

function onVideoLoaded() {
  if (videoRef.value) {
    duration.value = videoRef.value.duration || 0
    currentTime.value = 0
    isPlaying.value = false
  }
}

function onVideoEnded() {
  isPlaying.value = false
}

function togglePlay() {
  const v = videoRef.value
  if (!v) return
  if (v.paused) {
    v.play()
    isPlaying.value = true
  } else {
    v.pause()
    isPlaying.value = false
  }
}

function seekTo(seconds: number) {
  const v = videoRef.value
  if (!v || !duration.value) return
  v.currentTime = Math.max(0, Math.min(duration.value, seconds))
  currentTime.value = v.currentTime
}

function onProgressMouseDown(e: MouseEvent) {
  isSeeking.value = true
  const bar = e.currentTarget as HTMLElement
  doSeekFromBar(e.clientX, bar)

  function onMove(ev: MouseEvent) { doSeekFromBar(ev.clientX, bar) }
  function onUp() {
    isSeeking.value = false
    window.removeEventListener('mousemove', onMove)
    window.removeEventListener('mouseup', onUp)
  }
  window.addEventListener('mousemove', onMove)
  window.addEventListener('mouseup', onUp)
}

function doSeekFromBar(clientX: number, bar: HTMLElement) {
  if (!duration.value) return
  const rect = bar.getBoundingClientRect()
  const ratio = Math.max(0, Math.min(1, (clientX - rect.left) / rect.width))
  seekTo(ratio * duration.value)
}

function onVideoKeydown(e: KeyboardEvent) {
  const v = videoRef.value
  if (!v) return
  if (e.key === ' ' || e.code === 'Space') {
    e.preventDefault()
    togglePlay()
  } else if (e.key === 'ArrowLeft') {
    e.preventDefault()
    seekTo(v.currentTime - (e.ctrlKey ? (1 / 30) : 1))
  } else if (e.key === 'ArrowRight') {
    e.preventDefault()
    seekTo(v.currentTime + (e.ctrlKey ? (1 / 30) : 1))
  }
}

function formatTime(s: number): string {
  if (!isFinite(s)) return '0:00'
  const m = Math.floor(s / 60)
  const sec = Math.floor(s % 60)
  return `${m}:${sec.toString().padStart(2, '0')}`
}

const progressPercent = computed(() =>
  duration.value > 0 ? (currentTime.value / duration.value) * 100 : 0
)

const props = withDefaults(defineProps<{
  file: FileEntry | null
  widthPercent?: number
  versions?: FileEntry[]
  /** 是否显示重命名/删除按钮（游戏介绍/项目素材页使用；预览视频侧边栏不显示） */
  allowActions?: boolean
  /** 笔记文本（有值时显示编辑区） */
  note?: string
  /** Teleport 目标选择器（默认 #content-row） */
  teleportTarget?: string
  /** 禁用 Teleport，就地渲染（弹窗内使用，避免卸载顺序导致崩溃） */
  teleportDisabled?: boolean
}>(), {
  teleportTarget: '#content-row',
})

const emit = defineEmits<{
  close: []
  'update:widthPercent': [value: number]
  'select-version': [file: FileEntry]
  /** 用户确认重命名，newName 为不含扩展名的新名称 */
  rename: [newName: string]
  /** 用户确认删除 */
  delete: []
  /** 保存笔记 */
  'save-note': [text: string]
}>()

const { openInExplorer } = useDirectoryFiles()
const { t } = useI18n()

// 笔记编辑
const noteText = ref('')
watch([() => props.file, () => props.note], () => {
  noteText.value = props.note ?? ''
})

/** 从文件完整路径提取所在目录 */
function getFolderPath(filePath: string): string {
  const sep = filePath.lastIndexOf('/')
  const bsep = filePath.lastIndexOf('\\')
  const idx = Math.max(sep, bsep)
  return idx > 0 ? filePath.substring(0, idx) : filePath
}

// ─── 文件类型判断 ────────────────────────────────────

const IMAGE_EXTS = ['png', 'jpg', 'jpeg', 'gif', 'webp', 'bmp', 'tiff', 'tif']
const VIDEO_EXTS = ['mp4', 'mov', 'avi', 'mkv', 'webm', 'flv']
const TEXT_EXTS  = ['txt']
const PSD_EXTS   = ['psd', 'psb']
const PDF_EXTS   = ['pdf']

const fileType = computed(() => {
  const ext = props.file?.extension.toLowerCase() ?? ''
  if (IMAGE_EXTS.includes(ext)) return 'image'
  if (VIDEO_EXTS.includes(ext)) return 'video'
  if (TEXT_EXTS.includes(ext))  return 'text'
  if (PSD_EXTS.includes(ext))   return 'psd'
  if (PDF_EXTS.includes(ext))   return 'pdf'
  return 'other'
})

// ─── TXT 内容 ────────────────────────────────────────

const txtContent = ref<string | null>(null)
const txtLoading = ref(false)

const psdThumbnail   = ref<string | null>(null)
const psdThumbLoading = ref(false)

watch(() => props.file, async (file) => {
  // 切换文件时重置视频状态
  isPlaying.value = false
  currentTime.value = 0
  duration.value = 0

  txtContent.value = null
  psdThumbnail.value = null

  if (!file) return

  if (fileType.value === 'text') {
    txtLoading.value = true
    try {
      txtContent.value = await invoke<string>('read_text_file', { path: file.path })
    } catch (e) {
      txtContent.value = t('fileDetail.readFailed')
      console.error('读取文本文件失败:', e)
    } finally {
      txtLoading.value = false
    }
  }

  if (fileType.value === 'psd') {
    psdThumbLoading.value = true
    psdThumbnail.value = await getPsdThumbnail(file.path, 800)
    psdThumbLoading.value = false
  }
})


// ─── 文件大小格式化 ──────────────────────────────────

function formatSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / 1024 / 1024).toFixed(1)} MB`
  return `${(bytes / 1024 / 1024 / 1024).toFixed(2)} GB`
}

// ─── 打开文件 ────────────────────────────────────────

async function openFile() {
  if (!props.file) return
  try {
    await invoke('open_file', { path: props.file.path })
  } catch (e) {
    console.error('打开文件失败:', e)
  }
}

// ─── 内联操作弹窗（重命名/删除） ─────────────────────

type SidebarDialog = 'none' | 'rename' | 'delete'
const sidebarDialog = ref<SidebarDialog>('none')
const renameInput = ref('')

/** 从文件名提取 stem（不含最后一个扩展名） */
function getFileStem(file: FileEntry): string {
  if (!file.extension) return file.name
  return file.name.slice(0, -(file.extension.length + 1))
}

function openRenameDialog() {
  renameInput.value = props.file ? getFileStem(props.file) : ''
  sidebarDialog.value = 'rename'
  nextTick(() => {
    (document.querySelector('.fds-dialog-input') as HTMLInputElement)?.select()
  })
}

function openDeleteDialog() {
  sidebarDialog.value = 'delete'
}

function closeSidebarDialog() {
  sidebarDialog.value = 'none'
  renameInput.value = ''
}

function confirmRename() {
  const trimmed = renameInput.value.trim()
  if (!trimmed || !props.file) { closeSidebarDialog(); return }
  if (trimmed === getFileStem(props.file)) { closeSidebarDialog(); return }
  emit('rename', trimmed)
  closeSidebarDialog()
}

function confirmDelete() {
  emit('delete')
  closeSidebarDialog()
}

// ─── 拖拽调整宽度 ────────────────────────────────────

const isResizing = ref(false)
const SIDEBAR_WIDTH_KEY = 'pgb1-sidebar-width'
const savedWidth = parseFloat(localStorage.getItem(SIDEBAR_WIDTH_KEY) || '')
const currentWidth = ref(isFinite(savedWidth) ? savedWidth : (props.widthPercent ?? 30))

watch(() => props.widthPercent, (v) => {
  if (v != null) currentWidth.value = v
})

function startResize(e: MouseEvent) {
  e.preventDefault()
  isResizing.value = true
  const startX     = e.clientX
  const startWidth = currentWidth.value

  function onMouseMove(ev: MouseEvent) {
    const windowWidth  = window.innerWidth
    const deltaPercent = ((startX - ev.clientX) / windowWidth) * 100
    currentWidth.value = Math.min(60, Math.max(20, startWidth + deltaPercent))
    emit('update:widthPercent', currentWidth.value)
  }
  function onMouseUp() {
    isResizing.value = false
    localStorage.setItem(SIDEBAR_WIDTH_KEY, String(currentWidth.value))
    window.removeEventListener('mousemove', onMouseMove)
    window.removeEventListener('mouseup', onMouseUp)
  }
  window.addEventListener('mousemove', onMouseMove)
  window.addEventListener('mouseup', onMouseUp)
}
</script>

<template>
  <Teleport :to="teleportTarget" :disabled="teleportDisabled">
    <Transition name="file-sidebar">
      <div
        v-if="file"
        class="file-detail-sidebar"
        :class="{ 'is-resizing': isResizing }"
        :style="{ width: currentWidth + '%' }"
      >
        <!-- 拖拽把手 -->
        <div class="resize-handle" @mousedown="startResize" />

        <!-- 标题 -->
        <div class="sidebar-header">
          <span class="sidebar-title">{{ $t('fileDetail.detail') }}</span>
        </div>

        <!-- 内容区 -->
        <div class="sidebar-body">

          <!-- 图片预览 -->
          <div v-if="fileType === 'image'" class="preview-image-wrap">
            <ImageViewer
              :key="file.path"
              :src="convertFileSrc(file.path)"
              :alt="file.name"
            />
          </div>

          <!-- 视频预览 -->
          <div
            v-else-if="fileType === 'video'"
            class="preview-video-wrap"
            tabindex="0"
            @keydown="onVideoKeydown"
          >
            <video
              ref="videoRef"
              :key="file.path"
              :src="convertFileSrc(file.path)"
              class="preview-video"
              preload="metadata"
              @timeupdate="onVideoTimeUpdate"
              @loadedmetadata="onVideoLoaded"
              @ended="onVideoEnded"
              @click="togglePlay"
            />
            <!-- 自定义控制条 -->
            <div class="video-controls">
              <button class="video-play-btn" @click.stop="togglePlay">
                <!-- 播放图标 -->
                <svg v-if="!isPlaying" width="16" height="16" viewBox="0 0 24 24" fill="currentColor">
                  <polygon points="5,3 19,12 5,21" />
                </svg>
                <!-- 暂停图标 -->
                <svg v-else width="16" height="16" viewBox="0 0 24 24" fill="currentColor">
                  <rect x="6" y="4" width="4" height="16" />
                  <rect x="14" y="4" width="4" height="16" />
                </svg>
              </button>
              <div
                class="video-progress-bar"
                @mousedown="onProgressMouseDown"
              >
                <div class="video-progress-fill" :style="{ width: progressPercent + '%' }" />
                <div class="video-progress-thumb" :style="{ left: progressPercent + '%' }" />
              </div>
              <span class="video-time">{{ formatTime(currentTime) }} / {{ formatTime(duration) }}</span>
            </div>
          </div>

          <!-- TXT 文本预览 -->
          <div v-else-if="fileType === 'text'" class="preview-text-wrap">
            <p v-if="txtLoading" class="txt-loading">{{ $t('common.loading') }}</p>
            <pre v-else class="txt-content">{{ txtContent }}</pre>
          </div>

          <!-- PSD/PSB 预览 -->
          <div v-else-if="fileType === 'psd'" class="preview-psd-wrap">
            <p v-if="psdThumbLoading" class="txt-loading">{{ $t('fileDetail.loadingThumbnail') }}</p>
            <img
              v-else-if="psdThumbnail"
              :src="psdThumbnail"
              :alt="file.name"
              class="psd-thumb-img"
            />
            <div v-else class="preview-other">
              <div class="file-icon">
                <svg width="48" height="48" viewBox="0 0 48 48" fill="none">
                  <rect width="48" height="48" rx="8" fill="#001E36"/>
                  <text x="24" y="33" font-family="sans-serif" font-size="17" font-weight="700" fill="#31A8FF" text-anchor="middle">Ps</text>
                </svg>
                <span class="file-ext">{{ file.extension.toUpperCase() }}</span>
              </div>
            </div>
            <button class="open-file-btn" @click="openFile">{{ $t('fileDetail.openInPhotoshop') }}</button>
          </div>

          <!-- PDF 预览 -->
          <div v-else-if="fileType === 'pdf'" class="preview-pdf-wrap">
            <iframe
              :key="file.path"
              :src="convertFileSrc(file.path)"
              class="preview-pdf-frame"
              frameborder="0"
            />
          </div>

          <!-- 其他：文件类型图标 + 打开按钮 -->
          <div v-else class="preview-other">
            <div class="file-icon">
              <svg width="56" height="56" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.2">
                <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z" />
                <polyline points="14 2 14 8 20 8" />
              </svg>
              <span class="file-ext">{{ file.extension.toUpperCase() || $t('fileDetail.file') }}</span>
            </div>
            <button class="open-file-btn" @click="openFile">{{ $t('fileDetail.openFile') }}</button>
          </div>

          <!-- 基本信息（文本类不显示） -->
          <div v-if="fileType !== 'text'" class="sidebar-section">
            <p class="section-title">{{ $t('fileDetail.basicInfo') }}</p>
            <div class="info-list">
              <div class="info-row">
                <span class="info-label">{{ $t('fileDetail.fileName') }}</span>
                <span class="info-value">{{ file.name }}</span>
              </div>
              <div class="info-row">
                <span class="info-label">{{ $t('fileDetail.type') }}</span>
                <span class="info-value">{{ file.extension.toUpperCase() || $t('fileDetail.unknown') }}</span>
              </div>
              <div class="info-row">
                <span class="info-label">{{ $t('fileDetail.size') }}</span>
                <span class="info-value">{{ formatSize(file.size_bytes) }}</span>
              </div>
            </div>
          </div>

          <!-- 版本列表（仅预览视频使用） -->
          <div v-if="versions && versions.length > 0" class="sidebar-section">
            <p class="section-title">{{ $t('fileDetail.versionHistory') }}</p>
            <div class="version-list">
              <div
                v-for="(v, i) in versions"
                :key="v.path"
                class="version-card"
                :class="{ active: v.path === file.path }"
                :title="v.path"
                @click="emit('select-version', v)"
              >
                <div class="version-card-left">
                  <span class="version-name">
                    {{ i === versions.length - 1 ? $t('fileDetail.latestVersion') : $t('fileDetail.versionN', { n: i + 1 }) }}
                  </span>
                  <span class="version-meta">{{ formatSize(v.size_bytes) }}</span>
                </div>
                <div class="version-card-right">
                  <span class="version-ext">{{ v.extension.toUpperCase() }}</span>
                  <button
                    class="version-folder-btn"
                    :title="$t('common.openContainingFolder')"
                    @click.stop="openInExplorer(getFolderPath(v.path))"
                  >
                    <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                      <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z" />
                    </svg>
                  </button>
                </div>
              </div>
            </div>
          </div>

          <!-- 笔记编辑区 -->
          <div v-if="note != null" class="sidebar-section">
            <p class="section-title">{{ $t('note.note') }}</p>
            <NoteEditor
              v-model="noteText"
              @save="emit('save-note', noteText)"
              @toggle-checkbox="(idx: number) => { noteText = toggleCheckbox(noteText, idx); emit('save-note', noteText) }"
            />
          </div>

        </div>

        <!-- 底部操作按钮（重命名/删除）；allowActions=false 时不渲染 -->
        <div v-if="allowActions" class="sidebar-actions">
          <button class="sidebar-action-btn" @click="openRenameDialog">{{ $t('common.rename') }}</button>
          <button class="sidebar-action-btn danger" @click="openDeleteDialog">{{ $t('common.delete') }}</button>
        </div>

        <!-- 内联操作弹窗 -->
        <div v-if="sidebarDialog !== 'none'" class="sidebar-dialog-overlay">
          <!-- 重命名弹窗 -->
          <div v-if="sidebarDialog === 'rename'" class="sidebar-dialog">
            <p class="sidebar-dialog-title">{{ $t('fileDetail.renameTitle') }}</p>
            <input
              v-model="renameInput"
              class="sidebar-dialog-input"
              :placeholder="$t('fileDetail.renamePlaceholder')"
              @keydown.enter="confirmRename"
              @keydown.escape="closeSidebarDialog"
            />
            <div class="sidebar-dialog-actions">
              <button class="sidebar-dialog-btn" @click="closeSidebarDialog">{{ $t('common.cancel') }}</button>
              <button class="sidebar-dialog-btn primary" @click="confirmRename">{{ $t('common.confirm') }}</button>
            </div>
          </div>
          <!-- 删除确认弹窗 -->
          <div v-if="sidebarDialog === 'delete'" class="sidebar-dialog">
            <p class="sidebar-dialog-title">{{ $t('fileDetail.deleteTitle') }}</p>
            <p class="sidebar-dialog-desc">{{ $t('fileDetail.deleteDesc', { name: file?.name }) }}</p>
            <div class="sidebar-dialog-actions">
              <button class="sidebar-dialog-btn" @click="closeSidebarDialog">{{ $t('common.cancel') }}</button>
              <button class="sidebar-dialog-btn danger" @click="confirmDelete">{{ $t('fileDetail.confirmDelete') }}</button>
            </div>
          </div>
        </div>
      </div>
    </Transition>
  </Teleport>
</template>

<style>
/* 非 scoped — 因为 Teleport 到 #content-row 层级 */

.file-detail-sidebar {
  position: relative;
  min-height: 0;
  display: flex;
  flex-direction: column;
  border-radius: var(--floating-main-radius);
  padding: var(--floating-main-padding);
  overflow: hidden;
  flex-shrink: 0;
  /* 手动复刻 glass-strong 视觉，不用 backdrop-filter：
     与 main-content(glass-medium) 相邻时，双 backdrop-filter
     在 WebView2 + Windows Acrylic 下产生白色闪烁伪影 */
  background: var(--glass-strong-bg);
  border: var(--glass-strong-border);
  box-shadow: var(--glass-strong-shadow);
}

.file-detail-sidebar.is-resizing {
  user-select: none;
}

.file-detail-sidebar .resize-handle {
  position: absolute;
  top: 0;
  left: 0;
  width: 4px;
  height: 100%;
  cursor: col-resize;
  z-index: 10;
}

.file-detail-sidebar .resize-handle:hover,
.file-detail-sidebar .resize-handle:active {
  background: var(--color-primary);
  opacity: 0.5;
}

/* 文件侧边栏进入/离开动画 */
.file-sidebar-enter-active {
  transition: transform var(--duration-normal) var(--ease-slide-in),
              width var(--duration-normal) var(--ease-slide-in);
  overflow: hidden;
}
.file-sidebar-leave-active {
  transition: transform var(--duration-fast) var(--ease-slide-out),
              width var(--duration-fast) var(--ease-slide-out);
  overflow: hidden;
}
.file-sidebar-enter-from,
.file-sidebar-leave-to {
  transform: translateX(100%);
  width: 0 !important;
}

/* 标题 */
.file-detail-sidebar .sidebar-header {
  display: flex;
  align-items: center;
  padding-bottom: var(--spacing-3);
  border-bottom: 1px solid var(--border-medium);
  flex-shrink: 0;
}

.file-detail-sidebar .sidebar-title {
  font-size: var(--text-2xl);
  font-weight: var(--font-weight-heading);
  color: var(--text-primary);
}

/* body 滚动区 */
.file-detail-sidebar .sidebar-body {
  flex: 1;
  overflow-y: auto;
  padding-top: var(--spacing-4);
  display: flex;
  flex-direction: column;
  gap: var(--spacing-4);
  min-height: 0;
}

/* ─── 图片预览 ─── */
.preview-image-wrap {
  width: 100%;
  aspect-ratio: 4 / 3;
  border-radius: var(--radius-lg);
  background: var(--glass-subtle-bg);
  overflow: hidden;
  flex-shrink: 0;
}

/* ─── 视频预览 ─── */
.preview-video-wrap {
  width: 100%;
  border-radius: var(--radius-lg);
  overflow: hidden;
  background: var(--color-neutral-900);
  flex-shrink: 0;
  display: flex;
  flex-direction: column;
  outline: none;
}

.preview-video-wrap:focus-within {
  box-shadow: 0 0 0 2px var(--color-primary-500);
}

.preview-video {
  width: 100%;
  display: block;
  object-fit: contain;
  cursor: pointer;
}

/* 自定义控制条 */
.video-controls {
  display: flex;
  align-items: center;
  gap: var(--spacing-2);
  padding: var(--spacing-2) var(--spacing-3);
  background: rgba(0, 0, 0, 0.6);
  backdrop-filter: blur(var(--glass-light-blur));
  -webkit-backdrop-filter: blur(var(--glass-light-blur));
}

.video-play-btn {
  flex-shrink: 0;
  width: 28px;
  height: 28px;
  border: none;
  background: rgba(255, 255, 255, 0.15);
  border-radius: 50%;
  color: #fff;
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  transition: background var(--duration-fast);
}

.video-play-btn:hover {
  background: rgba(255, 255, 255, 0.25);
}

.video-progress-bar {
  flex: 1;
  height: 4px;
  background: rgba(255, 255, 255, 0.2);
  border-radius: 2px;
  position: relative;
  cursor: pointer;
}

.video-progress-bar:hover {
  height: 6px;
}

.video-progress-fill {
  height: 100%;
  background: var(--color-primary-500);
  border-radius: 2px;
  pointer-events: none;
}

.video-progress-thumb {
  position: absolute;
  top: 50%;
  transform: translate(-50%, -50%);
  width: 10px;
  height: 10px;
  background: #fff;
  border-radius: 50%;
  pointer-events: none;
  opacity: 0;
  transition: opacity var(--duration-fast);
}

.video-progress-bar:hover .video-progress-thumb {
  opacity: 1;
}

.video-time {
  flex-shrink: 0;
  font-size: var(--text-xs);
  color: rgba(255, 255, 255, 0.7);
  font-variant-numeric: tabular-nums;
  white-space: nowrap;
}

/* ─── TXT 预览 ─── */
.preview-text-wrap {
  flex: 1;
  overflow-y: auto;
  min-height: 0;
}

.txt-loading {
  color: var(--text-tertiary);
  font-size: var(--text-sm);
}

.txt-content {
  font-family: var(--font-mono);
  font-size: var(--text-sm);
  color: var(--text-primary);
  white-space: pre-wrap;
  word-break: break-all;
  line-height: 1.7;
  margin: 0;
}

/* ─── PSD 预览 ─── */
.preview-psd-wrap {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: var(--spacing-3);
  flex-shrink: 0;
}

.psd-thumb-img {
  width: 100%;
  border-radius: var(--radius-lg);
  object-fit: contain;
  background: var(--glass-subtle-bg);
}

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

/* ─── 其他文件 ─── */
.preview-other {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: var(--spacing-4);
  padding: var(--spacing-6) 0;
  flex-shrink: 0;
}

.file-icon {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: var(--spacing-2);
  color: var(--text-tertiary);
}

.file-ext {
  font-size: var(--text-sm);
  font-weight: var(--font-weight-heading);
  color: var(--text-tertiary);
  letter-spacing: 0.05em;
}

.open-file-btn {
  padding: var(--spacing-2) var(--spacing-5);
  border-radius: var(--radius-button);
  border: var(--glass-medium-border);
  background: var(--glass-medium-bg);
  backdrop-filter: blur(var(--glass-subtle-blur));
  -webkit-backdrop-filter: blur(var(--glass-subtle-blur));
  color: var(--text-secondary);
  font-size: var(--text-base);
  font-family: inherit;
  cursor: pointer;
  transition: all var(--transition-fast);
}

.open-file-btn:hover {
  color: var(--text-primary);
}

/* ─── 基本信息区 ─── */
.file-detail-sidebar .sidebar-section {
  display: flex;
  flex-direction: column;
  gap: var(--spacing-2);
}

.file-detail-sidebar .section-title {
  font-size: var(--text-base);
  font-weight: var(--font-weight-heading);
  color: var(--text-secondary);
}

.file-detail-sidebar .info-list {
  display: flex;
  flex-direction: column;
  gap: var(--spacing-2);
}

.file-detail-sidebar .info-row {
  display: flex;
  justify-content: space-between;
  align-items: flex-start;
  gap: var(--spacing-3);
}

.file-detail-sidebar .info-label {
  font-size: var(--text-sm);
  color: var(--text-tertiary);
  flex-shrink: 0;
}

.file-detail-sidebar .info-value {
  font-size: var(--text-sm);
  color: var(--text-primary);
  text-align: right;
  word-break: break-all;
}

/* ─── 版本列表 ─── */
.file-detail-sidebar .version-list {
  display: flex;
  flex-direction: column;
  gap: var(--spacing-2);
}

.file-detail-sidebar .version-card {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: var(--spacing-3);
  border-radius: var(--radius-md);
  border: 1px solid var(--border-light);
  background: transparent;
  cursor: pointer;
  transition: all var(--transition-fast);
}

.file-detail-sidebar .version-card:hover {
  background: var(--bg-hover);
  border-color: var(--border-medium);
}

.file-detail-sidebar .version-card.active {
  background: var(--bg-selected);
  border-color: var(--color-primary);
}

.file-detail-sidebar .version-card-left {
  display: flex;
  flex-direction: column;
  gap: var(--spacing-1);
  min-width: 0;
}

.file-detail-sidebar .version-name {
  font-size: var(--text-base);
  font-weight: var(--font-weight-heading);
  color: var(--text-primary);
}

.file-detail-sidebar .version-card.active .version-name {
  color: var(--color-primary);
}

.file-detail-sidebar .version-meta {
  font-size: var(--text-sm);
  color: var(--text-tertiary);
}

.file-detail-sidebar .version-card-right {
  display: flex;
  align-items: center;
  gap: var(--spacing-2);
  flex-shrink: 0;
}

.file-detail-sidebar .version-ext {
  font-size: var(--text-sm);
  font-weight: var(--font-medium);
  color: var(--text-secondary);
}

.file-detail-sidebar .version-folder-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 28px;
  height: 28px;
  border-radius: 50%;
  border: 1px solid var(--border-medium);
  background: transparent;
  color: var(--text-tertiary);
  cursor: pointer;
  transition: all var(--transition-fast);
}

.file-detail-sidebar .version-folder-btn:hover {
  background: var(--color-primary);
  color: var(--color-neutral-0);
  border-color: var(--color-primary);
}

/* .sidebar-actions / .sidebar-action-btn / .sidebar-dialog-* → design-system.css 公共类 */
</style>
