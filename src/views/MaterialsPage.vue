<script setup lang="ts">
import { nextTick, ref, computed, onMounted, onUnmounted } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { invoke } from '@tauri-apps/api/core'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { startDrag } from '@crabnebula/tauri-plugin-drag'
import { useNavigation } from '../composables/useNavigation'
import { useProjects } from '../composables/useProjects'
import { useDirectoryFiles, type FileEntry } from '../composables/useDirectoryFiles'
import NormalCard from '../components/NormalCard.vue'
import FileDetailSidebar from '../components/FileDetailSidebar.vue'
import { useRubberBandSelect } from '../composables/useRubberBandSelect'

const route = useRoute()
const router = useRouter()
const { setNavigation } = useNavigation()
const { projects, loadProjects } = useProjects()
const { openInExplorer } = useDirectoryFiles()

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
    openInExplorer(file.path)
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

/* 注册项目素材页导航配置 */
setNavigation({
  title: '项目素材',
  showBackButton: true,
  onBack: () => router.push({ name: 'project', params: { projectId } }),
  actions: [],
  moreMenuItems: [
    { id: 'refresh', label: '刷新', handler: refreshAll },
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
    await refreshAll()
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
      <span class="sub-title">素材文件夹</span>
      <div class="view-buttons">
        <button class="view-btn" @click="refreshAll">刷新</button>
        <button
          class="view-btn"
          :class="{ active: isMultiSelect }"
          @click="toggleMultiSelect"
        >
          {{ isMultiSelect ? '多选 ✓' : '多选' }}
        </button>
        <button
          v-if="isMultiSelect"
          class="view-btn"
          @click="toggleSelectAll"
        >
          {{ isAllSelected ? '取消全选' : '全选' }}
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
      <p v-if="loading" class="loading-text">扫描中...</p>

      <p v-else-if="groups.length === 0" class="empty-text">暂无素材</p>

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
              title="打开文件夹"
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
              :class="{ selected: !isMultiSelect && selectedFile?.path === file.path, 'multi-checked': isMultiSelect && selectedPaths.has(file.path) }"
              @click="onCardClick(file)"
              @mousedown="onCardMouseDown($event, file)"
            />
          </TransitionGroup>

          <!-- 空分组提示（新项目或目录为空时） -->
          <p
            v-if="group.files.length === 0 && !group.subGroups?.length"
            class="drop-hint"
          >将文件拖入此处</p>

          <!-- 子分组（flatten 展开） -->
          <div v-if="group.subGroups" class="sub-groups">
            <div v-for="sub in group.subGroups" :key="sub.label" class="sub-group">
              <div class="sub-group-header">
                <span class="sub-group-label">{{ sub.label }}</span>
                <button
                  class="folder-btn"
                  title="打开文件夹"
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
      @close="selectedFile = null"
      @update:width-percent="sidebarWidth = $event"
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
  padding-top: var(--spacing-4);
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

/* .group-label → design-system.css 公共类 */

.folder-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 28px;
  height: 28px;
  border: none;
  border-radius: var(--radius-md);
  background: transparent;
  color: var(--text-tertiary);
  cursor: pointer;
  transition: var(--transition-bg);
}

.folder-btn:hover {
  background: var(--bg-hover);
  color: var(--text-primary);
}

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
