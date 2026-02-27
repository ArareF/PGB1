<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted, nextTick } from 'vue'
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
import type { FileEntry } from '../composables/useDirectoryFiles'
import { useRubberBandSelect } from '../composables/useRubberBandSelect'

interface PreviewVideoEntry {
  name: string
  path: string
  extension: string
  size_bytes: number
  upload_status: 'uploaded' | 'outdated' | 'none'
}
import MaterialCard from '../components/MaterialCard.vue'
import SequencePreview from '../components/SequencePreview.vue'
import ImageViewer from '../components/ImageViewer.vue'
import FileDetailSidebar from '../components/FileDetailSidebar.vue'
import UploadConfirmDialog from '../components/UploadConfirmDialog.vue'
import NormalizationDialog from '../components/NormalizationDialog.vue'
import PageGuideOverlay from '../components/PageGuideOverlay.vue'
import { PAGE_GUIDE_ANNOTATIONS } from '../config/onboarding'

interface GlobalTaskChild {
  name: string
}

interface GlobalTask {
  name: string
  children: GlobalTaskChild[]
}

interface GlobalTaskConfig {
  tasks: GlobalTask[]
}

interface MaterialVersion {
  stage: string
  stage_label: string
  scale: string
  file_path: string
  folder_path: string
  extension: string
  size_bytes: number
}

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

let taskFolderPath = ''
let nextcloudPath = ''
let nextcloudPreviewPath = ''

/** 当前视图模式 */
const viewMode = ref<'tree' | 'name'>('tree')

/** 多选模式状态 */
const isMultiSelect = ref(false)
const selectedPaths = ref<Set<string>>(new Set())

function toggleMultiSelect() {
  if (isMultiSelect.value) {
    // 退出多选
    isMultiSelect.value = false
    selectedPaths.value = new Set()
  } else {
    // 进入多选，关闭侧边栏
    isMultiSelect.value = true
    if (selectedMaterial.value) {
      closeSidebar()
    }
  }
}

function toggleMaterialSelection(material: MaterialInfo) {
  const paths = new Set(selectedPaths.value)
  if (paths.has(material.path)) {
    paths.delete(material.path)
  } else {
    paths.add(material.path)
  }
  selectedPaths.value = paths
}

const isAllSelected = computed(() => {
  return materials.value.length > 0 && selectedPaths.value.size === materials.value.length
})

function toggleSelectAll() {
  if (isAllSelected.value) {
    selectedPaths.value = new Set()
  } else {
    selectedPaths.value = new Set(materials.value.map(m => m.path))
  }
}

const selectedMaterials = computed(() => {
  return materials.value.filter(m => selectedPaths.value.has(m.path))
})

// ─── 框选多选 ──────────────────────────────────────
/** 滚动容器 ref */
const scrollRef = ref<HTMLElement | null>(null)

const { isSelecting, selectionRect, justFinished, onContainerMouseDown, onContainerScroll } =
  useRubberBandSelect({
    containerRef: scrollRef,
    cardSelector: '.material-card[data-path]',
    isEnabled: isMultiSelect,
    onSelect: (paths) => {
      selectedPaths.value = paths
    },
  })

/** 卡片点击处理：多选模式下切换选中，否则打开侧边栏 */
function onCardClick(material: MaterialInfo) {
  if (isMultiSelect.value) {
    toggleMaterialSelection(material)
  } else {
    selectMaterial(material)
  }
}

// ─── 拖拽上传 ──────────────────────────────────────

/** 上传确认弹窗状态 */
const showUploadConfirm = ref(false)
const draggedMaterials = ref<MaterialInfo[]>([])

/** 预览视频上传确认弹窗状态 */
const showPreviewUploadConfirm = ref(false)
const draggedPreviewFile = ref<PreviewVideoEntry | null>(null)

/** 规范化弹窗状态 */
const showNormalizeDialog = ref(false)
const showGuide = ref(false)

/** 拖拽意图检测：mousedown 记录起始位置，mousemove 超过阈值后启动拖拽 */
const DRAG_THRESHOLD = 5 // 像素

function onCardMouseDown(e: MouseEvent, material: MaterialInfo) {
  if (e.button !== 0) return

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

      // 确定要拖拽的素材
      if (isMultiSelect.value) {
        if (!selectedPaths.value.has(material.path)) {
          toggleMaterialSelection(material)
        }
        if (selectedPaths.value.size > 0) {
          performDrag(selectedMaterials.value)
        }
      } else {
        performDrag([material])
      }
    }
  }

  function onMouseUp() {
    cleanup()
  }

  function cleanup() {
    document.removeEventListener('mousemove', onMouseMove)
    document.removeEventListener('mouseup', onMouseUp)
  }

  document.addEventListener('mousemove', onMouseMove)
  document.addEventListener('mouseup', onMouseUp)
}

async function performDrag(materialsToDrag: MaterialInfo[]) {
  try {
    // 调用 Rust 后端收集实际文件路径
    const filePaths = await invoke<string[]>('collect_drag_files', {
      taskPath: taskFolderPath,
      materials: materialsToDrag.map(m => ({
        name: m.name,
        material_type: m.material_type,
      })),
    })

    if (filePaths.length === 0) {
      console.warn('没有可拖拽的文件')
      return
    }

    // 用第一个素材的预览图作为拖拽图标
    const iconPath = materialsToDrag[0]?.preview_path ?? ''

    // 发起 OS 级拖拽
    await startDrag(
      { item: filePaths, icon: iconPath },
      (payload) => {
        if (payload.result === 'Dropped') {
          // 只有拖拽的素材包含"已输出"状态时才询问上传
          const hasDone = materialsToDrag.some(m => m.progress === 'done')
          if (hasDone) {
            draggedMaterials.value = materialsToDrag
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
  if (e.button !== 0) return

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
      startDrag({ item: [version.file_path], icon: '' }).catch(err => {
        console.error('版本拖拽失败:', err)
      })
    }
  }

  function onMouseUp() {
    cleanup()
  }

  function cleanup() {
    document.removeEventListener('mousemove', onMouseMove)
    document.removeEventListener('mouseup', onMouseUp)
  }

  document.addEventListener('mousemove', onMouseMove)
  document.addEventListener('mouseup', onMouseUp)
}

/** 确认上传：复制文件到 nextcloud */
async function confirmUpload() {
  showUploadConfirm.value = false

  try {
    const result = await invoke<{ copied_count: number; errors: string[] }>('copy_to_nextcloud', {
      taskPath: taskFolderPath,
      materialNames: draggedMaterials.value.map(m => ({
        name: m.name,
        material_type: m.material_type,
      })),
    })

    // 刷新素材列表（进度会更新为"已上传"）
    await refresh()

    // 如果在多选模式，退出多选
    if (isMultiSelect.value) {
      isMultiSelect.value = false
      selectedPaths.value = new Set()
    }

    if (result.errors.length > 0) {
      console.warn('部分文件复制失败:', result.errors)
    }
  } catch (err) {
    console.error('复制到 nextcloud 失败:', err)
  }

  draggedMaterials.value = []
}

function startScaling() {
  router.push({
    name: 'scale',
    params: { projectId, taskId },
    query: { taskPath: taskFolderPath },
  })
}

function cancelUpload() {
  showUploadConfirm.value = false
  draggedMaterials.value = []
}

/** 当前选中的素材（用于侧边栏） */
const selectedMaterial = ref<MaterialInfo | null>(null)

/** 03_preview 视频分组 */
interface PreviewVideoGroup {
  baseName: string
  versions: PreviewVideoEntry[]   // 按文件名排序，最后一个是最新版
  /** 组的上传状态：取最新版本的状态 */
  uploadStatus: 'uploaded' | 'outdated' | 'none'
}

const previewGroups = ref<PreviewVideoGroup[]>([])

/** 缩略图缓存：文件 path → canvas dataURL */
const videoThumbnails = ref<Map<string, string>>(new Map())

/** 当前选中的预览视频（驱动 FileDetailSidebar） */
const selectedPreviewVideo = ref<PreviewVideoEntry | null>(null)

/** 将 PreviewVideoEntry 适配为 FileEntry（补充 is_dir: false） */
const selectedPreviewVideoAsFileEntry = computed<FileEntry | null>(() => {
  const v = selectedPreviewVideo.value
  if (!v) return null
  return { name: v.name, path: v.path, extension: v.extension, size_bytes: v.size_bytes, is_dir: false }
})

/** 将当前组的 versions 适配为 FileEntry[]（补充 is_dir: false） */
const selectedPreviewGroupVersionsAsFileEntries = computed<FileEntry[] | undefined>(() => {
  return selectedPreviewGroup.value?.versions.map(v => ({
    name: v.name, path: v.path, extension: v.extension, size_bytes: v.size_bytes, is_dir: false,
  }))
})

/** 当前选中的预览视频组（用于传版本列表给侧边栏） */
const selectedPreviewGroup = ref<PreviewVideoGroup | null>(null)

/** 侧边栏共享宽度（两个侧边栏用同一宽度变量，避免跳变） */
const fileDetailWidthPercent = ref(30)

/** 从文件名提取版本号 [major, minor]，无版本返回 [0, 0] */
function extractVersion(name: string): [number, number] {
  const stem = name.replace(/\.[^.]+$/, '')
  const m = stem.match(/_(\d+)(?:\.(\d+))?$/)
  if (!m) return [0, 0]
  return [parseInt(m[1], 10), m[2] ? parseInt(m[2], 10) : 0]
}

/** 将 PreviewVideoEntry[] 分组为 PreviewVideoGroup[] */
function groupPreviewVideos(files: PreviewVideoEntry[]): PreviewVideoGroup[] {
  const map = new Map<string, PreviewVideoEntry[]>()
  for (const f of files) {
    const nameWithoutExt = f.name.replace(/\.[^.]+$/, '')
    const baseName = nameWithoutExt.replace(/_\d+(\.\d+)?$/, '')
    if (!map.has(baseName)) map.set(baseName, [])
    map.get(baseName)!.push(f)
  }
  const groups: PreviewVideoGroup[] = []
  for (const [baseName, versions] of map) {
    versions.sort((a, b) => {
      const va = extractVersion(a.name)
      const vb = extractVersion(b.name)
      return va[0] - vb[0] || va[1] - vb[1]
    })
    const latest = versions[versions.length - 1]
    groups.push({ baseName, versions, uploadStatus: latest.upload_status })
  }
  groups.sort((a, b) => a.baseName.localeCompare(b.baseName))
  return groups
}

/** 对分组数据截帧（取每组 latest），结果写入 videoThumbnails */
function captureGroupThumbnails(groups: PreviewVideoGroup[]) {
  for (const group of groups) {
    const latest = group.versions[group.versions.length - 1]
    if (videoThumbnails.value.has(latest.path)) continue
    const video = document.createElement('video')
    video.crossOrigin = 'anonymous'
    video.preload = 'metadata'
    video.src = convertFileSrc(latest.path)
    video.currentTime = 0.1
    video.addEventListener('seeked', () => {
      const canvas = document.createElement('canvas')
      canvas.width = video.videoWidth || 320
      canvas.height = video.videoHeight || 180
      const ctx = canvas.getContext('2d')
      if (ctx) {
        ctx.drawImage(video, 0, 0, canvas.width, canvas.height)
        const newMap = new Map(videoThumbnails.value)
        newMap.set(latest.path, canvas.toDataURL('image/jpeg', 0.7))
        videoThumbnails.value = newMap
      }
      video.src = ''
    }, { once: true })
    video.addEventListener('error', () => { video.src = '' }, { once: true })
  }
}

/** 选中素材的其他版本列表 */
const versions = ref<MaterialVersion[]>([])

/** 侧边栏内联操作弹窗 */
type SidebarDialog = 'none' | 'rename' | 'delete'
const sidebarDialog = ref<SidebarDialog>('none')
const renameInput = ref('')

function openRenameDialog() {
  renameInput.value = selectedMaterial.value?.name ?? ''
  sidebarDialog.value = 'rename'
  nextTick(() => {
    (document.querySelector('.sidebar-dialog-input') as HTMLInputElement)?.focus()
  })
}

function openDeleteDialog() {
  sidebarDialog.value = 'delete'
}

function closeSidebarDialog() {
  sidebarDialog.value = 'none'
  renameInput.value = ''
}

/** 帧率内联编辑 */
const editingFps = ref(false)
const fpsInput = ref('')

function startEditFps() {
  const mat = selectedMaterial.value
  if (!mat || mat.fps == null) return
  fpsInput.value = String(mat.fps)
  editingFps.value = true
  nextTick(() => {
    (document.querySelector('.fps-input') as HTMLInputElement)?.select()
  })
}

function cancelEditFps() {
  editingFps.value = false
  fpsInput.value = ''
}

async function confirmEditFps() {
  const mat = selectedMaterial.value
  if (!mat) return
  const newFps = parseInt(fpsInput.value, 10)
  if (!newFps || newFps <= 0 || newFps === mat.fps) {
    cancelEditFps()
    return
  }
  try {
    await invoke('rename_sequence_fps', {
      taskPath: taskFolderPath,
      baseName: mat.name,
      oldFps: mat.fps,
      newFps,
    })
    cancelEditFps()
    await refresh()
    // refresh 只更新 materials.value，selectedMaterial 需手动同步到新数据
    const updated = materials.value.find(m => m.name === mat.name)
    if (updated) {
      selectedMaterial.value = updated
      versions.value = await invoke<MaterialVersion[]>('scan_material_versions', {
        taskPath: taskFolderPath,
        baseName: updated.name,
        materialType: updated.material_type,
      })
    }
  } catch (e) {
    console.error('修改帧率失败:', e)
  }
}

/** 打开序列帧工程文件（.tps） */
async function openTpsFile() {
  const mat = selectedMaterial.value
  if (!mat) return
  const doneVersion = versions.value.find(v => v.stage === '02_done')
  if (!doneVersion) return
  const tpsPath = doneVersion.folder_path.replace(/\\/g, '/') + '/' + mat.name + '.tps'
  try {
    await invoke('open_file', { path: tpsPath })
  } catch (e) {
    console.error('打开工程文件失败:', e)
  }
}

async function confirmRename() {
  const mat = selectedMaterial.value
  if (!mat || !renameInput.value.trim() || renameInput.value.trim() === mat.name) {
    closeSidebarDialog()
    return
  }
  try {
    await invoke('rename_material', {
      taskPath: taskFolderPath,
      baseName: mat.name,
      newBaseName: renameInput.value.trim(),
      materialType: mat.material_type,
    })
    closeSidebarDialog()
    closeSidebar()
    await refresh()
  } catch (e) {
    console.error('重命名失败:', e)
  }
}

async function confirmDelete() {
  const mat = selectedMaterial.value
  if (!mat) return
  try {
    await invoke('delete_material', {
      taskPath: taskFolderPath,
      baseName: mat.name,
      materialType: mat.material_type,
    })
    closeSidebarDialog()
    closeSidebar()
    await refresh()
  } catch (e) {
    console.error('删除失败:', e)
  }
}

/** 子任务状态 */
const subtaskCompleted = ref(0)   // 分子：用户手动勾选完成的
const subtaskTotal = ref(0)       // 分母：Tab 1 勾选启用的子任务数
const enabledSubtasks = ref<string[]>([])   // 启用的子任务 key 列表
const completedSubtasks = ref<Set<string>>(new Set())  // 已完成的子任务 key 集合
const showSubtaskDialog = ref(false)
const subtaskAutoPrompt = ref(false)  // 是否由自动检测触发（文件全传 → 提醒勾子任务）
const subtaskRevertPrompt = ref(false)  // 反向检测触发（子任务全勾但文件未全传 → 提醒取消勾选）
const subtaskSnapshot = ref<Set<string>>(new Set())  // 弹窗打开时的勾选快照

/** 自动触发弹窗是否处于活跃状态（禁止点击外部关闭） */
const isAutoTriggered = computed(() => subtaskAutoPrompt.value || subtaskRevertPrompt.value)

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

/** 跳过按钮：长按期间持续抖动，满 1.5s 关闭 */
const dialogShaking = ref(false)
let skipPressTimer: ReturnType<typeof setTimeout> | null = null

function onSkipMouseDown() {
  dialogShaking.value = true
  skipPressTimer = setTimeout(() => {
    skipPressTimer = null
    dialogShaking.value = false
    closeSubtaskDialog()
  }, 1500)
}

function stopSkipPress() {
  if (skipPressTimer) {
    clearTimeout(skipPressTimer)
    skipPressTimer = null
  }
  dialogShaking.value = false
}

let currentProjectPath = ''

/** 更新导航栏（含子任务数量） */
function updateNavigation() {
  const hasSubtasks = subtaskTotal.value > 0
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
      { id: 'normalize', label: t('task.normalize'), handler: () => { showNormalizeDialog.value = true } },
      { id: 'scale', label: t('task.scale'), handler: startScaling },
      { id: 'convert', label: t('task.convert'), handler: () => router.push({ name: 'convert', params: { projectId, taskId }, query: { taskPath: taskFolderPath } }) },
    ],
    moreMenuItems: [
      { id: 'open-nextcloud', label: t('task.openNextcloudFolder'), handler: () => { if (nextcloudPath) openInExplorer(nextcloudPath) } },
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

/** 记录卡片布局变化前的屏幕 Y 坐标，变化后补偿滚动 */
function preserveCardPosition(cardSelector: string, action: () => void) {
  const container = scrollRef.value
  const card = container?.querySelector(cardSelector) as HTMLElement | null
  const beforeY = card?.getBoundingClientRect().top ?? null

  action()

  nextTick(() => {
    requestAnimationFrame(() => {
      if (!container || beforeY === null) return
      const afterCard = container.querySelector(cardSelector) as HTMLElement | null
      if (!afterCard) return
      const afterY = afterCard.getBoundingClientRect().top
      const delta = afterY - beforeY
      container.scrollTop += delta
    })
  })
}

async function selectMaterial(material: MaterialInfo) {
  // 互斥：关闭预览视频侧边栏
  selectedPreviewVideo.value = null
  selectedPreviewGroup.value = null

  // 再次点击同一素材则关闭侧边栏
  if (selectedMaterial.value?.path === material.path) {
    closeSidebar()
    return
  }

  const wasOpen = !!selectedMaterial.value

  preserveCardPosition(
    wasOpen ? '.material-card.selected' : `.material-card[data-path="${CSS.escape(material.path)}"]`,
    () => {
      selectedMaterial.value = material
      versions.value = []
    },
  )

  try {
    versions.value = await invoke<MaterialVersion[]>('scan_material_versions', {
      taskPath: taskFolderPath,
      baseName: material.name,
      materialType: material.material_type,
    })
  } catch (e) {
    console.error('加载版本失败:', e)
  }
}

function closeSidebar() {
  preserveCardPosition('.material-card.selected', () => {
    selectedMaterial.value = null
  })
}

function onPreviewVideoMouseDown(e: MouseEvent, group: PreviewVideoGroup) {
  if (e.button !== 0) return
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
      const latest = group.versions[group.versions.length - 1]
      startDrag({ item: [latest.path], icon: '' }, (payload) => {
        if (payload.result === 'Dropped') {
          draggedPreviewFile.value = latest
          showPreviewUploadConfirm.value = true
        }
      }).catch(err => {
        console.error('预览视频拖拽失败:', err)
      })
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

async function confirmPreviewUpload() {
  showPreviewUploadConfirm.value = false
  const file = draggedPreviewFile.value
  if (!file) return
  try {
    await invoke('copy_preview_to_nextcloud', {
      filePath: file.path,
      nextcloudPreviewPath,
    })
    // 刷新状态（重新扫描 03_preview 以更新 uploadStatus）
    const files = await invoke<PreviewVideoEntry[]>('scan_preview_videos', {
      taskPath: taskFolderPath,
      nextcloudPreviewPath,
    })
    previewGroups.value = groupPreviewVideos(files)
    // 检测是否触发子任务完成弹窗
    checkSubtaskAutoPrompt()
  } catch (err) {
    console.error('复制预览视频失败:', err)
  }
  draggedPreviewFile.value = null
}

function cancelPreviewUpload() {
  showPreviewUploadConfirm.value = false
  draggedPreviewFile.value = null
}

function selectPreviewVideo(group: PreviewVideoGroup) {
  const latest = group.versions[group.versions.length - 1]
  // 再次点击同一组则关闭
  if (selectedPreviewGroup.value?.baseName === group.baseName) {
    selectedPreviewVideo.value = null
    selectedPreviewGroup.value = null
    return
  }
  // 互斥：关闭素材侧边栏
  if (selectedMaterial.value) {
    closeSidebar()
  }
  selectedPreviewVideo.value = latest
  selectedPreviewGroup.value = group
}

/** 点击主内容区空白处关闭侧边栏 */
function onMainContentClick(e: MouseEvent) {
  if (justFinished.value) return
  const target = e.target as HTMLElement
  if (target.closest('.material-card')) return
  if (target.closest('.preview-video-card')) return  // 不关闭预览视频
  closeSidebar()
  selectedPreviewVideo.value = null
  selectedPreviewGroup.value = null
}

/** 侧边栏可拖拽宽度（百分比，范围 20-60） */
const sidebarWidthPercent = ref(30)
const isResizing = ref(false)

function startResize(e: MouseEvent) {
  e.preventDefault()
  isResizing.value = true
  const startX = e.clientX
  const startWidth = sidebarWidthPercent.value

  function onMouseMove(ev: MouseEvent) {
    const windowWidth = window.innerWidth
    const deltaPercent = ((startX - ev.clientX) / windowWidth) * 100
    const newWidth = Math.min(60, Math.max(20, startWidth + deltaPercent))
    sidebarWidthPercent.value = newWidth
  }

  function onMouseUp() {
    isResizing.value = false
    document.removeEventListener('mousemove', onMouseMove)
    document.removeEventListener('mouseup', onMouseUp)
  }

  document.addEventListener('mousemove', onMouseMove)
  document.addEventListener('mouseup', onMouseUp)
}

/** 文件大小格式化 */
function formatSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(1)} GB`
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
  if (taskFolderPath) {
    await loadMaterials(taskFolderPath)
    // 刷新后清除选中列表（素材列表可能变化）
    selectedPaths.value = new Set()
    // 同步刷新预览视频列表
    try {
      const files = await invoke<PreviewVideoEntry[]>('scan_preview_videos', {
        taskPath: taskFolderPath,
        nextcloudPreviewPath,
      })
      previewGroups.value = groupPreviewVideos(files)
      captureGroupThumbnails(previewGroups.value)
    } catch (e) {
      console.error('刷新预览视频失败:', e)
    }
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
    taskFolderPath = `${project.path}\\03_Render_VFX\\VFX\\Export\\${taskId}`
    nextcloudPath = `${project.path}\\03_Render_VFX\\VFX\\nextcloud\\${taskId}`
    nextcloudPreviewPath = `${project.path}\\03_Render_VFX\\VFX\\nextcloud\\preview`
    await loadMaterials(taskFolderPath)

    // 加载 03_preview 视频并分组
    try {
      const files = await invoke<PreviewVideoEntry[]>('scan_preview_videos', {
        taskPath: taskFolderPath,
        nextcloudPreviewPath,
      })
      previewGroups.value = groupPreviewVideos(files)
      captureGroupThumbnails(previewGroups.value)
    } catch (e) {
      console.error('加载预览视频失败:', e)
    }

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
          @click="openInExplorer(taskFolderPath)"
        >
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z" />
          </svg>
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

      <!-- 可滚动内容区 -->
      <div
        ref="scrollRef"
        class="scroll-content"
        @mousedown="onContainerMouseDown"
        @scroll="onContainerScroll"
      >
        <!-- 加载状态 -->
        <p v-if="loading" class="loading-text">{{ $t('common.scanning') }}</p>

        <!-- 空状态 -->
        <p v-else-if="materials.length === 0" class="empty-text">
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
              :class="{ selected: selectedPreviewGroup?.baseName === group.baseName }"
              @click="selectPreviewVideo(group)"
              @mousedown="(e: MouseEvent) => onPreviewVideoMouseDown(e, group)"
            >
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

  <!-- 侧边栏：传送到 content-row，与主功能区同级 -->
  <Teleport to="#content-row">
    <Transition name="sidebar">
    <div
      v-if="selectedMaterial"
      class="detail-sidebar"
      :class="{ 'is-resizing': isResizing }"
      :style="{ width: sidebarWidthPercent + '%' }"
    >
      <!-- 拖拽把手 -->
      <div class="resize-handle" @mousedown="startResize" />
      <div class="sidebar-header">
        <span class="sidebar-title">{{ $t('task.detail') }}</span>
      </div>
      <div class="sidebar-body">
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
      </div>

      <!-- 底部悬浮操作按钮 -->
      <div class="sidebar-actions">
        <button
          v-if="selectedMaterial?.material_type === 'sequence' && versions.some(v => v.stage === '02_done')"
          class="sidebar-action-btn"
          @click="openTpsFile"
        >{{ $t('task.modify') }}</button>
        <button class="sidebar-action-btn" @click="openRenameDialog">{{ $t('common.rename') }}</button>
        <button class="sidebar-action-btn danger" @click="openDeleteDialog">{{ $t('common.delete') }}</button>
      </div>

      <!-- 内联操作弹窗（覆盖整个侧边栏） -->
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
    </div>
    </Transition>
  </Teleport>

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
    :task-path="taskFolderPath"
    @close="showNormalizeDialog = false"
    @success="refresh"
  />

  <!-- 子任务完成弹窗 -->
  <Teleport to="body">
    <div v-if="showSubtaskDialog" class="subtask-overlay">
      <div class="subtask-dialog glass-strong" :class="{ 'dialog-shake': dialogShaking }">
        <p class="subtask-title">{{ $t('task.subtaskProgress') }}</p>
        <p v-if="subtaskAutoPrompt" class="subtask-hint">{{ $t('task.allUploadedHint') }}</p>
        <p v-else-if="subtaskRevertPrompt" class="subtask-hint">{{ $t('task.partialUploadHint') }}</p>
        <div class="subtask-list">
          <label
            v-for="key in enabledSubtasks"
            :key="key"
            class="subtask-row"
            @click.prevent="toggleSubtaskCompletion(key)"
          >
            <span
              class="subtask-checkbox"
              :class="{ checked: completedSubtasks.has(key) }"
            />
            <span class="subtask-name">{{ key.split('/')[1] }}</span>
          </label>
        </div>
        <div class="subtask-actions">
          <span
            v-if="isAutoTriggered"
            class="subtask-skip-btn"
            @mousedown.prevent="onSkipMouseDown"
            @mouseup="stopSkipPress"
            @mouseleave="stopSkipPress"
          >{{ $t('common.skip') }}</span>
          <button
            class="subtask-close-btn"
            :disabled="isAutoTriggered && !hasSubtaskChanges"
            @click="closeSubtaskDialog"
          >
            {{ isAutoTriggered ? $t('common.confirm') : $t('common.close') }}
          </button>
        </div>
      </div>
    </div>
  </Teleport>

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
}

.scroll-content {
  flex: 1;
  overflow-y: auto;
  padding-top: var(--spacing-4);
}

/* 小标题栏 */
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

.view-btn.active {
  background: var(--color-primary);
  color: var(--color-neutral-0);
  border-color: var(--color-primary);
}

/* 状态文字 */
.loading-text,
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

/* ─── 子任务弹窗 ─── */
.subtask-overlay {
  position: fixed;
  inset: 0;
  z-index: var(--z-modal, 1000);
  display: flex;
  align-items: center;
  justify-content: center;
  background: var(--overlay-backdrop);
  backdrop-filter: blur(var(--glass-light-blur));
}

.subtask-dialog {
  min-width: 320px;
  max-width: 320px;
  border-radius: var(--floating-navbar-radius);
  padding: var(--spacing-6);
  display: flex;
  flex-direction: column;
  gap: var(--spacing-5);
}

.subtask-title {
  font-size: var(--text-2xl);
  font-weight: var(--font-weight-heading);
  color: var(--text-primary);
}

.subtask-hint {
  font-size: var(--text-base);
  color: var(--text-secondary);
}

.subtask-list {
  display: flex;
  flex-direction: column;
  gap: var(--spacing-2);
}

.subtask-row {
  display: flex;
  align-items: center;
  gap: var(--spacing-3);
  padding: var(--spacing-2) var(--spacing-3);
  border-radius: var(--radius-md);
  cursor: pointer;
  transition: background var(--transition-fast);
}

.subtask-row:hover {
  background: var(--bg-hover);
}

.subtask-checkbox {
  width: 18px;
  height: 18px;
  min-width: 18px;
  border-radius: var(--radius-sm);
  border: 2px solid var(--border-medium);
  flex-shrink: 0;
  position: relative;
  transition: all var(--transition-fast);
}

.subtask-checkbox.checked {
  background: color-mix(in srgb, var(--color-primary-500) 75%, transparent);
  border-color: color-mix(in srgb, var(--color-primary-500) 75%, transparent);
  backdrop-filter: blur(var(--glass-light-blur));
  -webkit-backdrop-filter: blur(var(--glass-light-blur));
}

.subtask-checkbox.checked::after {
  content: '';
  position: absolute;
  top: 2px;
  left: 5px;
  width: 4px;
  height: 8px;
  border: solid var(--color-neutral-0);
  border-width: 0 2px 2px 0;
  transform: rotate(45deg);
}

.subtask-name {
  font-size: var(--text-base);
  color: var(--text-primary);
}

.subtask-actions {
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.subtask-skip-btn {
  font-size: var(--text-xs);
  color: var(--text-quaternary, var(--text-tertiary));
  opacity: 0.5;
  cursor: pointer;
  user-select: none;
  transition: opacity var(--transition-fast);
}

.subtask-skip-btn:hover {
  opacity: 0.7;
}

.subtask-skip-btn:active {
  opacity: 1;
}

@keyframes dialog-shake {
  0%, 100% { transform: translate(0, 0); }
  15% { transform: translate(-4px, 0); }
  30% { transform: translate(4px, 0); }
  45% { transform: translate(-3px, 0); }
  60% { transform: translate(3px, 0); }
  75% { transform: translate(-2px, 0); }
  90% { transform: translate(2px, 0); }
}

.dialog-shake {
  animation: dialog-shake 0.3s ease infinite;
}

.subtask-close-btn {
  display: inline-flex;
  align-items: center;
  height: var(--button-height);
  padding: 0 var(--spacing-5);
  font-size: var(--text-base);
  font-weight: var(--font-weight-heading);
  border-radius: var(--radius-md);
  border: 1px solid var(--border-medium);
  background: transparent;
  color: var(--text-secondary);
  cursor: pointer;
  transition: all var(--transition-fast);
}

.subtask-close-btn:hover:not(:disabled) {
  background: var(--bg-hover);
  color: var(--text-primary);
}

.subtask-close-btn:disabled {
  opacity: 0.35;
  cursor: not-allowed;
}

</style>

<!-- 侧边栏样式（Teleport 到 #content-row，不在 scoped 范围内） -->
<style>
.detail-sidebar {
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
     在 WebView2 + Windows Acrylic 下 gap 区域产生白色闪烁伪影 */
  background: var(--glass-strong-bg);
  border: var(--glass-strong-border);
  box-shadow: var(--glass-strong-shadow);
}

.detail-sidebar.is-resizing {
  user-select: none;
}

.resize-handle {
  position: absolute;
  top: 0;
  left: 0;
  width: 4px;
  height: 100%;
  cursor: col-resize;
  z-index: 10;
}

.resize-handle:hover,
.resize-handle:active {
  background: var(--color-primary);
  opacity: 0.5;
}

/* 侧边栏进入/离开动画 */
.sidebar-enter-active {
  transition: transform var(--duration-normal) var(--ease-slide-in),
              width var(--duration-normal) var(--ease-slide-in);
  overflow: hidden;
}
.sidebar-leave-active {
  transition: transform var(--duration-fast) var(--ease-slide-out),
              width var(--duration-fast) var(--ease-slide-out);
  overflow: hidden;
}
.sidebar-enter-from,
.sidebar-leave-to {
  transform: translateX(100%);
  width: 0 !important;
}

.sidebar-header {
  display: flex;
  align-items: center;
  padding-bottom: var(--spacing-3);
  border-bottom: 1px solid var(--border-medium);
  flex-shrink: 0;
}

.sidebar-title {
  font-size: var(--text-2xl);
  font-weight: var(--font-weight-heading);
  color: var(--text-primary);
}

.sidebar-body {
  flex: 1;
  overflow-y: auto;
  padding-top: var(--spacing-4);
  padding-bottom: calc(var(--button-md-height) + var(--spacing-4) * 2);
  display: flex;
  flex-direction: column;
  gap: var(--spacing-4);
}

.sidebar-preview {
  width: 100%;
  aspect-ratio: 4 / 3;
  border-radius: var(--radius-md);
  overflow: hidden;
  background: var(--bg-hover);
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
}


.sidebar-no-preview {
  font-size: var(--text-base);
  color: var(--text-tertiary);
}

/* 侧边栏区块（基本信息 / 其他版本） */
.sidebar-section {
  display: flex;
  flex-direction: column;
  gap: var(--spacing-3);
}

.section-title {
  font-size: var(--text-base);
  font-weight: var(--font-weight-heading);
  color: var(--text-secondary);
  padding-bottom: var(--spacing-2);
  border-bottom: 1px solid var(--border-light);
}

.info-list {
  display: flex;
  flex-direction: column;
  gap: var(--spacing-2);
}

.info-row {
  display: flex;
  justify-content: space-between;
  align-items: baseline;
  gap: var(--spacing-2);
}

.info-label {
  font-size: var(--text-base);
  color: var(--text-tertiary);
  flex-shrink: 0;
}

.info-value {
  font-size: var(--text-base);
  color: var(--text-primary);
  text-align: right;
  word-break: break-all;
}

.version-list {
  display: flex;
  flex-direction: column;
  gap: var(--spacing-2);
}

.version-card {
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

.version-card:hover {
  background: var(--bg-hover);
  border-color: var(--border-medium);
}

.version-card-left {
  display: flex;
  flex-direction: column;
  gap: var(--spacing-1);
  min-width: 0;
}

.version-name {
  font-size: var(--text-base);
  font-weight: var(--font-weight-heading);
  color: var(--text-primary);
}

.version-meta {
  font-size: var(--text-sm);
  color: var(--text-tertiary);
}

.version-card-right {
  display: flex;
  align-items: center;
  gap: var(--spacing-2);
  flex-shrink: 0;
}

.version-ext {
  font-size: var(--text-sm);
  font-weight: var(--font-medium);
  color: var(--text-secondary);
}

.version-folder-btn {
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

.version-folder-btn:hover {
  background: var(--color-primary);
  color: var(--color-neutral-0);
  border-color: var(--color-primary);
}

/* ─── 侧边栏底部悬浮操作按钮 ─── */
.sidebar-actions {
  position: absolute;
  bottom: var(--spacing-4);
  left: var(--spacing-4);
  display: flex;
  gap: var(--spacing-3);
  pointer-events: none;
}

.sidebar-action-btn {
  pointer-events: all;
  padding: var(--spacing-2) var(--spacing-4);
  border: var(--glass-medium-border);
  border-radius: var(--radius-button);
  background: var(--glass-medium-bg);
  /* 不用 backdrop-filter，避免与父级 glass 容器 compositor 冲突 */
  color: var(--text-secondary);
  font-size: var(--text-2xl);
  font-family: inherit;
  white-space: nowrap;
  cursor: pointer;
  transition: var(--transition-all);
}

.sidebar-action-btn:hover {
  color: var(--text-primary);
}

.sidebar-action-btn.danger:hover {
  background: color-mix(in srgb, var(--color-danger) 15%, transparent);
  border-color: color-mix(in srgb, var(--color-danger) 50%, transparent);
  color: var(--color-danger);
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
.sidebar-dialog-overlay {
  position: absolute;
  inset: 0;
  z-index: 20;
  display: flex;
  align-items: center;
  justify-content: center;
  background: var(--overlay-backdrop);
  backdrop-filter: blur(var(--glass-light-blur));
  -webkit-backdrop-filter: blur(var(--glass-light-blur));
  border-radius: var(--floating-main-radius);
}

/* 手动复刻 glass-strong 视觉，不用 backdrop-filter：
   在 Teleport 到 #content-row 的侧边栏内，与 main-content 同层 */
.sidebar-dialog {
  width: calc(100% - var(--spacing-8) * 2);
  border-radius: var(--radius-xl);
  padding: var(--spacing-5);
  display: flex;
  flex-direction: column;
  gap: var(--spacing-4);
  background: var(--glass-strong-bg);
  border: var(--glass-strong-border);
  box-shadow: var(--glass-strong-shadow);
}

.sidebar-dialog-title {
  font-size: var(--text-lg);
  font-weight: var(--font-weight-heading);
  color: var(--text-primary);
}

.sidebar-dialog-desc {
  font-size: var(--text-sm);
  color: var(--text-secondary);
  line-height: 1.6;
}

.sidebar-dialog-input {
  width: 100%;
  height: var(--button-height);
  padding: 0 var(--spacing-3);
  border-radius: var(--radius-md);
  border: 1px solid var(--border-medium);
  background: var(--glass-bg-subtle);
  color: var(--text-primary);
  font-size: var(--text-base);
  font-family: inherit;
  outline: none;
  box-sizing: border-box;
}

.sidebar-dialog-input:focus {
  border-color: var(--color-primary);
}

.sidebar-dialog-actions {
  display: flex;
  gap: var(--spacing-3);
  justify-content: flex-end;
}

.sidebar-dialog-btn {
  height: var(--button-md-height);
  padding: 0 var(--spacing-6);
  border-radius: var(--radius-lg);
  border: 1px solid var(--border-medium);
  background: transparent;
  color: var(--text-secondary);
  font-family: inherit;
  font-weight: var(--font-bold);
  cursor: pointer;
  transition: all var(--duration-fast);
}

.sidebar-dialog-btn:hover {
  background: var(--bg-hover);
  color: var(--text-primary);
}

.sidebar-dialog-btn.primary {
  background: color-mix(in srgb, var(--color-primary-500) 15%, transparent);
  border-color: color-mix(in srgb, var(--color-primary-500) 40%, transparent);
  color: var(--color-primary-500);
}

.sidebar-dialog-btn.primary:hover {
  background: color-mix(in srgb, var(--color-primary-500) 25%, transparent);
}

.sidebar-dialog-btn.danger {
  background: color-mix(in srgb, var(--color-danger) 15%, transparent);
  border-color: color-mix(in srgb, var(--color-danger) 40%, transparent);
  color: var(--color-danger);
}

.sidebar-dialog-btn.danger:hover {
  background: color-mix(in srgb, var(--color-danger) 25%, transparent);
}

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
  background: rgba(59, 130, 246, 0.1);
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
