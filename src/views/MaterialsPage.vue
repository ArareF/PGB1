<script setup lang="ts">
import { nextTick, ref, computed, onMounted, onUnmounted } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { invoke } from '@tauri-apps/api/core'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { startDrag } from '@crabnebula/tauri-plugin-drag'
import { useNavigation } from '../composables/useNavigation'
import { useProjects } from '../composables/useProjects'
import { useDirectoryFiles, type FileEntry } from '../composables/useDirectoryFiles'
import { useNotes, usePageNote } from '../composables/useNotes'
import { useMultiSelect } from '../composables/useMultiSelect'
import { createDragHandler } from '../composables/useDragIntent'
import NormalCard from '../components/NormalCard.vue'
import NoteDialog from '../components/NoteDialog.vue'
import NoteRenderer from '../components/NoteRenderer.vue'
import FileDetailSidebar from '../components/FileDetailSidebar.vue'
import FolderBrowserDialog from '../components/FolderBrowserDialog.vue'
import { useI18n } from 'vue-i18n'
import PageGuideOverlay from '../components/PageGuideOverlay.vue'
import { PAGE_GUIDE_ANNOTATIONS } from '../config/onboarding'
import { PSD_SUBPATH } from '../config/projectPaths'
import { groupIntoSeries, flattenVersions, parseSeriesName, type MaterialSeries } from '../utils/materialSeries'

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
  series: MaterialSeries[]
}

/** 素材分组 */
interface MaterialGroup {
  label: string
  dirPath: string
  series: MaterialSeries[]
  subGroups?: SubGroup[]
}

const groups = ref<MaterialGroup[]>([])
const loading = ref(false)

let projectPath = ''
const projectPathRef = ref('')

// 笔记（主标题级，key 'page:materials'）
const { loadNotes: loadPageNotes, hasNote: hasPageNote, getNote: getPageNote, saveNote: savePageNote } = useNotes(projectPathRef)
const { showPageNote, pageNoteText, openPageNote, closePageNote, onPageNoteSave, onPageNoteUpdate, onPageNoteCheckbox } =
  usePageNote(getPageNote, savePageNote, 'page:materials')

async function openPinboard() {
  if (!projectPathRef.value) return
  await invoke('open_pinboard_window', {
    dirPath: projectPathRef.value,
    canvasKey: 'materials',
    title: t('materialsPage.title'),
  })
}

// 各分组笔记缓存：dirPath → notes map
const groupNotesMap = ref<Record<string, Record<string, string>>>({})

/** 文件夹浏览弹窗 */
const showFolderBrowser = ref(false)
const folderBrowserPath = ref('')

/** 侧边栏选中的系列（卡片身份）与当前预览的具体版本文件 */
const selectedSeries = ref<MaterialSeries | null>(null)
const selectedFile = ref<FileEntry | null>(null)
const sidebarWidth = ref(30)

const scrollRef = ref<HTMLElement | null>(null)

/** 遍历所有分组的系列（含子分组），供全选 / 笔记查找复用 */
function* iterateSeries(): Generator<{ dirPath: string; series: MaterialSeries }> {
  for (const g of groups.value) {
    for (const s of g.series) yield { dirPath: g.dirPath, series: s }
    for (const sg of g.subGroups ?? []) {
      for (const s of sg.series) yield { dirPath: sg.dirPath, series: s }
    }
  }
}

// 全选范围 = 每个系列的主文件（合并卡代表最新版本，旧版本要单独操作请进侧边栏）
const allSelectablePaths = computed(() => {
  const result: string[] = []
  for (const { series } of iterateSeries()) {
    if (!series.primary.is_dir) result.push(series.primary.path)
  }
  return result
})

const {
  isMultiSelect, selectedPaths, isAllSelected,
  toggleMultiSelect, toggleSelection, toggleSelectAll,
  isSelecting, selectionRect, justFinished, onContainerMouseDown, onContainerScroll,
} = useMultiSelect({
  allPaths: allSelectablePaths,
  onEnter: () => { clearSelection() },
  rubberBand: { containerRef: scrollRef, cardSelector: '.normal-card[data-path]' },
})

function clearSelection() {
  selectedSeries.value = null
  selectedFile.value = null
}

function onCardClick(series: MaterialSeries) {
  if (series.primary.is_dir) {
    folderBrowserPath.value = series.primary.path
    showFolderBrowser.value = true
    return
  }
  if (isMultiSelect.value) {
    toggleSelection(series.primary.path)
    return
  }
  if (selectedSeries.value?.key === series.key) {
    clearSelection()
  } else {
    selectedSeries.value = series
    selectedFile.value = series.primary
  }
}

function onMainClick(e: MouseEvent) {
  if (justFinished.value) return
  if (!(e.target as HTMLElement).closest('.normal-card')) {
    clearSelection()
  }
}

/** 侧边栏版本列表：单文件系列不显示版本区 */
const sidebarVersions = computed(() =>
  selectedSeries.value && selectedSeries.value.fileCount > 1
    ? flattenVersions(selectedSeries.value)
    : undefined
)

/**
 * 版本条目标题：显示日期（格式由版本卡右侧的扩展名标签负责）。
 * 带尾缀的文件（如 `..._260706_---.psd`）附上尾缀，否则同日同格式的两行无法区分。
 */
function versionLabel(file: FileEntry): string {
  const parsed = parseSeriesName(file)
  if (!parsed) return file.name
  return parsed.date + parsed.suffix
}

/**
 * 卡片格式标签：最新版本包含多个格式时列出全部（如 PSD·JPG）。
 * 单格式返回 undefined，交回 NormalCard 默认逻辑（目录会显示 DIR）。
 */
function seriesFormatLabel(series: MaterialSeries): string | undefined {
  const exts = [...new Set(series.versions[0].files.map(f => f.extension).filter(Boolean))]
  return exts.length > 1 ? exts.map(e => e.toUpperCase()).join('·') : undefined
}

/** 查找文件所在分组 dirPath */
function findGroupDirForFile(filePath: string): string | null {
  for (const { dirPath, series } of iterateSeries()) {
    if (flattenVersions(series).some(f => f.path === filePath)) return dirPath
  }
  return null
}

function noteOf(dirPath: string, fileName: string): string {
  return groupNotesMap.value[dirPath]?.['card:' + fileName.toLowerCase()] ?? ''
}

/**
 * 系列笔记：主文件优先，否则取系列内第一条有笔记的版本。
 * 这样给旧版本写的笔记不会因为合并而在卡片上消失。
 */
function seriesNotePreview(dirPath: string, series: MaterialSeries): string {
  const primaryNote = noteOf(dirPath, series.primary.name)
  if (primaryNote) return primaryNote
  for (const file of flattenVersions(series)) {
    const note = noteOf(dirPath, file.name)
    if (note) return note
  }
  return ''
}

function seriesHasNote(dirPath: string, series: MaterialSeries): boolean {
  return !!seriesNotePreview(dirPath, series)
}

function getFileNote(file: FileEntry): string | undefined {
  const dir = findGroupDirForFile(file.path)
  if (!dir) return undefined
  // 保持 ?? 语义：key 不存在才返回 undefined，空串仍需显示笔记编辑区
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

async function onSidebarRename(newName: string) {
  const file = selectedFile.value
  if (!file) return
  try {
    await invoke('rename_file', { path: file.path, newName })
    clearSelection()
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
    clearSelection()
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
  { label: '03_Render_VFX / VFX / PSD', subPath: PSD_SUBPATH, flatten: true },
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
                subGroups.push({ label: f.name, dirPath: f.path, series: groupIntoSeries(subFiles) })
              }
            } catch { /* 子目录扫描失败跳过 */ }
          } else {
            rootFiles.push(f)
          }
        }
        result.push({ label: config.label, dirPath, series: groupIntoSeries(rootFiles), subGroups })
      } else {
        result.push({ label: config.label, dirPath, series: groupIntoSeries(files) })
      }
    } catch (e) {
      // 目录不存在（如深层子目录）是合法场景，debug 级记录即可——拖入时 import_files 会自动创建
      console.debug(`[MaterialsPage] 分组目录扫描失败（可能尚未创建） ${dirPath}:`, e)
      result.push({ label: config.label, dirPath, series: [] })
    }
  }

  groups.value = result
  loading.value = false

  // 加载各分组笔记
  const notesMap: Record<string, Record<string, string>> = {}
  for (const g of result) {
    try {
      notesMap[g.dirPath] = await invoke<Record<string, string>>('get_notes', { dirPath: g.dirPath })
    } catch (e) {
      console.warn(`[MaterialsPage] 读取分组笔记失败 ${g.dirPath}:`, e)
      notesMap[g.dirPath] = {}
    }
    if (g.subGroups) {
      for (const sg of g.subGroups) {
        try {
          notesMap[sg.dirPath] = await invoke<Record<string, string>>('get_notes', { dirPath: sg.dirPath })
        } catch (e) {
          console.warn(`[MaterialsPage] 读取子分组笔记失败 ${sg.dirPath}:`, e)
          notesMap[sg.dirPath] = {}
        }
      }
    }
  }
  groupNotesMap.value = notesMap
}

// ─── 拖入/拖出 ──────────────────────────────────────

const isDragOver = ref(false)
const dropTargetLabel = ref('')
let unlistenDragDrop: (() => void) | null = null

/** 卡片拖出：拖的是系列主文件（最新版本的 PSD），旧版本请从侧边栏版本列表拖 */
function onCardMouseDown(e: MouseEvent, series: MaterialSeries) {
  const primary = series.primary
  createDragHandler(
    () => {
      if (isMultiSelect.value) {
        if (!selectedPaths.value.has(primary.path)) {
          toggleSelection(primary.path)
        }
        const paths = [...selectedPaths.value]
        if (paths.length > 0) {
          startDrag({ item: paths, icon: '' }).catch(err => console.error('拖拽失败:', err))
        }
      } else {
        startDrag({ item: [primary.path], icon: '' }).catch(err => console.error('拖拽失败:', err))
      }
    },
    (ev) => ev.button !== 0 || primary.is_dir,
  )(e)
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
        @click="openPageNote()"
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
          <TransitionGroup v-if="group.series.length > 0" name="card" tag="div" class="card-grid">
            <NormalCard
              v-for="(s, i) in group.series"
              :key="s.key"
              :style="{ '--delay': i * 40 + 'ms' }"
              :file="s.cover"
              :selection-path="s.primary.path"
              :display-name="s.fileCount > 1 ? s.label : undefined"
              :sub-label="s.fileCount > 1 ? $t('materialsPage.latestDate', { date: s.versions[0].date }) : undefined"
              :version-count="s.versions.length"
              :format-label="seriesFormatLabel(s)"
              :multi-select="isMultiSelect"
              :checked="selectedPaths.has(s.primary.path)"
              :has-note="seriesHasNote(group.dirPath, s)"
              :note-preview="seriesNotePreview(group.dirPath, s)"
              :class="{ selected: !isMultiSelect && selectedSeries?.key === s.key, 'multi-checked': isMultiSelect && selectedPaths.has(s.primary.path) }"
              @click="onCardClick(s)"
              @mousedown="onCardMouseDown($event, s)"
            />
          </TransitionGroup>

          <!-- 空分组提示（新项目或目录为空时） -->
          <p
            v-if="group.series.length === 0 && !group.subGroups?.length"
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
                  v-for="(s, i) in sub.series"
                  :key="s.key"
                  :style="{ '--delay': i * 40 + 'ms' }"
                  :file="s.cover"
                  :selection-path="s.primary.path"
                  :display-name="s.fileCount > 1 ? s.label : undefined"
                  :sub-label="s.fileCount > 1 ? $t('materialsPage.latestDate', { date: s.versions[0].date }) : undefined"
                  :version-count="s.versions.length"
                  :format-label="seriesFormatLabel(s)"
                  :multi-select="isMultiSelect"
                  :checked="selectedPaths.has(s.primary.path)"
                  :has-note="seriesHasNote(sub.dirPath, s)"
                  :note-preview="seriesNotePreview(sub.dirPath, s)"
                  :class="{ selected: !isMultiSelect && selectedSeries?.key === s.key, 'multi-checked': isMultiSelect && selectedPaths.has(s.primary.path) }"
                  @click="onCardClick(s)"
                  @mousedown="onCardMouseDown($event, s)"
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
      :versions="sidebarVersions"
      :version-label-of="versionLabel"
      :note="selectedFile ? getFileNote(selectedFile) : undefined"
      @close="clearSelection"
      @update:width-percent="sidebarWidth = $event"
      @select-version="selectedFile = $event"
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
    @cancel="closePageNote"
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
/* .view-buttons, .view-btn → design-system.css 公共类 */

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
