<script setup lang="ts">
import { nextTick, ref, computed, onMounted, onUnmounted } from 'vue'
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
const { openInExplorer } = useDirectoryFiles()
const showGuide = ref(false)

const projectId = route.params.projectId as string

/** 子分组（flatten 展开用） */
interface SubGroup {
  label: string
  dirPath: string
  files: FileEntry[]
}

/** 素材分组 */
interface MaterialGroup {
  label: string
  dirPath: string
  files: FileEntry[]
  subGroups?: SubGroup[]
}

const groups = ref<MaterialGroup[]>([])
const loading = ref(false)

let projectPath = ''
const projectPathRef = ref('')

// 笔记（主标题级，key 'page:materials'）
const { loadNotes: loadPageNotes, hasNote: hasPageNote, getNote: getPageNote, saveNote: savePageNote } = useNotes(projectPathRef)
const showPageNote = ref(false)
const pageNoteText = ref('')

async function openPinboard() {
  if (!projectPathRef.value) return
  await invoke('open_pinboard_window', {
    dirPath: projectPathRef.value,
    canvasKey: 'materials',
    title: t('materialsPage.title'),
  })
}

function onPageNoteCheckbox(key: string, lineIndex: number) {
  const raw = getPageNote(key) ?? ''
  const updated = toggleCheckbox(raw, lineIndex)
  savePageNote(key, updated)
}

// 各分组笔记缓存：dirPath → notes map
const groupNotesMap = ref<Record<string, Record<string, string>>>({})

/** 文件夹浏览弹窗 */
const showFolderBrowser = ref(false)
const folderBrowserPath = ref('')

/** 侧边栏选中文件 */
const selectedFile = ref<FileEntry | null>(null)
const sidebarWidth = ref(30)

const scrollRef = ref<HTMLElement | null>(null)
const isMultiSelect = ref(false)
const selectedPaths = ref<Set<string>>(new Set())

// 从所有分组中收集所有文件（用于全选，排除目录）
const allFiles = computed(() => {
  const result: FileEntry[] = []
  for (const g of groups.value) {
    result.push(...g.files.filter(f => !f.is_dir))
    if (g.subGroups) {
      for (const sg of g.subGroups) {
        result.push(...sg.files.filter(f => !f.is_dir))
      }
    }
  }
  return result
})

const isAllSelected = computed(() =>
  allFiles.value.length > 0 && allFiles.value.every(f => selectedPaths.value.has(f.path))
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
  if (isAllSelected.value) {
    selectedPaths.value = new Set()
  } else {
    selectedPaths.value = new Set(allFiles.value.map(f => f.path))
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

/** 查找文件所在分组 dirPath */
function findGroupDirForFile(filePath: string): string | null {
  for (const g of groups.value) {
    if (g.files.some(f => f.path === filePath)) return g.dirPath
    if (g.subGroups) {
      for (const sg of g.subGroups) {
        if (sg.files.some(f => f.path === filePath)) return sg.dirPath
      }
    }
  }
  return null
}

function groupHasNote(dirPath: string, fileName: string): boolean {
  return !!(groupNotesMap.value[dirPath]?.['card:' + fileName.toLowerCase()])
}

function groupNotePreview(dirPath: string, fileName: string): string {
  return groupNotesMap.value[dirPath]?.['card:' + fileName.toLowerCase()] ?? ''
}

function getFileNote(file: FileEntry): string | undefined {
  const dir = findGroupDirForFile(file.path)
  if (!dir) return undefined
  return groupNotesMap.value[dir]?.['card:' + file.name.toLowerCase()] ?? undefined
}

async function onSidebarNoteSave(text: string) {
  const file = selectedFile.value
  if (!file) return
  const dir = findGroupDirForFile(file.path)
  if (!dir) return
  await invoke('set_note', { dirPath: dir, key: 'card:' + file.name.toLowerCase(), note: text || null })
  // 刷新该分组笔记缓存
  try {
    const notes = await invoke<Record<string, string>>('get_notes', { dirPath: dir })
    groupNotesMap.value = { ...groupNotesMap.value, [dir]: notes }
  } catch { /* 忽略 */ }
}

async function onPageNoteSave(text: string) {
  await savePageNote('page:materials', text)
  showPageNote.value = false
}

/** 页面笔记 checkbox 切换：静默保存，不关闭弹窗 */
async function onPageNoteUpdate(text: string) {
  pageNoteText.value = text
  await savePageNote('page:materials', text)
}

async function onSidebarRename(newName: string) {
  const file = selectedFile.value
  if (!file) return
  try {
    await invoke('rename_file', { path: file.path, newName })
    selectedFile.value = null
    await refreshAll()
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
    await refreshAll()
  } catch (e) {
    console.error('删除失败:', e)
  }
}

/* 注册项目素材页导航配置 */
setNavigation({
  title: t('materialsPage.title'),
  showBackButton: true,
  onBack: () => router.push({ name: 'project', params: { projectId } }),
  actions: [
    { id: 'game-intro', label: t('project.gameIntro'), handler: () => router.push({ name: 'gameIntro', params: { projectId } }) },
  ],
  moreMenuItems: [
    { id: 'refresh', label: t('common.refresh'), handler: refreshAll },
    { id: 'page-guide', label: t('common.pageGuide'), handler: () => { showGuide.value = true } },
  ],
})

/** 素材目录配置 */
const DIR_CONFIG = [
  { label: '01_Preproduction', subPath: '01_Preproduction', flatten: false },
  { label: '02_Production', subPath: '02_Production', flatten: false },
  { label: '03_Render_VFX / VFX / PSD', subPath: '03_Render_VFX\\VFX\\PSD', flatten: true },
  { label: '05_Outside', subPath: '05_Outside', flatten: false },
]

async function refreshAll() {
  if (!projectPath) return
  loading.value = true

  const result: MaterialGroup[] = []

  for (const config of DIR_CONFIG) {
    const dirPath = `${projectPath}\\${config.subPath}`
    try {
      const files = await invoke<FileEntry[]>('scan_directory', { dirPath })

      if (config.flatten) {
        // 展开子文件夹：每个子目录作为一个子分组
        const subGroups: SubGroup[] = []
        const rootFiles: FileEntry[] = []
        for (const f of files) {
          if (f.is_dir) {
            try {
              const subFiles = await invoke<FileEntry[]>('scan_directory', { dirPath: f.path })
              if (subFiles.length > 0) {
                subGroups.push({ label: f.name, dirPath: f.path, files: subFiles })
              }
            } catch { /* 子目录扫描失败跳过 */ }
          } else {
            rootFiles.push(f)
          }
        }
        result.push({ label: config.label, dirPath, files: rootFiles, subGroups })
      } else {
        result.push({ label: config.label, dirPath, files })
      }
    } catch {
      // 目录不存在（如深层子目录），仍渲染为空分组——拖入时 import_files 会自动创建
      result.push({ label: config.label, dirPath, files: [] })
    }
  }

  groups.value = result
  loading.value = false

  // 加载各分组笔记
  const notesMap: Record<string, Record<string, string>> = {}
  for (const g of result) {
    try {
      notesMap[g.dirPath] = await invoke<Record<string, string>>('get_notes', { dirPath: g.dirPath })
    } catch { notesMap[g.dirPath] = {} }
    if (g.subGroups) {
      for (const sg of g.subGroups) {
        try {
          notesMap[sg.dirPath] = await invoke<Record<string, string>>('get_notes', { dirPath: sg.dirPath })
        } catch { notesMap[sg.dirPath] = {} }
      }
    }
  }
  groupNotesMap.value = notesMap
}

// ─── 拖入/拖出 ──────────────────────────────────────

const isDragOver = ref(false)
const dropTargetLabel = ref('')
let unlistenDragDrop: (() => void) | null = null

const DRAG_THRESHOLD = 5

/** 卡片拖出 */
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

/** 根据 Y 坐标找到对应分组 */
function findGroupAtY(y: number): MaterialGroup | null {
  const sections = document.querySelectorAll('.material-group')
  for (let i = 0; i < sections.length; i++) {
    const rect = sections[i].getBoundingClientRect()
    if (y >= rect.top && y <= rect.bottom) {
      return groups.value[i] ?? null
    }
  }
  return null
}

/** 拖入处理 */
async function handleFileDrop(paths: string[], y: number) {
  if (paths.length === 0) return

  // 根据 Y 坐标找到对应分组
  const group = findGroupAtY(y)
  const targetDir = group?.dirPath

  if (!targetDir) return

  const scrollEl = document.querySelector('.materials-page .scroll-content')
  const scrollPos = scrollEl?.scrollTop ?? 0
  try {
    await invoke('import_files', { sourcePaths: paths, targetDir })
    await refreshAll()
    await nextTick()
    if (scrollEl) scrollEl.scrollTop = scrollPos
  } catch (err) {
    console.error('导入文件失败:', err)
  }
}

onMounted(async () => {
  await loadProjects()
  const project = projects.value.find(p => p.name === projectId)
  if (project) {
    projectPath = project.path
    projectPathRef.value = project.path
    await refreshAll()
    await loadPageNotes()
    pageNoteText.value = getPageNote('page:materials') ?? ''
  }

  // 监听外部文件拖入
  const appWindow = getCurrentWindow()
  const unlisten = await appWindow.onDragDropEvent((event) => {
    if (event.payload.type === 'over') {
      isDragOver.value = true
      const pos = event.payload.position
      const group = findGroupAtY(pos.y)
      dropTargetLabel.value = group?.label ?? ''
    } else if (event.payload.type === 'leave') {
      isDragOver.value = false
      dropTargetLabel.value = ''
    } else if (event.payload.type === 'drop') {
      isDragOver.value = false
      dropTargetLabel.value = ''
      handleFileDrop(event.payload.paths, event.payload.position.y)
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
  <div class="materials-page" :class="{ 'drag-over': isDragOver }" @click="onMainClick">
    <!-- 固定小标题栏 -->
    <div class="sub-title-bar">
      <span class="sub-title">{{ $t('materialsPage.materialFolders') }}</span>
      <div v-if="hasPageNote('page:materials')" class="note-preview-inline">
        <NoteRenderer :text="getPageNote('page:materials')!" @toggle-checkbox="onPageNoteCheckbox('page:materials', $event)" />
      </div>
      <button
        class="note-btn"
        :class="{ 'has-note': hasPageNote('page:materials') }"
        :title="$t('note.pageNote')"
        @click="pageNoteText = getPageNote('page:materials') ?? ''; showPageNote = true"
      >
        <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z" />
          <polyline points="14 2 14 8 20 8" />
          <line x1="16" y1="13" x2="8" y2="13" />
          <line x1="16" y1="17" x2="8" y2="17" />
        </svg>
      </button>
      <button
        class="note-btn"
        :title="$t('pinboard.title')"
        @click="openPinboard"
      >
        <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="3" y="3" width="18" height="18" rx="2"/><path d="M9 3v18"/><path d="M3 9h6"/></svg>
      </button>
      <div class="view-buttons">
        <button class="view-btn" @click="refreshAll">{{ $t('common.refresh') }}</button>
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

      <p v-else-if="groups.length === 0" class="empty-text">{{ $t('materialsPage.noMaterials') }}</p>

      <template v-else>
        <section
          v-for="group in groups"
          :key="group.label"
          class="material-group"
          :class="{ 'drop-target': isDragOver && dropTargetLabel === group.label }"
        >
          <div class="group-header">
            <h3 class="group-label">{{ group.label }}</h3>
            <button
              class="folder-btn"
              :title="$t('common.openFolder')"
              @click="openInExplorer(group.dirPath)"
            >
              <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z" />
              </svg>
            </button>
          </div>

          <!-- 普通文件（根级） -->
          <TransitionGroup v-if="group.files.length > 0" name="card" tag="div" class="card-grid">
            <NormalCard
              v-for="(file, i) in group.files"
              :key="file.name"
              :style="{ '--delay': i * 40 + 'ms' }"
              :file="file"
              :multi-select="isMultiSelect"
              :checked="selectedPaths.has(file.path)"
              :has-note="groupHasNote(group.dirPath, file.name)"
              :note-preview="groupNotePreview(group.dirPath, file.name)"
              :class="{ selected: !isMultiSelect && selectedFile?.path === file.path, 'multi-checked': isMultiSelect && selectedPaths.has(file.path) }"
              @click="onCardClick(file)"
              @mousedown="onCardMouseDown($event, file)"
            />
          </TransitionGroup>

          <!-- 空分组提示（新项目或目录为空时） -->
          <p
            v-if="group.files.length === 0 && !group.subGroups?.length"
            class="drop-hint"
          >{{ $t('materialsPage.dropHint') }}</p>

          <!-- 子分组（flatten 展开） -->
          <div v-if="group.subGroups" class="sub-groups">
            <div v-for="sub in group.subGroups" :key="sub.label" class="sub-group">
              <div class="sub-group-header">
                <span class="sub-group-label">{{ sub.label }}</span>
                <button
                  class="folder-btn"
                  :title="$t('common.openFolder')"
                  @click="openInExplorer(sub.dirPath)"
                >
                  <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                    <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z" />
                  </svg>
                </button>
              </div>
              <TransitionGroup name="card" tag="div" class="card-grid">
                <NormalCard
                  v-for="(file, i) in sub.files"
                  :key="file.name"
                  :style="{ '--delay': i * 40 + 'ms' }"
                  :file="file"
                  :multi-select="isMultiSelect"
                  :checked="selectedPaths.has(file.path)"
                  :has-note="groupHasNote(sub.dirPath, file.name)"
                  :note-preview="groupNotePreview(sub.dirPath, file.name)"
                  :class="{ selected: !isMultiSelect && selectedFile?.path === file.path, 'multi-checked': isMultiSelect && selectedPaths.has(file.path) }"
                  @click="onCardClick(file)"
                  @mousedown="onCardMouseDown($event, file)"
                />
              </TransitionGroup>
            </div>
          </div>
        </section>
      </template>
    </div>

    <!-- 文件详情侧边栏 -->
    <FileDetailSidebar
      :file="selectedFile"
      :width-percent="sidebarWidth"
      allow-actions
      :note="selectedFile ? getFileNote(selectedFile) : undefined"
      @close="selectedFile = null"
      @update:width-percent="sidebarWidth = $event"
      @rename="onSidebarRename"
      @delete="onSidebarDelete"
      @save-note="onSidebarNoteSave"
    />
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

  <PageGuideOverlay :show="showGuide" :annotations="PAGE_GUIDE_ANNOTATIONS.materials" @close="showGuide = false" />

  <FolderBrowserDialog
    :show="showFolderBrowser"
    :initial-path="folderBrowserPath"
    @close="showFolderBrowser = false"
  />
</template>

<style scoped>
.materials-page {
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
  padding: var(--spacing-4) var(--spacing-2) var(--spacing-2);
  display: flex;
  flex-direction: column;
  gap: var(--spacing-6);
}

.loading-text,
.empty-text {
  font-size: var(--text-lg);
  color: var(--text-tertiary);
}

.material-group {
  display: flex;
  flex-direction: column;
  gap: var(--spacing-3);
}

.group-header {
  display: flex;
  align-items: center;
  gap: var(--spacing-2);
}

/* .group-label, .folder-btn → design-system.css 公共类 */

.card-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(var(--card-normal-width), 1fr));
  gap: var(--gap-card);
}

/* 子分组 */
.sub-groups {
  display: flex;
  flex-direction: column;
  gap: var(--spacing-5);
}

.sub-group {
  display: flex;
  flex-direction: column;
  gap: var(--spacing-2);
}

.sub-group-header {
  display: flex;
  align-items: center;
  gap: var(--spacing-2);
}

.sub-group-label {
  font-size: var(--text-xl);
  font-weight: var(--font-weight-heading);
  color: var(--text-tertiary);
}

/* 空分组拖入提示 */
.drop-hint {
  padding: var(--spacing-6) var(--spacing-4);
  font-size: var(--text-sm);
  color: var(--text-tertiary);
  border: 1px dashed var(--border-medium);
  border-radius: var(--radius-md);
  text-align: center;
}

/* 拖入反馈 */
.materials-page.drag-over {
  outline: 2px dashed var(--color-primary);
  outline-offset: -2px;
  border-radius: var(--radius-lg);
}

.material-group.drop-target {
  outline: 2px solid var(--color-primary);
  outline-offset: var(--spacing-2);
  border-radius: var(--radius-md);
  background: var(--bg-hover);
}

.view-btn.active {
  background: var(--bg-active);
  color: var(--text-primary);
  border-color: var(--border-heavy);
}

</style>
