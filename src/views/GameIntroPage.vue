<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { invoke } from '@tauri-apps/api/core'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { startDrag } from '@crabnebula/tauri-plugin-drag'
import { useNavigation } from '../composables/useNavigation'
import { useProjects } from '../composables/useProjects'
import { useDirectoryFiles, type FileEntry } from '../composables/useDirectoryFiles'
import { useNotes, toggleCheckbox } from '../composables/useNotes'
import NormalCard from '../components/NormalCard.vue'
import NoteDialog from '../components/NoteDialog.vue'
import NoteRenderer from '../components/NoteRenderer.vue'
import FileDetailSidebar from '../components/FileDetailSidebar.vue'
import FolderBrowserDialog from '../components/FolderBrowserDialog.vue'
import { useRubberBandSelect } from '../composables/useRubberBandSelect'
import { useI18n } from 'vue-i18n'
import PageGuideOverlay from '../components/PageGuideOverlay.vue'
import { PAGE_GUIDE_ANNOTATIONS } from '../config/onboarding'

const { t } = useI18n()
const route = useRoute()
const router = useRouter()
const { setNavigation } = useNavigation()
const { projects, loadProjects } = useProjects()
const { files, loading, loadFiles, openInExplorer } = useDirectoryFiles()

const projectId = route.params.projectId as string

let dirPath = ''
const dirPathRef = ref('')
const showGuide = ref(false)

// 笔记
const { loadNotes, hasNote, saveNote, getNote } = useNotes(dirPathRef)
const showPageNote = ref(false)
const pageNoteText = ref('')

function onPageNoteCheckbox(key: string, lineIndex: number) {
  const raw = getNote(key) ?? ''
  const updated = toggleCheckbox(raw, lineIndex)
  saveNote(key, updated)
}

/** 侧边栏选中文件 */
const selectedFile = ref<FileEntry | null>(null)
const sidebarWidth = ref(30)

/** 文件夹浏览弹窗 */
const showFolderBrowser = ref(false)
const folderBrowserPath = ref('')

/** 游戏原型启动程序路径（null = 未检测到，支持 Unity / Godot） */
const gameExePath = ref<string | null>(null)

const scrollRef = ref<HTMLElement | null>(null)
const isMultiSelect = ref(false)
const selectedPaths = ref<Set<string>>(new Set())

const isAllSelected = computed(() =>
  files.value.filter(f => !f.is_dir).length > 0 &&
  files.value.filter(f => !f.is_dir).every(f => selectedPaths.value.has(f.path))
)

function toggleMultiSelect() {
  if (isMultiSelect.value) {
    isMultiSelect.value = false
    selectedPaths.value = new Set()
  } else {
    isMultiSelect.value = true
    selectedFile.value = null
  }
}

function toggleSelectAll() {
  const fileOnly = files.value.filter(f => !f.is_dir)
  if (isAllSelected.value) {
    selectedPaths.value = new Set()
  } else {
    selectedPaths.value = new Set(fileOnly.map(f => f.path))
  }
}

function toggleFileSelection(file: FileEntry) {
  const newSet = new Set(selectedPaths.value)
  if (newSet.has(file.path)) {
    newSet.delete(file.path)
  } else {
    newSet.add(file.path)
  }
  selectedPaths.value = newSet
}

const { isSelecting, selectionRect, justFinished, onContainerMouseDown, onContainerScroll } =
  useRubberBandSelect({
    containerRef: scrollRef,
    cardSelector: '.normal-card[data-path]',
    isEnabled: isMultiSelect,
    onSelect: (paths) => { selectedPaths.value = paths },
  })

function onCardClick(file: FileEntry) {
  if (file.is_dir) {
    folderBrowserPath.value = file.path
    showFolderBrowser.value = true
    return
  }
  if (isMultiSelect.value) {
    toggleFileSelection(file)
    return
  }
  if (selectedFile.value?.path === file.path) {
    selectedFile.value = null
  } else {
    selectedFile.value = file
  }
}

function onMainClick(e: MouseEvent) {
  if (justFinished.value) return
  if (!(e.target as HTMLElement).closest('.normal-card')) {
    selectedFile.value = null
  }
}

async function onPageNoteSave(text: string) {
  await saveNote('page', text)
  showPageNote.value = false
}

/** 页面笔记 checkbox 切换：静默保存，不关闭弹窗 */
async function onPageNoteUpdate(text: string) {
  pageNoteText.value = text
  await saveNote('page', text)
}

async function onSidebarNoteSave(text: string) {
  const file = selectedFile.value
  if (!file) return
  await saveNote('card:' + file.name.toLowerCase(), text)
}

async function onSidebarRename(newName: string) {
  const file = selectedFile.value
  if (!file) return
  try {
    await invoke('rename_file', { path: file.path, newName })
    selectedFile.value = null
    await loadFiles(dirPath)
  } catch (e) {
    console.error('重命名失败:', e)
  }
}

async function onSidebarDelete() {
  const file = selectedFile.value
  if (!file) return
  try {
    await invoke('delete_file', { path: file.path })
    selectedFile.value = null
    await loadFiles(dirPath)
  } catch (e) {
    console.error('删除失败:', e)
  }
}

// ─── 拖入/拖出 ──────────────────────────────────────

const isDragOver = ref(false)
let unlistenDragDrop: (() => void) | null = null

const DRAG_THRESHOLD = 5

/** 卡片拖出：mousedown + 移动阈值 */
function onCardMouseDown(e: MouseEvent, file: FileEntry) {
  if (e.button !== 0 || file.is_dir) return

  const startX = e.clientX
  const startY = e.clientY
  let dragStarted = false

  function onMouseMove(ev: MouseEvent) {
    if (dragStarted) return
    const dx = ev.clientX - startX
    const dy = ev.clientY - startY
    if (Math.sqrt(dx * dx + dy * dy) > DRAG_THRESHOLD) {
      dragStarted = true
      cleanup()
      if (isMultiSelect.value) {
        if (!selectedPaths.value.has(file.path)) {
          const newSet = new Set(selectedPaths.value)
          newSet.add(file.path)
          selectedPaths.value = newSet
        }
        const paths = [...selectedPaths.value]
        if (paths.length > 0) {
          startDrag({ item: paths, icon: '' }).catch(err => console.error('拖拽失败:', err))
        }
      } else {
        startDrag({ item: [file.path], icon: '' }).catch(err => console.error('拖拽失败:', err))
      }
    }
  }

  function onMouseUp() { cleanup() }
  function cleanup() {
    document.removeEventListener('mousemove', onMouseMove)
    document.removeEventListener('mouseup', onMouseUp)
  }
  document.addEventListener('mousemove', onMouseMove)
  document.addEventListener('mouseup', onMouseUp)
}

/** 拖入处理 */
async function handleFileDrop(paths: string[]) {
  if (!dirPath || paths.length === 0) return
  const scrollEl = document.querySelector('.game-intro-page .scroll-content')
  const scrollPos = scrollEl?.scrollTop ?? 0
  try {
    await invoke('import_files', { sourcePaths: paths, targetDir: dirPath })
    await loadFiles(dirPath)
    await nextTick()
    if (scrollEl) scrollEl.scrollTop = scrollPos
  } catch (err) {
    console.error('导入文件失败:', err)
  }
}

/** 递归扫描 00_Game Design & Doc，寻找游戏原型启动程序（Unity / Godot） */
async function scanGameExe() {
  if (!dirPath) return
  try {
    const exePath = await invoke<string | null>('find_game_exe', { rootDir: dirPath })
    gameExePath.value = exePath ?? null
  } catch {
    gameExePath.value = null
  }
}

/** 注册/更新顶部导航配置 */
function refreshNav() {
  const actions = [
    ...(gameExePath.value
      ? [{ id: 'launch-game', label: t('gameIntro.launchPrototype'), handler: () => { invoke('open_file', { path: gameExePath.value! }) } }]
      : []
    ),
  ]
  setNavigation({
    title: t('gameIntro.title'),
    showBackButton: true,
    onBack: () => router.push({ name: 'project', params: { projectId } }),
    actions,
    moreMenuItems: [
      { id: 'refresh', label: t('common.refresh'), handler: () => { if (dirPath) loadFiles(dirPath) } },
      { id: 'page-guide', label: t('common.pageGuide'), handler: () => { showGuide.value = true } },
    ],
  })
}

/* 初始注册导航（无启动按钮） */
refreshNav()

onMounted(async () => {
  await loadProjects()
  const project = projects.value.find(p => p.name === projectId)
  if (project) {
    dirPath = `${project.path}\\00_Game Design & Doc`
    dirPathRef.value = dirPath
    await loadFiles(dirPath)
    await scanGameExe()
    if (gameExePath.value) refreshNav()
    await loadNotes()
    pageNoteText.value = getNote('page') ?? ''
  }

  // 监听外部文件拖入
  const appWindow = getCurrentWindow()
  const unlisten = await appWindow.onDragDropEvent((event) => {
    if (event.payload.type === 'over') {
      isDragOver.value = true
    } else if (event.payload.type === 'leave') {
      isDragOver.value = false
    } else if (event.payload.type === 'drop') {
      isDragOver.value = false
      handleFileDrop(event.payload.paths)
    }
  })
  unlistenDragDrop = unlisten
})

onUnmounted(() => {
  if (unlistenDragDrop) {
    unlistenDragDrop()
    unlistenDragDrop = null
  }
})
</script>

<template>
  <div class="game-intro-page" :class="{ 'drag-over': isDragOver }" @click="onMainClick">
    <!-- 固定小标题栏 -->
    <div class="sub-title-bar">
      <span class="sub-title">{{ $t('gameIntro.docTitle') }}</span>
      <button
        class="folder-btn"
        :title="$t('common.openFolder')"
        @click="openInExplorer(dirPath)"
      >
        <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z" />
        </svg>
      </button>
      <div v-if="hasNote('page')" class="note-preview-inline">
        <NoteRenderer :text="getNote('page')!" @toggle-checkbox="onPageNoteCheckbox('page', $event)" />
      </div>
      <button
        class="note-btn"
        :class="{ 'has-note': hasNote('page') }"
        :title="$t('note.pageNote')"
        @click="pageNoteText = getNote('page') ?? ''; showPageNote = true"
      >
        <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M12 20h9"/><path d="M16.5 3.5a2.121 2.121 0 0 1 3 3L7 19l-4 1 1-4L16.5 3.5z"/></svg>
      </button>
      <div class="view-buttons">
        <button class="view-btn" @click="() => { if (dirPath) loadFiles(dirPath) }">{{ $t('common.refresh') }}</button>
        <button
          class="view-btn"
          :class="{ active: isMultiSelect }"
          @click="toggleMultiSelect"
        >
          {{ isMultiSelect ? $t('common.multiSelectOn') : $t('common.multiSelect') }}
        </button>
        <button
          v-if="isMultiSelect"
          class="view-btn"
          @click="toggleSelectAll"
        >
          {{ isAllSelected ? $t('common.deselectAll') : $t('common.selectAll') }}
        </button>
      </div>
    </div>

    <!-- 可滚动内容区 -->
    <div
      ref="scrollRef"
      class="scroll-content"
      @mousedown="onContainerMouseDown"
      @scroll="onContainerScroll"
    >
      <p v-if="loading" class="loading-text">{{ $t('common.scanning') }}</p>

      <p v-else-if="files.length === 0" class="empty-text">{{ $t('gameIntro.noFiles') }}</p>

      <TransitionGroup v-else name="card" tag="div" class="card-grid">
        <NormalCard
          v-for="(file, i) in files"
          :key="file.name"
          :style="{ '--delay': i * 40 + 'ms' }"
          :file="file"
          :multi-select="isMultiSelect"
          :checked="selectedPaths.has(file.path)"
          :has-note="hasNote('card:' + file.name.toLowerCase())"
          :note-preview="getNote('card:' + file.name.toLowerCase()) ?? ''"
          :class="{
            selected: !isMultiSelect && selectedFile?.path === file.path,
            'multi-checked': isMultiSelect && selectedPaths.has(file.path),
          }"
          @click="onCardClick(file)"
          @mousedown="onCardMouseDown($event, file)"
        />
      </TransitionGroup>
    </div>

    <!-- 文件详情侧边栏 -->
    <FileDetailSidebar
      :file="selectedFile"
      :width-percent="sidebarWidth"
      allow-actions
      :note="selectedFile ? (getNote('card:' + selectedFile.name.toLowerCase()) ?? undefined) : undefined"
      @close="selectedFile = null"
      @update:width-percent="sidebarWidth = $event"
      @rename="onSidebarRename"
      @delete="onSidebarDelete"
      @save-note="onSidebarNoteSave"
    />

    <!-- 拖入视觉反馈 -->
    <div v-if="isDragOver" class="drop-overlay">
      <div class="drop-hint glass-medium">
        <svg width="48" height="48" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
          <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4" />
          <polyline points="17 8 12 3 7 8" />
          <line x1="12" y1="3" x2="12" y2="15" />
        </svg>
        <span>{{ $t('gameIntro.dropHint') }}</span>
      </div>
    </div>
  </div>

  <!-- 框选矩形覆盖层 -->
  <Teleport to="body">
    <div
      v-if="isSelecting && selectionRect"
      class="rubber-band-overlay"
      :style="{
        left: selectionRect.left + 'px',
        top: selectionRect.top + 'px',
        width: (selectionRect.right - selectionRect.left) + 'px',
        height: (selectionRect.bottom - selectionRect.top) + 'px',
      }"
    />
  </Teleport>

  <NoteDialog
    :show="showPageNote"
    :title="$t('note.pageNote')"
    :note="pageNoteText"
    @save="onPageNoteSave"
    @update="onPageNoteUpdate"
    @cancel="showPageNote = false"
  />

  <PageGuideOverlay :show="showGuide" :annotations="PAGE_GUIDE_ANNOTATIONS.gameIntro" @close="showGuide = false" />

  <FolderBrowserDialog
    :show="showFolderBrowser"
    :initial-path="folderBrowserPath"
    @close="showFolderBrowser = false"
  />
</template>

<style scoped>
.game-intro-page {
  display: flex;
  flex-direction: column;
  height: 100%;
  overflow: hidden;
}

/* 固定小标题栏 */
/* .sub-title-bar, .sub-title → design-system.css 公共类 */

.view-buttons {
  display: flex;
  gap: var(--spacing-2);
  margin-left: auto;
}

.view-btn {
  display: inline-flex;
  align-items: center;
  height: var(--button-height-sm);
  padding: 0 var(--spacing-3);
  font-size: var(--text-sm);
  border-radius: var(--radius-md);
  border: 1px solid var(--border-medium);
  background: transparent;
  color: var(--text-secondary);
  cursor: pointer;
  transition: all var(--transition-fast);
}

.view-btn:hover {
  background: var(--bg-hover);
  color: var(--text-primary);
}

/* 可滚动区 */
.scroll-content {
  flex: 1;
  overflow-y: auto;
  padding-top: var(--spacing-4);
}

.loading-text,
.empty-text {
  font-size: var(--text-lg);
  color: var(--text-tertiary);
}

.card-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(var(--card-normal-width), 1fr));
  gap: var(--gap-card);
}

/* 拖入反馈 */
.game-intro-page {
  position: relative;
}

.game-intro-page.drag-over {
  outline: 2px dashed var(--color-primary);
  outline-offset: -2px;
  border-radius: var(--radius-lg);
}

.drop-overlay {
  position: absolute;
  inset: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  background: var(--overlay-backdrop);
  backdrop-filter: blur(var(--glass-light-blur));
  border-radius: var(--radius-lg);
  z-index: 10;
}

.drop-hint {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: var(--spacing-3);
  padding: var(--spacing-8);
  border-radius: var(--floating-navbar-radius);
  color: var(--text-primary);
  font-size: var(--text-xl);
  font-weight: var(--font-weight-heading);
}

.view-btn.active {
  background: var(--bg-active);
  color: var(--text-primary);
  border-color: var(--border-heavy);
}

</style>
