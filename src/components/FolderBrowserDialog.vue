<script setup lang="ts">
import { ref, computed, watch } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { useI18n } from 'vue-i18n'
import type { FileEntry } from '../composables/useDirectoryFiles'
import { useDirectoryFiles } from '../composables/useDirectoryFiles'
import NormalCard from './NormalCard.vue'
import FileDetailSidebar from './FileDetailSidebar.vue'

const props = defineProps<{
  show: boolean
  initialPath: string
}>()

defineEmits<{
  close: []
}>()

const { t } = useI18n()
const { openInExplorer } = useDirectoryFiles()

// ─── 路径栈 ───────────────────────────────────────────
const pathStack = ref<string[]>([])
const files = ref<FileEntry[]>([])
const loading = ref(false)

const currentPath = computed(() => pathStack.value[pathStack.value.length - 1] ?? '')

/** 面包屑：从 initialPath 起每层目录名 */
const breadcrumbs = computed(() =>
  pathStack.value.map((p, i) => ({
    label: p.split('\\').pop()!,
    path: p,
    isLast: i === pathStack.value.length - 1,
  }))
)

async function loadCurrentDir() {
  const dir = currentPath.value
  if (!dir) return
  loading.value = true
  try {
    files.value = await invoke<FileEntry[]>('scan_directory', { dirPath: dir })
  } catch {
    files.value = []
  } finally {
    loading.value = false
  }
  selectedFile.value = null
}

function enterFolder(folder: FileEntry) {
  pathStack.value = [...pathStack.value, folder.path]
  loadCurrentDir()
}

function navigateTo(index: number) {
  if (index < pathStack.value.length - 1) {
    pathStack.value = pathStack.value.slice(0, index + 1)
    loadCurrentDir()
  }
}

function goBack() {
  if (pathStack.value.length > 1) {
    pathStack.value = pathStack.value.slice(0, -1)
    loadCurrentDir()
  }
}

// ─── 文件选中（侧边栏） ──────────────────────────────
const selectedFile = ref<FileEntry | null>(null)

function onCardClick(file: FileEntry) {
  if (file.is_dir) {
    enterFolder(file)
    return
  }
  if (selectedFile.value?.path === file.path) {
    selectedFile.value = null
  } else {
    selectedFile.value = file
  }
}

function onBodyClick(e: MouseEvent) {
  if (!(e.target as HTMLElement).closest('.normal-card')) {
    selectedFile.value = null
  }
}

// ─── 弹窗尺寸（比例 + 拖拽调整 + 持久化） ─────────────
const STORAGE_KEY = 'pgb1-folder-browser-size'
const MIN_PCT = 40
const MAX_PCT = 95

function clamp(v: number) {
  return Math.max(MIN_PCT, Math.min(MAX_PCT, v))
}

function loadSavedSize(): { w: number; h: number } {
  try {
    const raw = localStorage.getItem(STORAGE_KEY)
    if (raw) {
      const parsed = JSON.parse(raw)
      return { w: clamp(parsed.w), h: clamp(parsed.h) }
    }
  } catch { /* 忽略 */ }
  return { w: 70, h: 75 }
}

const saved = loadSavedSize()
const dialogWidth = ref(saved.w)
const dialogHeight = ref(saved.h)

function saveSize() {
  localStorage.setItem(STORAGE_KEY, JSON.stringify({ w: dialogWidth.value, h: dialogHeight.value }))
}

function onResizeStart(e: MouseEvent, edge: string) {
  e.preventDefault()
  const startX = e.clientX
  const startY = e.clientY
  const startW = dialogWidth.value
  const startH = dialogHeight.value
  const vw = window.innerWidth
  const vh = window.innerHeight

  function onMove(ev: MouseEvent) {
    const dx = ev.clientX - startX
    const dy = ev.clientY - startY
    if (edge.includes('e')) dialogWidth.value = clamp(startW + (dx / vw) * 100)
    if (edge.includes('w')) dialogWidth.value = clamp(startW - (dx / vw) * 100)
    if (edge.includes('s')) dialogHeight.value = clamp(startH + (dy / vh) * 100)
    if (edge.includes('n')) dialogHeight.value = clamp(startH - (dy / vh) * 100)
  }

  function onUp() {
    saveSize()
    window.removeEventListener('mousemove', onMove)
    window.removeEventListener('mouseup', onUp)
  }

  window.addEventListener('mousemove', onMove)
  window.addEventListener('mouseup', onUp)
}

// ─── show 变化时初始化 ────────────────────────────────
watch(() => props.show, (v) => {
  if (v) {
    pathStack.value = [props.initialPath]
    selectedFile.value = null
    loadCurrentDir()
  }
})
</script>

<template>
  <Teleport to="body">
    <Transition name="dialog">
      <div v-if="show" class="fb-overlay" @click.self="$emit('close')">
        <div
          class="fb-dialog glass-strong"
          :style="{ width: dialogWidth + 'vw', height: dialogHeight + 'vh' }"
        >
          <!-- 拖拽调整尺寸手柄（8 方向） -->
          <div class="fb-resize fb-resize-n" @mousedown="onResizeStart($event, 'n')" />
          <div class="fb-resize fb-resize-s" @mousedown="onResizeStart($event, 's')" />
          <div class="fb-resize fb-resize-e" @mousedown="onResizeStart($event, 'e')" />
          <div class="fb-resize fb-resize-w" @mousedown="onResizeStart($event, 'w')" />
          <div class="fb-resize fb-resize-ne" @mousedown="onResizeStart($event, 'ne')" />
          <div class="fb-resize fb-resize-nw" @mousedown="onResizeStart($event, 'nw')" />
          <div class="fb-resize fb-resize-se" @mousedown="onResizeStart($event, 'se')" />
          <div class="fb-resize fb-resize-sw" @mousedown="onResizeStart($event, 'sw')" />

          <!-- 顶部：返回 + 面包屑 + 操作按钮 -->
          <div class="fb-header">
            <button
              class="fb-back-btn"
              :disabled="pathStack.length <= 1"
              @click="goBack"
            >
              <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                <polyline points="15 18 9 12 15 6" />
              </svg>
            </button>
            <div class="fb-breadcrumbs">
              <template v-for="(crumb, i) in breadcrumbs" :key="crumb.path">
                <span v-if="i > 0" class="fb-separator">/</span>
                <button
                  class="fb-crumb"
                  :class="{ active: crumb.isLast }"
                  :disabled="crumb.isLast"
                  @click="navigateTo(i)"
                >{{ crumb.label }}</button>
              </template>
              <button
                class="folder-btn"
                :title="t('common.openFolder')"
                @click="openInExplorer(currentPath)"
              >
                <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                  <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z" />
                </svg>
              </button>
            </div>
            <button class="fb-action-btn fb-close-btn" @click="$emit('close')">
              <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                <line x1="18" y1="6" x2="6" y2="18" />
                <line x1="6" y1="6" x2="18" y2="18" />
              </svg>
            </button>
          </div>

          <!-- 内容区：卡片网格 + 可选侧边栏 -->
          <div ref="bodyRef" class="fb-body" @click="onBodyClick">
            <div class="fb-content">
              <p v-if="loading" class="fb-status">{{ t('common.scanning') }}</p>
              <p v-else-if="files.length === 0" class="fb-status">{{ t('folderBrowser.empty') }}</p>
              <TransitionGroup v-else name="card" tag="div" class="card-grid">
                <NormalCard
                  v-for="(file, i) in files"
                  :key="file.path"
                  :style="{ '--delay': i * 30 + 'ms' }"
                  :file="file"
                  :class="{ selected: selectedFile?.path === file.path }"
                  @click="onCardClick(file)"
                />
              </TransitionGroup>
            </div>

            <!-- 侧边栏：就地渲染，禁用 Teleport（避免弹窗关闭时卸载顺序崩溃） -->
            <FileDetailSidebar
              :file="selectedFile"
              :width-percent="35"
              teleport-disabled
              @close="selectedFile = null"
            />
          </div>
        </div>
      </div>
    </Transition>
  </Teleport>
</template>

<style scoped>
/* ─── 遮罩层 ─── */
.fb-overlay {
  position: fixed;
  inset: 0;
  z-index: var(--z-modal, 1000);
  display: flex;
  align-items: center;
  justify-content: center;
  background: var(--overlay-backdrop);
  backdrop-filter: blur(var(--glass-light-blur));
}

/* ─── 弹窗主体 ─── */
.fb-dialog {
  position: relative;
  display: flex;
  flex-direction: column;
  border-radius: var(--floating-navbar-radius);
  overflow: hidden;
}

/* ─── 顶部导航栏 ─── */
.fb-header {
  display: flex;
  align-items: center;
  gap: var(--spacing-2);
  padding: var(--spacing-3) var(--spacing-4);
  border-bottom: 1px solid var(--border-light);
  flex-shrink: 0;
  position: relative;
  z-index: 21; /* 高于 resize handle (z-index:20)，保证按钮可点击 */
}

.fb-back-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 28px;
  height: 28px;
  border-radius: var(--radius-md);
  border: 1px solid var(--border-heavy);
  background: transparent;
  color: var(--text-primary);
  cursor: pointer;
  transition: all var(--duration-fast);
  flex-shrink: 0;
}

.fb-back-btn:hover:not(:disabled) {
  background: var(--bg-hover);
  border-color: var(--color-primary);
  color: var(--color-primary);
}

.fb-back-btn:disabled {
  opacity: 0.25;
  cursor: default;
}

/* ─── 面包屑 ─── */
.fb-breadcrumbs {
  display: flex;
  align-items: center;
  gap: var(--spacing-1);
  flex: 1;
  min-width: 0;
  overflow-x: auto;
  scrollbar-width: none;
}

.fb-breadcrumbs::-webkit-scrollbar {
  display: none;
}

.fb-separator {
  color: var(--text-tertiary);
  font-size: var(--text-sm);
  flex-shrink: 0;
}

.fb-crumb {
  background: none;
  border: none;
  font-family: inherit;
  font-size: var(--text-base);
  color: var(--text-secondary);
  cursor: pointer;
  padding: var(--spacing-1) var(--spacing-2);
  border-radius: var(--radius-sm);
  transition: all var(--duration-fast);
  white-space: nowrap;
  flex-shrink: 0;
}

.fb-crumb:hover:not(:disabled) {
  background: var(--bg-hover);
  color: var(--text-primary);
}

.fb-crumb.active {
  color: var(--text-primary);
  font-weight: var(--font-weight-heading);
  cursor: default;
}

/* ─── 操作按钮 ─── */
.fb-action-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 28px;
  height: 28px;
  border-radius: var(--radius-md);
  border: 1px solid var(--border-heavy);
  background: transparent;
  color: var(--text-primary);
  cursor: pointer;
  transition: all var(--duration-fast);
  flex-shrink: 0;
}

.fb-action-btn:hover {
  background: var(--bg-hover);
  border-color: var(--color-primary);
  color: var(--color-primary);
}

.fb-close-btn:hover {
  background: var(--color-danger);
  color: var(--color-neutral-0);
  border-color: var(--color-danger);
}

/* ─── 内容区 ─── */
.fb-body {
  flex: 1;
  display: flex;
  overflow: hidden;
  min-height: 0;
}

.fb-content {
  flex: 1;
  overflow-y: auto;
  padding: var(--spacing-4);
  min-width: 0;
}

.fb-status {
  font-size: var(--text-lg);
  color: var(--text-tertiary);
  text-align: center;
  padding: var(--spacing-8) 0;
}

.card-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(var(--card-normal-width), 1fr));
  gap: var(--gap-card);
}

/* ─── 拖拽调整尺寸手柄 ─── */
.fb-resize {
  position: absolute;
  z-index: 20;
}

/* 四条边 */
.fb-resize-n { top: 0; left: 8px; right: 8px; height: 5px; cursor: n-resize; }
.fb-resize-s { bottom: 0; left: 8px; right: 8px; height: 5px; cursor: s-resize; }
.fb-resize-e { top: 8px; right: 0; bottom: 8px; width: 5px; cursor: e-resize; }
.fb-resize-w { top: 8px; left: 0; bottom: 8px; width: 5px; cursor: w-resize; }

/* 四个角 */
.fb-resize-ne { top: 0; right: 0; width: 10px; height: 10px; cursor: ne-resize; }
.fb-resize-nw { top: 0; left: 0; width: 10px; height: 10px; cursor: nw-resize; }
.fb-resize-se { bottom: 0; right: 0; width: 10px; height: 10px; cursor: se-resize; }
.fb-resize-sw { bottom: 0; left: 0; width: 10px; height: 10px; cursor: sw-resize; }
</style>
