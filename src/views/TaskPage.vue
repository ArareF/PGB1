<script setup lang="ts">
import { ref, computed, watch, onMounted, onUnmounted } from 'vue'
import { useI18n } from 'vue-i18n'
import { useRoute, useRouter } from 'vue-router'
import { convertFileSrc, invoke } from '@tauri-apps/api/core'
import { startDrag } from '@crabnebula/tauri-plugin-drag'
import { useNavigation } from '../composables/useNavigation'
import { useProjects } from '../composables/useProjects'
import { useDirectoryFiles } from '../composables/useDirectoryFiles'
import { useMaterials } from '../composables/useMaterials'
import { useSettings } from '../composables/useSettings'
import type { MaterialInfo } from '../composables/useMaterials'
import { useNotes, toggleCheckbox, usePageNote } from '../composables/useNotes'
import { useMultiSelect } from '../composables/useMultiSelect'
import { createDragHandler } from '../composables/useDragIntent'
import { usePreviewVideos, type PreviewVideoGroup } from '../composables/usePreviewVideos'
import { useMaterialSidebar } from '../composables/useMaterialSidebar'
import type { GlobalTaskConfig } from '../types/task'
import type { MaterialVersion } from '../types/material'
import MaterialCard from '../components/MaterialCard.vue'
import SequencePreview from '../components/SequencePreview.vue'
import ImageViewer from '../components/ImageViewer.vue'
import FileDetailSidebar from '../components/FileDetailSidebar.vue'
import UploadConfirmDialog from '../components/UploadConfirmDialog.vue'
import NormalizationDialog from '../components/NormalizationDialog.vue'
import SubtaskDialog from '../components/SubtaskDialog.vue'
import NoteEditor from '../components/NoteEditor.vue'
import NoteDialog from '../components/NoteDialog.vue'
import NoteRenderer from '../components/NoteRenderer.vue'
import PageGuideOverlay from '../components/PageGuideOverlay.vue'
import SidebarShell from '../components/SidebarShell.vue'
import { PAGE_GUIDE_ANNOTATIONS } from '../config/onboarding'
import { formatSize } from '../utils/format'

const route = useRoute()
const router = useRouter()
const { t } = useI18n()
const { setNavigation } = useNavigation()
const { projects, loadProjects } = useProjects()
const { openInExplorer } = useDirectoryFiles()
const { materials, loading, loadMaterials } = useMaterials()
const { loadSettings, settings } = useSettings()

const projectId = route.params.projectId as string
const taskId = route.params.taskId as string
const pageNoteKey = 'card:' + taskId.toLowerCase()

// ─── 路径：ref 作为 SSOT，script / template / composable 全部走这里 ──
const taskFolderPathRef = ref('')
const nextcloudPathRef = ref('')
const nextcloudPreviewPathRef = ref('')

const { loadNotes, hasNote, saveNote: saveTaskNote, getNote } = useNotes(taskFolderPathRef)
const projectPathRef = ref('')
const { loadNotes: loadProjectNotes, hasNote: hasProjectNote, getNote: getProjectNote, saveNote: saveProjectNote } = useNotes(projectPathRef)

/** 当前视图模式 */
const viewMode = ref<'tree' | 'name'>('tree')

/** 滚动容器 ref */
const scrollRef = ref<HTMLElement | null>(null)

// ─── 预览视频 composable（state + 纯函数 + 单文件上传，不依赖多选） ──
const {
  previewGroups,
  videoThumbnails,
  selectedPreviewVideo,
  selectedPreviewGroup,
  showPreviewUploadConfirm,
  draggedPreviewFile,
  selectedPreviewVideoAsFileEntry,
  selectedPreviewGroupVersionsAsFileEntries,
  previewGroupKey,
  loadPreviewGroups,
  clearSelection: clearPreviewSelection,
  confirmPreviewUpload,
  cancelPreviewUpload,
} = usePreviewVideos({
  taskFolderPathRef,
  nextcloudPreviewPathRef,
  onAfterUpload: () => checkSubtaskAutoPrompt(),
})

// ─── 素材侧边栏 composable（选中 / 版本 / 重命名 / 删除 / 帧率 / TPS） ──
const {
  selectedMaterial,
  versions,
  sidebarDialog,
  renameInput,
  sidebarNoteText,
  editingFps,
  fpsInput,
  selectMaterial,
  closeSidebar,
  onSidebarNoteSave,
  openRenameDialog,
  openDeleteDialog,
  closeSidebarDialog,
  confirmRename,
  confirmDelete,
  startEditFps,
  cancelEditFps,
  confirmEditFps,
  openTpsFile,
} = useMaterialSidebar({
  taskFolderPathRef,
  scrollRef,
  materials,
  getNote,
  saveNote: saveTaskNote,
  refresh: () => refresh(),
  onPreviewSelectionCleared: () => clearPreviewSelection(),
})

// ─── 多选（allPaths 合并 materials + previewGroups；onEnter 互斥关闭两个侧边栏） ──
const {
  isMultiSelect, selectedPaths, isAllSelected,
  toggleMultiSelect: _toggleMultiSelect, toggleSelection, toggleSelectAll,
  isSelecting, selectionRect, justFinished, onContainerMouseDown, onContainerScroll,
} = useMultiSelect({
  allPaths: computed(() => [
    ...materials.value.map(m => m.path),
    ...previewGroups.value.map(previewGroupKey),
  ]),
  onEnter: () => {
    if (selectedMaterial.value) closeSidebar()
    clearPreviewSelection()
  },
  rubberBand: {
    containerRef: scrollRef,
    cardSelector: '.material-card[data-path], .preview-video-card[data-path]',
  },
})

function toggleMultiSelect() { _toggleMultiSelect() }

const selectedMaterials = computed(() =>
  materials.value.filter(m => selectedPaths.value.has(m.path))
)

const selectedPreviewGroups = computed(() =>
  previewGroups.value.filter(g => selectedPaths.value.has(previewGroupKey(g)))
)

/** 卡片点击处理：多选模式下切换选中，否则打开侧边栏 */
function onCardClick(material: MaterialInfo) {
  if (isMultiSelect.value) {
    toggleSelection(material.path)
  } else {
    selectMaterial(material)
  }
}

// ─── 预览视频选中 / 拖拽（依赖多选，作为 wrapper 留在父组件避免循环依赖） ──
function selectPreviewVideo(group: PreviewVideoGroup) {
  // 多选模式：点击切换勾选态，不打开侧边栏
  if (isMultiSelect.value) {
    toggleSelection(previewGroupKey(group))
    return
  }
  // 再次点击同一组则关闭
  if (selectedPreviewGroup.value?.baseName === group.baseName) {
    clearPreviewSelection()
    return
  }
  // 互斥：关闭素材侧边栏
  if (selectedMaterial.value) {
    closeSidebar()
  }
  selectedPreviewVideo.value = group.versions[group.versions.length - 1]
  selectedPreviewGroup.value = group
}

function onPreviewVideoMouseDown(e: MouseEvent, group: PreviewVideoGroup) {
  createDragHandler(
    () => {
      // 多选模式：若当前组未选中先勾上，再走统一的混合批量拖拽
      if (isMultiSelect.value) {
        const key = previewGroupKey(group)
        if (!selectedPaths.value.has(key)) {
          toggleSelection(key)
        }
        if (selectedPaths.value.size > 0) {
          performDrag(selectedMaterials.value, selectedPreviewGroups.value)
        }
        return
      }
      // 单选模式：保留原"单文件拖拽 + 独立预览视频上传弹窗"
      const latest = group.versions[group.versions.length - 1]
      startDrag({ item: [latest.path], icon: '' }, (payload) => {
        if (payload.result === 'Dropped') {
          draggedPreviewFile.value = latest
          showPreviewUploadConfirm.value = true
        }
      }).catch(err => {
        console.error('预览视频拖拽失败:', err)
      })
    },
    (ev) => ev.button !== 0,
  )(e)
}

// ─── 混合拖拽上传（素材 + 预览视频 混合弹窗） ──
const showUploadConfirm = ref(false)
const draggedMaterials = ref<MaterialInfo[]>([])
const draggedPreviewGroupsForUpload = ref<PreviewVideoGroup[]>([])

function onCardMouseDown(e: MouseEvent, material: MaterialInfo) {
  createDragHandler(
    () => {
      if (isMultiSelect.value) {
        if (!selectedPaths.value.has(material.path)) {
          toggleSelection(material.path)
        }
        if (selectedPaths.value.size > 0) {
          performDrag(selectedMaterials.value, selectedPreviewGroups.value)
        }
      } else {
        performDrag([material], [])
      }
    },
    (ev) => ev.button !== 0,
  )(e)
}

async function performDrag(
  materialsToDrag: MaterialInfo[],
  previewGroupsToDrag: PreviewVideoGroup[],
) {
  try {
    // 素材走 Rust 后端解析具体文件路径；预览视频直接取最新版本的 path
    let filePaths: string[] = []
    if (materialsToDrag.length > 0) {
      filePaths = await invoke<string[]>('collect_drag_files', {
        taskPath: taskFolderPathRef.value,
        materials: materialsToDrag.map(m => ({
          name: m.name,
          material_type: m.material_type,
        })),
      })
    }
    for (const g of previewGroupsToDrag) {
      filePaths.push(previewGroupKey(g))
    }

    if (filePaths.length === 0) {
      console.warn('没有可拖拽的文件')
      return
    }

    // 用第一个素材的预览图作为拖拽图标（预览视频没有独立预览图，留空即可）
    const iconPath = materialsToDrag[0]?.preview_path ?? ''

    await startDrag(
      { item: filePaths, icon: iconPath },
      (payload) => {
        if (payload.result === 'Dropped') {
          // 素材「已输出」 或 预览视频「非已上传」 才询问上传
          const hasMaterialDone = materialsToDrag.some(m => m.progress === 'done')
          const previewsToUpload = previewGroupsToDrag.filter(g => g.uploadStatus !== 'uploaded')
          if (hasMaterialDone || previewsToUpload.length > 0) {
            draggedMaterials.value = materialsToDrag
            draggedPreviewGroupsForUpload.value = previewsToUpload
            showUploadConfirm.value = true
          }
        }
      },
    )
  } catch (err) {
    console.error('拖拽失败:', err)
  }
}

/** 版本卡片拖拽：直接拖出文件，不弹窗 */
function onVersionMouseDown(e: MouseEvent, version: MaterialVersion) {
  createDragHandler(
    () => {
      startDrag({ item: [version.file_path], icon: '' }).catch(err => {
        console.error('版本拖拽失败:', err)
      })
    },
    (ev) => ev.button !== 0,
  )(e)
}

/** 确认上传：素材走 copy_to_nextcloud；预览视频逐个走 copy_preview_to_nextcloud */
async function confirmUpload() {
  showUploadConfirm.value = false

  const materialsToUpload = draggedMaterials.value
  const previewGroupsToUpload = draggedPreviewGroupsForUpload.value

  // 素材批量上传
  if (materialsToUpload.length > 0) {
    try {
      const result = await invoke<{ copied_count: number; errors: string[] }>('copy_to_nextcloud', {
        taskPath: taskFolderPathRef.value,
        materialNames: materialsToUpload.map(m => ({
          name: m.name,
          material_type: m.material_type,
        })),
      })
      if (result.errors.length > 0) {
        console.warn('部分素材复制失败:', result.errors)
      }
    } catch (err) {
      console.error('复制素材到 nextcloud 失败:', err)
    }
  }

  // 预览视频逐个上传（后端接口单文件语义）
  for (const g of previewGroupsToUpload) {
    const latestPath = previewGroupKey(g)
    try {
      await invoke('copy_preview_to_nextcloud', {
        filePath: latestPath,
        nextcloudPreviewPath: nextcloudPreviewPathRef.value,
      })
    } catch (err) {
      console.error(`预览视频复制失败 (${latestPath}):`, err)
    }
  }

  // 刷新素材 + 预览视频列表
  await refresh()
  if (previewGroupsToUpload.length > 0) {
    await loadPreviewGroups()
  }

  // 多选模式下清理
  if (isMultiSelect.value) {
    isMultiSelect.value = false
    selectedPaths.value = new Set()
  }

  draggedMaterials.value = []
  draggedPreviewGroupsForUpload.value = []
}

function startScaling() {
  router.push({
    name: 'scale',
    params: { projectId, taskId },
    query: { taskPath: taskFolderPathRef.value },
  })
}

function cancelUpload() {
  showUploadConfirm.value = false
  draggedMaterials.value = []
  draggedPreviewGroupsForUpload.value = []
}

/** 规范化弹窗状态 */
const showNormalizeDialog = ref(false)
const showGuide = ref(false)

/** ─── 导航按钮高亮判定 ──────────────────────────────
 * 规范化：需要异步扫 00_original 与命名规则比对，结果写入 ref
 * 缩放 / 转换：可从 materials.value 同步 computed */
const hasNormalizeWork = ref(false)
const hasScaleWork = computed(() =>
  materials.value.some(m =>
    m.material_type === 'image' &&
    m.progress !== 'uploaded' &&
    m.scales.length === 0
  )
)
const hasConvertWork = computed(() =>
  materials.value.some(m =>
    (m.material_type === 'image' || m.material_type === 'sequence') &&
    m.progress !== 'done' &&
    m.progress !== 'uploaded'
  )
)

/** 扫描 00_original 判断是否存在需要规范化的文件 */
async function checkNormalizeWork() {
  if (!taskFolderPathRef.value) { hasNormalizeWork.value = false; return }
  try {
    const items = await invoke<unknown[]>('preview_normalize', { taskPath: taskFolderPathRef.value })
    hasNormalizeWork.value = items.length > 0
  } catch (e) {
    console.error('检测规范化状态失败:', e)
    hasNormalizeWork.value = false
  }
}

// ─── 笔记 ──────────────────────────────────────────────
const { showPageNote, pageNoteText, openPageNote, closePageNote, onPageNoteSave, onPageNoteUpdate, onPageNoteCheckbox } =
  usePageNote(getProjectNote, saveProjectNote, computed(() => 'card:' + taskId.toLowerCase()))

async function openPinboard() {
  if (!projectPathRef.value) return
  await invoke('open_pinboard_window', {
    dirPath: projectPathRef.value,
    canvasKey: `task:${taskId}`,
    title: taskId,
  })
}

// ─── 子任务状态 ────────────────────────────────────────
const subtaskCompleted = ref(0)   // 分子：用户手动勾选完成的
const subtaskTotal = ref(0)       // 分母：Tab 1 勾选启用的子任务数
const enabledSubtasks = ref<string[]>([])   // 启用的子任务 key 列表
const completedSubtasks = ref<Set<string>>(new Set())  // 已完成的子任务 key 集合
const showSubtaskDialog = ref(false)
const subtaskAutoPrompt = ref(false)  // 是否由自动检测触发（文件全传 → 提醒勾子任务）
const subtaskRevertPrompt = ref(false)  // 反向检测触发（子任务全勾但文件未全传 → 提醒取消勾选）
const subtaskSnapshot = ref<Set<string>>(new Set())  // 弹窗打开时的勾选快照

/** 对比当前勾选状态与弹窗打开时的快照，有变动才允许确认 */
const hasSubtaskChanges = computed(() => {
  const snap = subtaskSnapshot.value
  const curr = completedSubtasks.value
  if (snap.size !== curr.size) return true
  for (const k of snap) {
    if (!curr.has(k)) return true
  }
  return false
})

/** 关闭子任务弹窗并清理自动触发标记 */
function closeSubtaskDialog() {
  showSubtaskDialog.value = false
  subtaskAutoPrompt.value = false
  subtaskRevertPrompt.value = false
}

let currentProjectPath = ''

/** 更新导航栏（含子任务数量） */
function updateNavigation() {
  const hasSubtasks = subtaskTotal.value > 0

  // 工作流优先级：规范化 > 缩放 > 转换
  // 优先级最高且有活的按钮 → 全亮 (active)
  // 其余有活的按钮 → 弱强调描边 (hint)
  // 无活按钮 → 常态
  const primaryId: 'normalize' | 'scale' | 'convert' | null =
    hasNormalizeWork.value ? 'normalize'
    : hasScaleWork.value ? 'scale'
    : hasConvertWork.value ? 'convert'
    : null
  const levelFor = (id: 'normalize' | 'scale' | 'convert', hasWork: boolean) => ({
    active: primaryId === id,
    hint: primaryId !== id && hasWork,
  })

  setNavigation({
    title: taskId,
    showBackButton: true,
    onBack: () => router.push({ name: 'project', params: { projectId } }),
    actions: [
      {
        id: 'subtasks',
        label: `${t('task.subtasks')} ${subtaskCompleted.value}/${subtaskTotal.value}`,
        handler: () => { subtaskAutoPrompt.value = false; subtaskRevertPrompt.value = false; showSubtaskDialog.value = true },
        disabled: !hasSubtasks,
      },
      { id: 'normalize', label: t('task.normalize'), handler: () => { showNormalizeDialog.value = true }, ...levelFor('normalize', hasNormalizeWork.value) },
      { id: 'scale', label: t('task.scale'), handler: startScaling, ...levelFor('scale', hasScaleWork.value) },
      { id: 'convert', label: t('task.convert'), handler: () => router.push({ name: 'convert', params: { projectId, taskId }, query: { taskPath: taskFolderPathRef.value } }), ...levelFor('convert', hasConvertWork.value) },
    ],
    moreMenuItems: [
      { id: 'open-nextcloud', label: t('task.openNextcloudFolder'), handler: () => { if (nextcloudPathRef.value) openInExplorer(nextcloudPathRef.value) } },
      { id: 'page-guide', label: t('common.pageGuide'), handler: () => { showGuide.value = true } },
    ],
  })
}

/** 切换子任务完成状态 */
async function toggleSubtaskCompletion(subtaskKey: string) {
  try {
    const updated = await invoke<string[]>('toggle_subtask_completion', {
      projectPath: currentProjectPath,
      subtaskKey,
    })
    completedSubtasks.value = new Set(updated)
    // 只统计当前任务的已启用子任务中已完成的
    subtaskCompleted.value = enabledSubtasks.value.filter(k => completedSubtasks.value.has(k)).length
    updateNavigation()
    // 子任务状态变化后检测弹窗（全部完成 + 文件未全传 → 反向提醒）
    checkSubtaskAutoPrompt()
  } catch (e) {
    console.error('切换子任务完成状态失败:', e)
  }
}

// 初始注册导航
updateNavigation()

// 三个按钮的"有活可干"信号变化时自动刷新导航，保证按钮高亮态实时对齐
watch([hasNormalizeWork, hasScaleWork, hasConvertWork], () => {
  updateNavigation()
})

/** 判断是否为 Prototype 任务 */
const isPrototype = computed(() => taskId.toLowerCase() === 'prototype')

/** 树形视图分组数据 */
const groupedMaterials = computed(() => {
  if (isPrototype.value) {
    // Prototype：先按子分类分组，再按缩放比例子分组
    const subcatMap = new Map<string, MaterialInfo[]>()
    for (const m of materials.value) {
      const slashIndex = m.name.indexOf('/')
      const subCategory = slashIndex > 0 ? m.name.substring(0, slashIndex) : t('task.others')
      if (!subcatMap.has(subCategory)) subcatMap.set(subCategory, [])
      subcatMap.get(subCategory)!.push(m)
    }
    const groups: { label: string; items: MaterialInfo[] }[] = []
    Array.from(subcatMap.entries())
      .sort(([a], [b]) => a.localeCompare(b))
      .forEach(([subcat, items]) => {
        groups.push({ label: subcat, items: [] }) // 子分类区块标题（section-label）
        const scaleMap = new Map<number | 'original', MaterialInfo[]>()
        for (const m of items) {
          if (m.scales.length === 0) {
            if (!scaleMap.has('original')) scaleMap.set('original', [])
            scaleMap.get('original')!.push(m)
          } else {
            for (const s of m.scales) {
              if (!scaleMap.has(s)) scaleMap.set(s, [])
              scaleMap.get(s)!.push(m)
            }
          }
        }
        if (scaleMap.has('original')) {
          groups.push({ label: t('task.original'), items: scaleMap.get('original')! })
        }
        const numericScales = Array.from(scaleMap.keys())
          .filter((k): k is number => k !== 'original')
          .sort((a, b) => b - a)
        for (const s of numericScales) {
          groups.push({ label: `[${s}]`, items: scaleMap.get(s)! })
        }
      })
    return groups
  }

  // 普通任务：静帧和序列帧各自按缩放比例分组，视频/其他按类型分组
  const groups: { label: string; items: MaterialInfo[]; isSubHeader?: boolean }[] = []

  function buildScaleGroups(
    items: MaterialInfo[],
    sectionLabel: string,
  ) {
    if (!items.length) return
    groups.push({ label: sectionLabel, items: [] }) // 分区标题行（空 items）
    const scaleMap = new Map<number | 'original', MaterialInfo[]>()
    for (const m of items) {
      if (m.scales.length === 0) {
        if (!scaleMap.has('original')) scaleMap.set('original', [])
        scaleMap.get('original')!.push(m)
      } else {
        for (const s of m.scales) {
          if (!scaleMap.has(s)) scaleMap.set(s, [])
          scaleMap.get(s)!.push(m)
        }
      }
    }
    // 原始组优先，然后按比例从大到小
    if (scaleMap.has('original')) {
      groups.push({ label: t('task.original'), items: scaleMap.get('original')!, isSubHeader: true })
    }
    const numericScales = Array.from(scaleMap.keys())
      .filter((k): k is number => k !== 'original')
      .sort((a, b) => b - a)
    for (const s of numericScales) {
      groups.push({ label: `[${s}]`, items: scaleMap.get(s)!, isSubHeader: true })
    }
  }

  buildScaleGroups(materials.value.filter(m => m.material_type === 'image'), t('task.images'))
  buildScaleGroups(materials.value.filter(m => m.material_type === 'sequence'), t('task.sequences'))

  const videos = materials.value.filter(m => m.material_type === 'video')
  const others = materials.value.filter(m => m.material_type === 'other')
  if (videos.length) groups.push({ label: t('task.videos'), items: videos })
  if (others.length) groups.push({ label: t('task.others'), items: others })
  return groups
})

/** 侧边栏共享宽度（两个侧边栏用同一宽度变量，避免跳变） */
const fileDetailWidthPercent = ref(30)

/** 点击主内容区空白处关闭侧边栏 */
function onMainContentClick(e: MouseEvent) {
  if (justFinished.value) return
  const target = e.target as HTMLElement
  if (target.closest('.material-card')) return
  if (target.closest('.preview-video-card')) return  // 不关闭预览视频
  closeSidebar()
  clearPreviewSelection()
}

/** 素材类型中文映射 */
function typeLabel(type: string): string {
  const map: Record<string, string> = { image: t('task.typeImage'), sequence: t('task.typeSequence'), video: t('task.typeVideo'), other: t('task.typeOther') }
  return map[type] ?? type
}

/** 进度状态中文映射 */
function progressLabel(progress: string): string {
  const map: Record<string, string> = {
    none: t('task.progressNone'), original: t('task.progressOriginal'), scaled: t('task.progressScaled'), done: t('task.progressDone'), uploaded: t('task.progressUploaded'), broken: t('task.progressBroken'),
  }
  return map[progress] ?? progress
}

/** 纯检测：对比当前 materials/previewGroups/completedSubtasks 与 upload_prompted_tasks，按需弹窗 */
function checkSubtaskAutoPrompt() {
  if (enabledSubtasks.value.length === 0 || !currentProjectPath) return

  const project = projects.value.find(p => p.name === projectId)
  if (!project) return

  const allMaterialsUploaded = materials.value.length > 0
    && materials.value.every(m => m.progress === 'uploaded')
  const allVideosUploaded = previewGroups.value.length === 0
    || previewGroups.value.every(g => g.uploadStatus === 'uploaded')
  const allUploaded = allMaterialsUploaded && allVideosUploaded
  const hasIncomplete = enabledSubtasks.value.some(k => !completedSubtasks.value.has(k))
  const allSubtasksDone = !hasIncomplete && enabledSubtasks.value.length > 0
  const alreadyPrompted = project.upload_prompted_tasks.includes(taskId.toLowerCase())

  if (allUploaded && hasIncomplete && !alreadyPrompted) {
    subtaskAutoPrompt.value = true
    subtaskSnapshot.value = new Set(completedSubtasks.value)
    showSubtaskDialog.value = true
    // 同步内存 + 持久化
    const key = taskId.toLowerCase()
    if (!project.upload_prompted_tasks.includes(key)) {
      project.upload_prompted_tasks.push(key)
    }
    invoke('mark_upload_prompted', {
      projectPath: currentProjectPath,
      taskName: taskId,
      prompted: true,
    }).catch(e => console.error('标记上传提醒失败:', e))
  } else if (!allUploaded && allSubtasksDone) {
    subtaskRevertPrompt.value = true
    subtaskSnapshot.value = new Set(completedSubtasks.value)
    showSubtaskDialog.value = true
  }

  if (!allUploaded && alreadyPrompted) {
    // 同步内存 + 持久化
    const key = taskId.toLowerCase()
    const idx = project.upload_prompted_tasks.indexOf(key)
    if (idx !== -1) project.upload_prompted_tasks.splice(idx, 1)
    invoke('mark_upload_prompted', {
      projectPath: currentProjectPath,
      taskName: taskId,
      prompted: false,
    }).catch(e => console.error('清除上传提醒标记失败:', e))
  }
}

async function refresh() {
  if (taskFolderPathRef.value) {
    await loadMaterials(taskFolderPathRef.value)
    // 刷新后清除选中列表（素材列表可能变化）
    selectedPaths.value = new Set()
    // 刷新规范化按钮高亮态（与 materials 解耦，需独立 invoke）
    checkNormalizeWork()
    // 同步刷新预览视频列表
    await loadPreviewGroups()
    // 刷新项目数据（获取最新 prompted 标记 + 子任务完成状态），再检测
    await loadProjects()
    const freshProject = projects.value.find(p => p.name === projectId)
    if (freshProject && enabledSubtasks.value.length > 0) {
      completedSubtasks.value = new Set(freshProject.completed_subtasks)
      subtaskCompleted.value = enabledSubtasks.value.filter(k => completedSubtasks.value.has(k)).length
      updateNavigation()
    }
    checkSubtaskAutoPrompt()
  }
}

onMounted(async () => {
  await loadProjects()
  await loadSettings()
  const project = projects.value.find(p => p.name === projectId)
  if (project) {
    taskFolderPathRef.value = `${project.path}\\03_Render_VFX\\VFX\\Export\\${taskId}`
    nextcloudPathRef.value = `${project.path}\\03_Render_VFX\\VFX\\nextcloud\\${taskId}`
    nextcloudPreviewPathRef.value = `${project.path}\\03_Render_VFX\\VFX\\nextcloud\\preview`
    projectPathRef.value = project.path
    await loadMaterials(taskFolderPathRef.value)
    checkNormalizeWork()
    await loadNotes()
    await loadProjectNotes()

    // 加载 03_preview 视频并分组 + 截帧
    await loadPreviewGroups()

    // 加载子任务数据
    currentProjectPath = project.path
    try {
      const config = await invoke<GlobalTaskConfig>('load_global_tasks', {
        rootDir: settings.value?.general.projectRootDir ?? '',
      })
      const taskLower = taskId.toLowerCase()
      const globalTask = config.tasks.find(t => t.name.toLowerCase() === taskLower)
      if (globalTask && globalTask.children.length > 0) {
        const prefix = `${globalTask.name}/`
        enabledSubtasks.value = project.enabled_tasks.filter(k => k.startsWith(prefix))
        subtaskTotal.value = enabledSubtasks.value.length
        completedSubtasks.value = new Set(project.completed_subtasks)
        subtaskCompleted.value = enabledSubtasks.value.filter(k => completedSubtasks.value.has(k)).length
        updateNavigation()
        checkSubtaskAutoPrompt()
      }
    } catch (e) {
      console.error('加载子任务数据失败:', e)
    }
  }
})

// 窗口重新可见时自动刷新（用户从外部操作回来后及时检测状态变化）
function onVisibilityChange() {
  if (document.visibilityState === 'visible') {
    refresh()
  }
}
document.addEventListener('visibilitychange', onVisibilityChange)
onUnmounted(() => {
  document.removeEventListener('visibilitychange', onVisibilityChange)
})
</script>

<template>
  <div class="task-page">
    <!-- 主内容区 -->
    <div class="main-content" @click="onMainContentClick">
      <!-- 小标题栏 -->
      <div class="sub-title-bar">
        <span class="sub-title">{{ $t('task.materialList') }}</span>
        <button
          class="folder-btn"
          :title="$t('task.openTaskFolder')"
          @click="openInExplorer(taskFolderPathRef)"
        >
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z" />
          </svg>
        </button>
        <div v-if="hasProjectNote(pageNoteKey)" class="note-preview-inline">
          <NoteRenderer :text="getProjectNote(pageNoteKey)!" @toggle-checkbox="onPageNoteCheckbox(pageNoteKey, $event)" />
        </div>
        <button
          class="note-btn"
          :class="{ 'has-note': hasProjectNote(pageNoteKey) }"
          :title="$t('note.pageNote')"
          @click="openPageNote()"
        >
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M12 20h9"/><path d="M16.5 3.5a2.121 2.121 0 0 1 3 3L7 19l-4 1 1-4L16.5 3.5z"/></svg>
        </button>
        <button
          class="note-btn"
          :title="$t('pinboard.title')"
          @click="openPinboard"
        >
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="3" y="3" width="18" height="18" rx="2"/><path d="M9 3v18"/><path d="M3 9h6"/></svg>
        </button>
        <div class="view-buttons">
          <button
            class="view-btn"
            :class="{ active: viewMode === 'tree' }"
            @click="viewMode = 'tree'"
          >
            {{ $t('task.treeView') }}
          </button>
          <button
            class="view-btn"
            :class="{ active: viewMode === 'name' }"
            @click="viewMode = 'name'"
          >
            {{ $t('task.nameView') }}
          </button>
          <button class="view-btn" @click="refresh">
            {{ $t('common.refresh') }}
          </button>
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

      <!-- 加载状态 overlay：absolute 定位到 main-content 顶部，
           refresh / 帧率编辑 / 重命名 / 删除 后的刷新都经由这里显示
           加载反馈，不触发 v-if 卸载 tree-view → scrollTop 保持 -->
      <Transition name="fade">
        <div v-if="loading" class="loading-overlay">
          <p class="loading-text">{{ $t('common.scanning') }}</p>
        </div>
      </Transition>

      <!-- 可滚动内容区 -->
      <div
        ref="scrollRef"
        class="scroll-content"
        @mousedown="onContainerMouseDown"
        @scroll="onContainerScroll"
      >
        <!-- 空状态 -->
        <p v-if="!loading && materials.length === 0" class="empty-text">
          {{ $t('task.noMaterials') }}
        </p>

        <!-- 树形视图 -->
        <div v-else-if="viewMode === 'tree'" class="tree-view">
          <div v-for="group in groupedMaterials" :key="group.label" class="type-group">
            <p :class="group.items.length === 0 ? 'section-label' : 'group-label'">{{ group.label }}</p>
            <div v-if="group.items.length > 0" class="card-grid">
              <MaterialCard
                v-for="m in group.items"
                :key="m.path"
                :material="m"
                :multi-select="isMultiSelect"
                :checked="selectedPaths.has(m.path)"
                :has-note="hasNote('card:' + m.name.toLowerCase())"
                :note-preview="getNote('card:' + m.name.toLowerCase()) ?? ''"
                :class="{
                  selected: !isMultiSelect && selectedMaterial?.path === m.path,
                  'multi-checked': isMultiSelect && selectedPaths.has(m.path),
                }"
                @click="onCardClick"
                @mousedown="(e: MouseEvent) => onCardMouseDown(e, m)"
              />
            </div>
          </div>
        </div>

        <!-- 名称视图 -->
        <div v-else class="name-view">
          <div class="card-grid">
            <MaterialCard
              v-for="m in materials"
              :key="m.path"
              :material="m"
              :multi-select="isMultiSelect"
              :checked="selectedPaths.has(m.path)"
              :has-note="hasNote('card:' + m.name.toLowerCase())"
              :note-preview="getNote('card:' + m.name.toLowerCase()) ?? ''"
              :class="{
                selected: !isMultiSelect && selectedMaterial?.path === m.path,
                'multi-checked': isMultiSelect && selectedPaths.has(m.path),
              }"
              @click="onCardClick"
              @mousedown="(e: MouseEvent) => onCardMouseDown(e, m)"
            />
          </div>
        </div>

        <!-- 03_preview 预览视频区块 -->
        <div v-if="previewGroups.length > 0" class="preview-videos-section">
          <p class="section-label">{{ $t('task.previewVideos') }}</p>
          <div class="preview-videos-grid">
            <button
              v-for="group in previewGroups"
              :key="group.baseName"
              class="preview-video-card glass-subtle"
              :data-path="previewGroupKey(group)"
              :class="{
                selected: !isMultiSelect && selectedPreviewGroup?.baseName === group.baseName,
                'multi-checked': isMultiSelect && selectedPaths.has(previewGroupKey(group)),
              }"
              @click="selectPreviewVideo(group)"
              @mousedown="(e: MouseEvent) => onPreviewVideoMouseDown(e, group)"
            >
              <!-- 多选复选框 -->
              <span
                v-if="isMultiSelect"
                class="pv-card-checkbox"
                :class="{ checked: selectedPaths.has(previewGroupKey(group)) }"
              >
                <svg v-if="selectedPaths.has(previewGroupKey(group))" width="14" height="14" viewBox="0 0 24 24" fill="currentColor">
                  <path d="M9 16.17L4.83 12l-1.42 1.41L9 19 21 7l-1.41-1.41z" />
                </svg>
              </span>
              <div class="pv-card-preview">
                <!-- 截帧缩略图 -->
                <img
                  v-if="videoThumbnails.get(group.versions[group.versions.length - 1].path)"
                  :src="videoThumbnails.get(group.versions[group.versions.length - 1].path)"
                  class="pv-card-thumb"
                />
                <!-- 截帧失败时显示播放图标 -->
                <svg v-else width="28" height="28" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" opacity="0.6">
                  <polygon points="5,3 19,12 5,21" fill="currentColor" stroke="none" />
                </svg>
              </div>
              <span class="pv-card-name">{{ group.baseName }}</span>
              <div class="pv-card-footer">
                <span
                  class="pv-upload-tag"
                  :class="group.uploadStatus"
                >{{ group.uploadStatus === 'uploaded' ? $t('task.uploaded') : group.uploadStatus === 'outdated' ? $t('task.outdated') : $t('task.notUploaded') }}</span>
                <span class="pv-card-count">{{ $t('task.versionCount', { n: group.versions.length }) }}</span>
              </div>
            </button>
          </div>
        </div>
      </div>
    </div>
  </div>

  <!-- 素材详情侧边栏 -->
  <SidebarShell
    :show="!!selectedMaterial"
    :title="$t('task.detail')"
    :has-actions="true"
  >
    <template v-if="selectedMaterial" #default="{ isFullscreen, toggleFullscreen }">
      <!-- 预览区 -->
      <div class="sidebar-preview">
        <SequencePreview
          v-if="selectedMaterial.material_type === 'sequence'"
          :key="`${selectedMaterial.path}-${selectedMaterial.name}`"
          :folder-path="selectedMaterial.path"
          :base-name="selectedMaterial.name"
          :fps="selectedMaterial.fps ?? settings?.preview.defaultFps ?? 24"
          :max-width="400"
          :transparent="settings?.preview.backgroundTransparent ?? false"
        />
        <ImageViewer
          v-else-if="selectedMaterial.preview_path"
          :key="selectedMaterial.preview_path"
          :src="convertFileSrc(selectedMaterial.preview_path)"
          :alt="selectedMaterial.name"
        />
        <div v-else class="sidebar-no-preview">{{ $t('common.noPreview') }}</div>
        <button
          v-if="selectedMaterial.material_type === 'sequence' || selectedMaterial.preview_path"
          class="preview-fullscreen-btn"
          :title="isFullscreen ? $t('common.exitFullscreen') : $t('common.fullscreen')"
          @click="toggleFullscreen"
        >
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
      <!-- 基本信息 -->
      <div class="sidebar-section">
        <p class="section-title">{{ $t('task.basicInfo') }}</p>
        <div class="info-list">
          <div class="info-row">
            <span class="info-label">{{ $t('task.fileName') }}</span>
            <span class="info-value">{{ selectedMaterial.file_name }}</span>
          </div>
          <div class="info-row">
            <span class="info-label">{{ $t('common.type') }}</span>
            <span class="info-value">{{ typeLabel(selectedMaterial.material_type) }}</span>
          </div>
          <div class="info-row">
            <span class="info-label">{{ $t('common.size') }}</span>
            <span class="info-value">{{ formatSize(selectedMaterial.size_bytes) }}</span>
          </div>
          <div v-if="selectedMaterial.material_type === 'sequence'" class="info-row">
            <span class="info-label">{{ $t('task.frameCount') }}</span>
            <span class="info-value">{{ selectedMaterial.frame_count }}</span>
          </div>
          <div v-if="selectedMaterial.material_type === 'sequence'" class="info-row">
            <span class="info-label">{{ $t('task.frameRate') }}</span>
            <template v-if="selectedMaterial.fps != null">
              <span v-if="!editingFps" class="info-value fps-clickable" :title="$t('task.modify')" @click="startEditFps">
                {{ selectedMaterial.fps }} fps
              </span>
              <span v-else class="fps-edit-group">
                <input
                  class="fps-input"
                  type="number"
                  min="1"
                  max="120"
                  v-model="fpsInput"
                  @keydown.enter="confirmEditFps"
                  @keydown.escape="cancelEditFps"
                  @blur="cancelEditFps"
                />
                <span class="fps-unit">fps</span>
              </span>
            </template>
            <span v-else class="info-value">{{ $t('task.notConverted') }}</span>
          </div>
          <div class="info-row">
            <span class="info-label">{{ $t('task.progress') }}</span>
            <span class="info-value">{{ progressLabel(selectedMaterial.progress) }}</span>
          </div>
        </div>
      </div>

      <!-- 笔记编辑区 -->
      <div class="sidebar-section">
        <p class="section-title">{{ $t('note.note') }}</p>
        <NoteEditor
          v-model="sidebarNoteText"
          @save="onSidebarNoteSave"
          @toggle-checkbox="(idx: number) => { sidebarNoteText = toggleCheckbox(sidebarNoteText, idx); onSidebarNoteSave() }"
        />
      </div>

      <!-- 其他版本 -->
      <div v-if="versions.length > 0" class="sidebar-section">
        <p class="section-title">{{ $t('task.otherVersions') }}</p>
        <div class="version-list">
          <div
            v-for="v in versions"
            :key="v.file_path"
            class="version-card"
            :title="v.file_path"
            @click="openInExplorer(v.file_path)"
            @mousedown="onVersionMouseDown($event, v)"
          >
            <div class="version-card-left">
              <span class="version-name">
                <template v-if="v.scale">{{ v.scale }}% - </template>
                {{ v.stage_label }}
              </span>
              <span class="version-meta">{{ formatSize(v.size_bytes) }}</span>
            </div>
            <div class="version-card-right">
              <span class="version-ext">{{ v.extension.toUpperCase() }}</span>
              <button
                class="version-folder-btn"
                :title="$t('common.openContainingFolder')"
                @click.stop="openInExplorer(v.folder_path)"
              >
                <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                  <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z" />
                </svg>
              </button>
            </div>
          </div>
        </div>
      </div>
    </template>

    <template #actions>
      <button
        v-if="selectedMaterial?.material_type === 'sequence' && versions.some(v => v.stage === '02_done')"
        class="sidebar-action-btn"
        @click="openTpsFile"
      >{{ $t('task.modify') }}</button>
      <button class="sidebar-action-btn" @click="openRenameDialog">{{ $t('common.rename') }}</button>
      <button class="sidebar-action-btn danger" @click="openDeleteDialog">{{ $t('common.delete') }}</button>
    </template>

    <template #overlay>
      <div v-if="sidebarDialog !== 'none'" class="sidebar-dialog-overlay">
        <!-- 重命名弹窗 -->
        <div v-if="sidebarDialog === 'rename'" class="sidebar-dialog">
          <p class="sidebar-dialog-title">{{ $t('task.renameTitle') }}</p>
          <input
            v-model="renameInput"
            class="sidebar-dialog-input"
            :placeholder="$t('task.inputNewName')"
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
          <p class="sidebar-dialog-title">{{ $t('task.deleteMaterial') }}</p>
          <p class="sidebar-dialog-desc">{{ $t('task.deleteMaterialDesc', { name: selectedMaterial?.name }) }}</p>
          <div class="sidebar-dialog-actions">
            <button class="sidebar-dialog-btn" @click="closeSidebarDialog">{{ $t('common.cancel') }}</button>
            <button class="sidebar-dialog-btn danger" @click="confirmDelete">{{ $t('task.confirmDelete') }}</button>
          </div>
        </div>
      </div>
    </template>
  </SidebarShell>

  <!-- 预览视频侧边栏（复用 FileDetailSidebar） -->
  <FileDetailSidebar
    :file="selectedPreviewVideoAsFileEntry"
    :versions="selectedPreviewGroupVersionsAsFileEntries"
    v-model:widthPercent="fileDetailWidthPercent"
    @close="selectedPreviewVideo = null; selectedPreviewGroup = null"
    @select-version="(f) => { const v = selectedPreviewGroup?.versions.find(v => v.path === f.path); if (v) selectedPreviewVideo = v }"
  />

  <!-- 上传确认弹窗（素材） -->
  <UploadConfirmDialog
    :show="showUploadConfirm"
    :file-count="draggedMaterials.length"
    @confirm="confirmUpload"
    @cancel="cancelUpload"
  />

  <!-- 上传确认弹窗（预览视频） -->
  <UploadConfirmDialog
    :show="showPreviewUploadConfirm"
    :file-count="1"
    @confirm="confirmPreviewUpload"
    @cancel="cancelPreviewUpload"
  />

  <!-- 规范化预览弹窗 -->
  <NormalizationDialog
    :show="showNormalizeDialog"
    :task-path="taskFolderPathRef"
    @close="showNormalizeDialog = false"
    @success="refresh"
  />

  <!-- 子任务完成弹窗 -->
  <SubtaskDialog
    :show="showSubtaskDialog"
    :enabled-subtasks="enabledSubtasks"
    :completed-subtasks="completedSubtasks"
    :auto-prompt="subtaskAutoPrompt"
    :revert-prompt="subtaskRevertPrompt"
    :has-changes="hasSubtaskChanges"
    @close="closeSubtaskDialog"
    @toggle="toggleSubtaskCompletion"
    @skip="closeSubtaskDialog"
  />

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

  <PageGuideOverlay :show="showGuide" :annotations="PAGE_GUIDE_ANNOTATIONS.task" @close="showGuide = false" />
</template>

<style scoped>
.task-page {
  height: 100%;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.main-content {
  flex: 1;
  display: flex;
  flex-direction: column;
  min-width: 0;
  overflow: hidden;
  position: relative;  /* 承载 .loading-overlay 的 absolute 定位参考 */
}

.scroll-content {
  flex: 1;
  overflow-y: auto;
  padding: var(--spacing-4) var(--spacing-2) var(--spacing-2);
}

/* 加载浮标：不占 scroll-content 空间，不触发 v-if 卸载，
   保证 refresh 期间 scrollTop 不被浏览器重置 */
.loading-overlay {
  position: absolute;
  top: var(--spacing-4);
  left: 50%;
  transform: translateX(-50%);
  padding: var(--spacing-2) var(--spacing-5);
  background: var(--glass-subtle-bg);
  border: 1px solid var(--border-medium);
  border-radius: var(--radius-md);
  box-shadow: var(--shadow-sm);
  z-index: 10;
  pointer-events: none;
}

.fade-enter-active,
.fade-leave-active {
  transition: opacity var(--duration-fast);
}
.fade-enter-from,
.fade-leave-to {
  opacity: 0;
}

/* 小标题栏 */
/* .sub-title-bar, .sub-title → design-system.css 公共类 */
/* .view-buttons, .view-btn → design-system.css 公共类 */

/* 状态文字 */
.loading-text {
  font-size: var(--text-sm);
  color: var(--text-secondary);
  margin: 0;
}

.empty-text {
  font-size: var(--text-lg);
  color: var(--text-tertiary);
  text-align: center;
  padding-top: var(--spacing-8);
}

/* 树形视图 */
.tree-view {
  display: flex;
  flex-direction: column;
  gap: var(--spacing-6);
}

.type-group {
  display: flex;
  flex-direction: column;
  gap: var(--spacing-3);
}

/* .group-label → design-system.css 公共类 */

/* 卡片网格 */
.card-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(var(--card-material-width), 1fr));
  gap: var(--gap-card);
}

</style>

<!-- 素材侧边栏内容层样式（通过 SidebarShell Teleport，需非 scoped） -->
<style>
/* ─── 预览区 ─── */
.sidebar-preview {
  position: relative;
  width: 100%;
  aspect-ratio: 4 / 3;
  max-height: var(--sidebar-preview-max-height);
  border-radius: var(--radius-md);
  overflow: hidden;
  background: var(--bg-hover);
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
}

.sidebar-preview:hover .preview-fullscreen-btn {
  opacity: 1;
}

/* 全屏模式：预览区铺满 */
.sidebar-shell.is-fullscreen .sidebar-preview {
  flex: 1;
  min-height: 0;
  max-height: none;
  aspect-ratio: unset;
  border-radius: 0;
  overflow: hidden;
}

/* 全屏模式下全屏按钮常驻可见 */
.sidebar-shell.is-fullscreen .preview-fullscreen-btn {
  opacity: 1;
}

/* 全屏时隐藏非预览内容 */
.sidebar-shell.is-fullscreen .sidebar-section,
.sidebar-shell.is-fullscreen .sidebar-dialog-overlay {
  display: none;
}

.sidebar-no-preview {
  font-size: var(--text-base);
  color: var(--text-tertiary);
}

/* ─── 帧率内联编辑 ─── */
.fps-edit-group {
  display: flex;
  align-items: baseline;
  gap: var(--spacing-1);
  justify-content: flex-end;
}
.fps-clickable {
  cursor: pointer;
  border-bottom: 1px dashed var(--text-tertiary);
  transition: color var(--duration-fast);
}
.fps-clickable:hover {
  color: var(--color-primary-500);
  border-bottom-color: var(--color-primary-500);
}
.fps-input {
  width: 48px;
  padding: 1px var(--spacing-1);
  background: var(--bg-secondary);
  border: 1px solid var(--border-medium);
  border-radius: var(--radius-sm);
  color: var(--text-primary);
  font-size: var(--text-sm);
  font-family: inherit;
  text-align: right;
}
.fps-input:focus {
  outline: none;
  border-color: var(--border-focus);
}
.fps-input::-webkit-outer-spin-button,
.fps-input::-webkit-inner-spin-button {
  -webkit-appearance: none;
}
.fps-unit {
  font-size: var(--text-sm);
  color: var(--text-secondary);
}

/* ─── 侧边栏内联弹窗 ─── */
/* .sidebar-dialog-overlay / .sidebar-dialog* → design-system.css 公共类 */

/* ─── 03_preview 视频区块 ─── */
.preview-videos-section {
  padding-bottom: var(--spacing-6);
  display: flex;
  flex-direction: column;
  gap: var(--spacing-3);
}

.preview-videos-grid {
  display: flex;
  flex-wrap: wrap;
  gap: var(--gap-card);
}

.preview-video-card {
  position: relative;
  width: var(--card-material-width);
  display: flex;
  flex-direction: column;
  padding: var(--spacing-3);
  border-radius: var(--card-border-radius);
  border: 1px solid transparent;
  cursor: pointer;
  transition: var(--transition-card-hover);
  text-align: left;
  gap: var(--spacing-2);
}

.preview-video-card:hover {
  transform: translateY(var(--card-hover-lift));
  box-shadow: var(--card-shadow-hover);
}

.preview-video-card.selected {
  border-color: var(--color-primary);
  background: var(--bg-selected);
}

.preview-video-card.multi-checked {
  outline: 2px solid var(--color-primary);
  outline-offset: -2px;
}

.pv-card-checkbox {
  position: absolute;
  top: var(--spacing-2);
  left: var(--spacing-2);
  width: 24px;
  height: 24px;
  border-radius: var(--radius-sm);
  border: 2px solid var(--border-heavy);
  background: var(--glass-subtle-bg);
  box-shadow: var(--shadow-sm);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 2;
  transition: all var(--transition-fast);
}

.pv-card-checkbox.checked {
  background: var(--color-primary);
  border-color: var(--color-primary);
  color: var(--color-neutral-0);
}

.pv-card-preview {
  width: 100%;
  aspect-ratio: 1 / 1;
  background: var(--bg-hover);
  border-radius: var(--radius-md);
  display: flex;
  align-items: center;
  justify-content: center;
  color: var(--text-tertiary);
  overflow: hidden;
}

.pv-card-name {
  font-size: var(--text-sm);
  color: var(--text-primary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  width: 100%;
}

.pv-card-footer {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--spacing-2);
  width: 100%;
}

.pv-card-count {
  font-size: var(--text-xs);
  color: var(--text-tertiary);
  flex-shrink: 0;
}

.pv-card-thumb {
  width: 100%;
  height: 100%;
  object-fit: cover;
}

.pv-upload-tag {
  align-self: flex-start;
  display: inline-flex;
  align-items: center;
  height: var(--tag-height);
  padding: 0 var(--tag-padding-x);
  font-size: var(--text-sm);
  font-weight: var(--tag-font-weight);
  border-radius: var(--tag-border-radius);
  color: var(--tag-status-text);
}

.pv-upload-tag.uploaded  { background: var(--tag-progress-uploaded-bg); }
.pv-upload-tag.outdated  { background: var(--tag-progress-outdated-bg); }
.pv-upload-tag.none      { background: var(--tag-progress-none-bg); }
</style>
