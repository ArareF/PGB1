<script setup lang="ts">
import { ref, computed, watch, nextTick } from 'vue'
import { useI18n } from 'vue-i18n'
import { convertFileSrc, invoke } from '@tauri-apps/api/core'
import { formatSize } from '../utils/format'
import { IMAGE_EXTS, VIDEO_EXTS, TEXT_EXTS, PSD_EXTS, PDF_EXTS } from '../config/fileTypes'
import { getPsdThumbnail, invalidatePsdCache } from '../composables/usePsdThumbnail'
import type { FileEntry } from '../composables/useDirectoryFiles'
import { useDirectoryFiles } from '../composables/useDirectoryFiles'
import { toggleCheckbox } from '../composables/useNotes'
import SidebarShell from './SidebarShell.vue'
import NoteEditor from './NoteEditor.vue'
import ImageViewer from './ImageViewer.vue'
import VideoPlayer from './VideoPlayer.vue'
import PdfPreviewSection from './PdfPreviewSection.vue'

const props = withDefaults(defineProps<{
  file: FileEntry | null
  widthPercent?: number
  versions?: FileEntry[]
  /**
   * 自定义版本条目标题。不传时按「最新版本 / 版本 N」编号（预览视频用，versions 为旧→新）。
   * 素材系列传入日期标签，因为它的 versions 是新→旧，编号会反。
   */
  versionLabelOf?: (file: FileEntry, index: number) => string
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

const shellRef = ref<InstanceType<typeof SidebarShell> | null>(null)

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

const fileType = computed(() => {
  const ext = props.file?.extension.toLowerCase() ?? ''
  if (IMAGE_EXTS.has(ext)) return 'image'
  if (VIDEO_EXTS.has(ext)) return 'video'
  if (TEXT_EXTS.has(ext))  return 'text'
  if (PSD_EXTS.has(ext))   return 'psd'
  if (PDF_EXTS.has(ext))   return 'pdf'
  return 'other'
})

// ─── TXT 内容 ────────────────────────────────────────

const txtContent = ref<string | null>(null)
const txtLoading = ref(false)

const psdThumbnail   = ref<string | null>(null)
const psdThumbLoading = ref(false)

// 侧边栏进场动画时长，与 --duration-normal 保持一致
const SIDEBAR_ENTER_MS = 300

// ─── Race condition 防护（对齐 Y-8 / N-11）────────────────────
// 快速切换文件时，慢速 invoke 可能在新文件已选中后才返回，覆盖当前状态。
//
// 为什么不用 AbortController？
// Tauri 2 的 invoke() 不接受 AbortSignal——IPC 调用一旦发出，后端会跑完整个
// 命令处理器，前端无法取消。任何尝试用 AbortController 包装 invoke 都是假动作。
//
// 正确方案（Tauri 架构下的标准写法）：loadToken 每次切换递增，
// 异步回调在写回 UI 状态前检查 token 是否已过期，过期就丢弃结果。
// 成本：stale 请求的 IPC 工作不可取消（已调用），但 UI 状态不会被污染。
let loadToken = 0

watch(() => props.file, async (file, prevFile) => {
  const token = ++loadToken
  txtContent.value = null
  psdThumbnail.value = null

  if (!file) return

  if (fileType.value === 'text') {
    txtLoading.value = true
    try {
      const content = await invoke<string>('read_text_file', { path: file.path })
      if (token !== loadToken) return   // 过期结果，丢弃
      txtContent.value = content
    } catch (e) {
      if (token !== loadToken) return
      txtContent.value = t('fileDetail.readFailed')
      console.error('读取文本文件失败:', e)
    } finally {
      if (token === loadToken) txtLoading.value = false
    }
  }

  if (fileType.value === 'psd') {
    psdThumbLoading.value = true

    try {
      // 侧边栏刚打开时（prevFile 为 null）正在播放进场动画，等动画结束再加载
      if (!prevFile) {
        await new Promise(resolve => setTimeout(resolve, SIDEBAR_ENTER_MS))
        if (token !== loadToken) return
      }

      // 800px 不走 JS 缓存（侧边栏需要感知文件修改，freshness > perf）
      invalidatePsdCache(file.path, 800)
      const thumb = await getPsdThumbnail(file.path, 800)
      if (token !== loadToken) return
      psdThumbnail.value = thumb
    } catch (e) {
      if (token !== loadToken) return
      console.error('加载 PSD 缩略图失败:', e)
    } finally {
      // 只在 token 未过期时重置 loading（与 text 路径对齐，防 stale 结果残留 true）
      if (token === loadToken) psdThumbLoading.value = false
    }
  }
})


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
</script>

<template>
  <SidebarShell
    ref="shellRef"
    :show="!!file"
    :title="$t('fileDetail.detail')"
    :width-percent="widthPercent"
    :teleport-target="teleportTarget"
    :teleport-disabled="teleportDisabled"
    :has-actions="allowActions"
    @update:width-percent="emit('update:widthPercent', $event)"
  >
    <template #default="{ isFullscreen, toggleFullscreen }">

      <!-- 图片预览 -->
      <div v-if="fileType === 'image'" class="preview-image-wrap">
        <ImageViewer
          :key="file!.path"
          :src="convertFileSrc(file!.path)"
          :alt="file!.name"
        />
        <button v-if="!teleportDisabled" class="preview-fullscreen-btn" :title="isFullscreen ? $t('common.exitFullscreen') : $t('common.fullscreen')" @click="toggleFullscreen">
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

      <!-- 视频预览 -->
      <VideoPlayer
        v-else-if="fileType === 'video'"
        :src="file!.path"
        :is-fullscreen="isFullscreen"
        @toggle-fullscreen="toggleFullscreen"
      />

      <!-- TXT 文本预览 -->
      <div v-else-if="fileType === 'text'" class="preview-text-wrap">
        <p v-if="txtLoading" class="txt-loading">{{ $t('common.loading') }}</p>
        <pre v-else class="txt-content">{{ txtContent }}</pre>
      </div>

      <!-- PSD/PSB 预览 -->
      <div v-else-if="fileType === 'psd'" class="preview-psd-section">
        <div class="preview-psd-wrap">
          <p v-if="psdThumbLoading" class="txt-loading">{{ $t('fileDetail.loadingThumbnail') }}</p>
          <img
            v-else-if="psdThumbnail"
            :src="psdThumbnail"
            :alt="file!.name"
            class="psd-thumb-img"
          />
          <div v-else class="preview-other">
            <div class="file-icon">
              <svg width="48" height="48" viewBox="0 0 48 48" fill="none">
                <rect width="48" height="48" rx="8" fill="#001E36"/>
                <text x="24" y="33" font-family="sans-serif" font-size="17" font-weight="700" fill="#31A8FF" text-anchor="middle">Ps</text>
              </svg>
              <span class="file-ext">{{ file!.extension.toUpperCase() }}</span>
            </div>
          </div>
          <button v-if="!teleportDisabled" class="preview-fullscreen-btn" :title="isFullscreen ? $t('common.exitFullscreen') : $t('common.fullscreen')" @click="toggleFullscreen">
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
        <button class="open-file-btn" @click="openFile">{{ $t('fileDetail.openInPhotoshop') }}</button>
      </div>

      <!-- PDF 预览 + 翻译 -->
      <PdfPreviewSection
        v-else-if="fileType === 'pdf'"
        :file-path="file!.path"
        :is-fullscreen="isFullscreen"
        @toggle-fullscreen="toggleFullscreen"
      />

      <!-- 其他：文件类型图标 + 打开按钮 -->
      <div v-else class="preview-other">
        <div class="file-icon">
          <svg width="56" height="56" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.2">
            <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z" />
            <polyline points="14 2 14 8 20 8" />
          </svg>
          <span class="file-ext">{{ file!.extension.toUpperCase() || $t('fileDetail.file') }}</span>
        </div>
        <button class="open-file-btn" @click="openFile">{{ $t('fileDetail.openFile') }}</button>
      </div>

      <!-- 基本信息（文本类不显示） -->
      <div v-if="fileType !== 'text'" class="sidebar-section">
        <p class="section-title">{{ $t('fileDetail.basicInfo') }}</p>
        <div class="info-list">
          <div class="info-row">
            <span class="info-label">{{ $t('fileDetail.fileName') }}</span>
            <span class="info-value">{{ file!.name }}</span>
          </div>
          <div class="info-row">
            <span class="info-label">{{ $t('fileDetail.type') }}</span>
            <span class="info-value">{{ file!.extension.toUpperCase() || $t('fileDetail.unknown') }}</span>
          </div>
          <div class="info-row">
            <span class="info-label">{{ $t('fileDetail.size') }}</span>
            <span class="info-value">{{ formatSize(file!.size_bytes) }}</span>
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
            :class="{ active: v.path === file!.path }"
            :title="v.path"
            @click="emit('select-version', v)"
          >
            <div class="version-card-left">
              <span class="version-name">
                {{ versionLabelOf
                  ? versionLabelOf(v, i)
                  : (i === versions.length - 1 ? $t('fileDetail.latestVersion') : $t('fileDetail.versionN', { n: i + 1 })) }}
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

    </template>

    <template v-if="allowActions" #actions>
      <button class="sidebar-action-btn" @click="openRenameDialog">{{ $t('common.rename') }}</button>
      <button class="sidebar-action-btn danger" @click="openDeleteDialog">{{ $t('common.delete') }}</button>
    </template>

    <template #overlay>
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
    </template>
  </SidebarShell>
</template>

<style>
/* 非 scoped — 因为 Teleport 到 #content-row 层级 */

/* ─── 图片预览 ─── */
.preview-image-wrap {
  width: 100%;
  aspect-ratio: 4 / 3;
  max-height: var(--sidebar-preview-max-height);
  border-radius: var(--radius-lg);
  background: var(--glass-subtle-bg);
  overflow: hidden;
  flex-shrink: 0;
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
.preview-psd-section {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: var(--spacing-3);
  flex-shrink: 0;
}

.preview-psd-wrap {
  display: flex;
  flex-direction: column;
  align-items: center;
  width: 100%;
  max-height: var(--sidebar-preview-max-height);
  overflow: hidden;
  flex-shrink: 0;
}

.psd-thumb-img {
  width: 100%;
  max-height: 100%;
  border-radius: var(--radius-lg);
  object-fit: contain;
  background: var(--glass-subtle-bg);
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

/* .sidebar-actions / .sidebar-action-btn / .sidebar-dialog-* → design-system.css 公共类 */

/* ─── 页面内全屏模式（预览区铺满） ─── */

/* 图片/PDF/PSD 预览铺满 */
.sidebar-shell.is-fullscreen .preview-image-wrap,
.sidebar-shell.is-fullscreen .preview-pdf-wrap {
  flex: 1;
  min-height: 0;
  max-height: none;
  aspect-ratio: unset;
  border-radius: 0;
  overflow: hidden;
}

/* PSD 全屏：图片充满，隐藏"用 PS 打开"按钮 */
.sidebar-shell.is-fullscreen .preview-psd-wrap {
  flex: 1;
  min-height: 0;
  max-height: none;
  justify-content: center;
  border-radius: 0;
  overflow: hidden;
}

.sidebar-shell.is-fullscreen .preview-psd-section .open-file-btn {
  display: none;
}

.sidebar-shell.is-fullscreen .preview-psd-section {
  flex: 1;
  min-height: 0;
}

.sidebar-shell.is-fullscreen .psd-thumb-img {
  flex: 1;
  min-height: 0;
  width: 100%;
  object-fit: contain;
}

/* 视频全屏：wrap 铺满，video flex: 1 + contain 显示完整画面 */
.sidebar-shell.is-fullscreen .preview-video-wrap {
  flex: 1;
  min-height: 0;
  max-height: none;
  border-radius: 0;
  overflow: hidden;
}

.sidebar-shell.is-fullscreen .preview-video {
  flex: 1;
  min-height: 0;
  height: 0; /* 让 flex: 1 生效 */
  object-fit: contain;
}

/* PDF iframe 全屏 min-height 解除 */
.sidebar-shell.is-fullscreen .preview-pdf-frame {
  min-height: 0;
}

/* ─── 全屏按钮（图片/PDF/PSD 预览区悬浮右下角） ─── */

.preview-image-wrap,
.preview-pdf-wrap,
.preview-psd-wrap {
  position: relative;
}

.preview-fullscreen-btn {
  position: absolute;
  top: var(--spacing-2);
  right: var(--spacing-2);
  width: 28px;
  height: 28px;
  border-radius: var(--radius-md);
  border: 1px solid var(--overlay-btn-border);
  background: var(--overlay-btn-bg);
  color: var(--overlay-btn-text);
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  opacity: 0;
  transition: opacity var(--duration-fast), background var(--duration-fast);
  z-index: 5;
  backdrop-filter: blur(4px);
  -webkit-backdrop-filter: blur(4px);
}

.preview-image-wrap:hover .preview-fullscreen-btn,
.preview-video-wrap:hover .preview-fullscreen-btn,
.preview-pdf-wrap:hover .preview-fullscreen-btn,
.preview-psd-wrap:hover .preview-fullscreen-btn {
  opacity: 1;
}

/* 全屏模式下按钮常驻可见 */
.sidebar-shell.is-fullscreen .preview-fullscreen-btn {
  opacity: 1;
}

.preview-fullscreen-btn:hover {
  background: var(--overlay-btn-bg-hover);
}

</style>
