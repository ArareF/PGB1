import { ref, computed, type Ref } from 'vue'
import { convertFileSrc, invoke } from '@tauri-apps/api/core'
import { mediaVersion } from './useMediaCache'
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
  /** 上次截帧所处的刷新代次，与 mediaVersion 不一致时整表作废 */
  let capturedVersion = mediaVersion.value
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

  /** 截帧超时：损坏/不支持格式的视频既不触发 seeked 也不 error，5 秒兜底 */
  const THUMBNAIL_TIMEOUT_MS = 5000

  /**
   * 对分组数据截帧（取每组 latest），结果写入 videoThumbnails。
   *
   * 修复项：
   *   1. 先做 GC：清理 groups 里已消失的 path，避免长时间运行缓存膨胀
   *   2. 每个 video 设 5s 超时兜底，防损坏视频让 <video> + closure 永久驻留
   *   3. 成功/失败/超时三路径都走 cleanup()：断 src + remove DOM 元素
   *   4. 必须设 crossOrigin='anonymous'：Tauri 2 用 http://asset.localhost 服务本地文件，
   *      相对应用 origin 是跨域资源；不带 CORS 请求 canvas 会被污染，toDataURL 抛
   *      SecurityError。asset 响应带 Access-Control-Allow-Origin:*，匿名模式可通过。
   */
  function captureGroupThumbnails(groups: PreviewVideoGroup[]) {
    // 手动刷新：整表作废重截。截帧缓存按 path 命中，同名覆盖的新版本视频路径不变，
    // 不作废的话下面的 has(path) 会一直短路，预览视频永远停在旧首帧
    if (capturedVersion !== mediaVersion.value) {
      capturedVersion = mediaVersion.value
      videoThumbnails.value = new Map()
    }

    // GC：丢弃不在当前 groups 里的缩略图缓存（切换任务/刷新后旧 dataURL 不再需要）
    const currentPaths = new Set(groups.map(g => g.versions[g.versions.length - 1].path))
    const cached = videoThumbnails.value
    if (cached.size > currentPaths.size) {
      const gcMap = new Map<string, string>()
      for (const [path, url] of cached) {
        if (currentPaths.has(path)) gcMap.set(path, url)
      }
      videoThumbnails.value = gcMap
    }

    for (const group of groups) {
      const latest = group.versions[group.versions.length - 1]
      if (videoThumbnails.value.has(latest.path)) continue

      const video = document.createElement('video')
      video.crossOrigin = 'anonymous'
      video.preload = 'metadata'
      video.muted = true
      video.src = `${convertFileSrc(latest.path)}?v=${capturedVersion}`
      // currentTime 必须在 loadedmetadata 后设置，否则在 metadata 未就绪时会静默失败

      let settled = false
      const cleanup = () => {
        if (settled) return
        settled = true
        clearTimeout(timer)
        video.src = ''
        video.remove()
      }

      const timer = setTimeout(() => {
        console.warn(
          `[usePreviewVideos] 截帧超时 (${THUMBNAIL_TIMEOUT_MS}ms) ` +
          `readyState=${video.readyState} networkState=${video.networkState} ${latest.path}`,
        )
        cleanup()
      }, THUMBNAIL_TIMEOUT_MS)

      video.addEventListener('loadedmetadata', () => {
        if (settled) return
        // 对极短视频取 duration 的 10% 兜底，避免 seek 到超出时长
        const seekTo = Number.isFinite(video.duration) && video.duration > 0
          ? Math.min(0.1, video.duration * 0.1)
          : 0.1
        video.currentTime = seekTo
      }, { once: true })

      video.addEventListener('seeked', () => {
        if (settled) return
        try {
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
        } catch (err) {
          console.warn(`[usePreviewVideos] 截帧异常 ${latest.path}:`, err)
        } finally {
          cleanup()
        }
      }, { once: true })

      video.addEventListener('error', () => {
        // cleanup 里 video.src='' 会触发 error 事件（msg="Empty src attribute"），
        // 所以必须先检查 settled，否则会把自己放出来的 error 当真打 warn
        if (settled) return
        // 打全 MediaError 诊断信息：code 1=ABORTED 2=NETWORK 3=DECODE 4=SRC_NOT_SUPPORTED
        const codeMap: Record<number, string> = {
          1: 'MEDIA_ERR_ABORTED',
          2: 'MEDIA_ERR_NETWORK',
          3: 'MEDIA_ERR_DECODE',
          4: 'MEDIA_ERR_SRC_NOT_SUPPORTED',
        }
        const mediaErr = video.error
        const code = mediaErr?.code ? `${mediaErr.code}/${codeMap[mediaErr.code] ?? 'UNKNOWN'}` : '?'
        const msg = mediaErr?.message || '(no message)'
        console.warn(
          `[usePreviewVideos] 截帧失败 code=${code} msg="${msg}" ` +
          `networkState=${video.networkState} readyState=${video.readyState} ${latest.path}`,
        )
        cleanup()
      }, { once: true })
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
