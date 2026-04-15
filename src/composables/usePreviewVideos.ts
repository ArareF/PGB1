import { ref, computed, type Ref } from 'vue'
import { convertFileSrc, invoke } from '@tauri-apps/api/core'
import type { PreviewVideoEntry } from '../types/material'
import type { FileEntry } from './useDirectoryFiles'

export interface PreviewVideoGroup {
  baseName: string
  /** 按文件名排序，最后一个是最新版 */
  versions: PreviewVideoEntry[]
  /** 组的上传状态：取最新版本的状态 */
  uploadStatus: 'uploaded' | 'outdated' | 'none'
}

export interface UsePreviewVideosOptions {
  /** 当前任务目录（Export/<taskId>） */
  taskFolderPathRef: Ref<string>
  /** nextcloud/preview 目标目录 */
  nextcloudPreviewPathRef: Ref<string>
  /** 单文件预览视频上传成功后的回调（通常是父组件的 checkSubtaskAutoPrompt） */
  onAfterUpload?: () => void
}

/**
 * 预览视频（03_preview）的分组、截帧、选中、单文件上传逻辑。
 *
 * 不处理：多选交互（selectPreviewVideo / onPreviewVideoMouseDown 的多选分支）、
 * 混合拖拽上传（draggedPreviewGroupsForUpload）——这些涉及跨 composable 的
 * useMultiSelect 依赖，留在父组件作为薄 wrapper 避免循环依赖。
 */
export function usePreviewVideos(opts: UsePreviewVideosOptions) {
  const previewGroups = ref<PreviewVideoGroup[]>([])
  /** 缩略图缓存：文件 path → canvas dataURL */
  const videoThumbnails = ref<Map<string, string>>(new Map())
  const selectedPreviewVideo = ref<PreviewVideoEntry | null>(null)
  const selectedPreviewGroup = ref<PreviewVideoGroup | null>(null)
  const showPreviewUploadConfirm = ref(false)
  const draggedPreviewFile = ref<PreviewVideoEntry | null>(null)

  /** 预览视频组的选中 key = 最新版本的文件路径（与 selectedPaths Set 对齐） */
  function previewGroupKey(g: PreviewVideoGroup): string {
    return g.versions[g.versions.length - 1].path
  }

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

  /** 扫描 03_preview 并分组 + 截帧（onMounted / refresh / 上传后都走这里） */
  async function loadPreviewGroups() {
    if (!opts.taskFolderPathRef.value) return
    try {
      const files = await invoke<PreviewVideoEntry[]>('scan_preview_videos', {
        taskPath: opts.taskFolderPathRef.value,
        nextcloudPreviewPath: opts.nextcloudPreviewPathRef.value,
      })
      previewGroups.value = groupPreviewVideos(files)
      captureGroupThumbnails(previewGroups.value)
    } catch (e) {
      console.error('加载预览视频失败:', e)
    }
  }

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

  /** 清空预览视频侧边栏选中（给 useMaterialSidebar 互斥调用） */
  function clearSelection() {
    selectedPreviewVideo.value = null
    selectedPreviewGroup.value = null
  }

  /** 单文件预览视频上传确认（单选拖拽场景，与混合上传独立） */
  async function confirmPreviewUpload() {
    showPreviewUploadConfirm.value = false
    const file = draggedPreviewFile.value
    if (!file) return
    try {
      await invoke('copy_preview_to_nextcloud', {
        filePath: file.path,
        nextcloudPreviewPath: opts.nextcloudPreviewPathRef.value,
      })
      // 刷新状态（重新扫描 03_preview 以更新 uploadStatus）
      await loadPreviewGroups()
      opts.onAfterUpload?.()
    } catch (err) {
      console.error('复制预览视频失败:', err)
    }
    draggedPreviewFile.value = null
  }

  function cancelPreviewUpload() {
    showPreviewUploadConfirm.value = false
    draggedPreviewFile.value = null
  }

  return {
    // State
    previewGroups,
    videoThumbnails,
    selectedPreviewVideo,
    selectedPreviewGroup,
    showPreviewUploadConfirm,
    draggedPreviewFile,
    // Computed
    selectedPreviewVideoAsFileEntry,
    selectedPreviewGroupVersionsAsFileEntries,
    // Functions
    previewGroupKey,
    groupPreviewVideos,
    loadPreviewGroups,
    clearSelection,
    confirmPreviewUpload,
    cancelPreviewUpload,
  }
}
