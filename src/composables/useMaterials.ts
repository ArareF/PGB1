import { ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'

export type MaterialType = 'image' | 'sequence' | 'video' | 'other'
export type MaterialProgress = 'none' | 'original' | 'scaled' | 'done' | 'uploaded' | 'broken'

export interface MaterialInfo {
  name: string
  file_name: string
  path: string
  material_type: MaterialType
  progress: MaterialProgress
  size_bytes: number
  frame_count: number
  extension: string
  preview_path: string | null
  /** 预览文件 mtime（Unix 秒），用作缓存破坏版本号 */
  preview_version: number
  scales: number[]
  fps: number | null
}

/**
 * 素材唯一标识（SSOT）——所有"这是不是同一个素材"的判定必须走这里。
 *
 * 单字段都不够用：
 * - `path` 不唯一：未规范化的 00_original 里序列帧平铺存放，同目录下 N 个序列的 path
 *   全是这个目录本身（后端 scan_materials 散落序列帧分支）。
 * - `name` 不唯一：静帧的 name 是去掉扩展名和 `_01` 后缀的基础名，
 *   `a.png` / `a.jpg` / `a_01.png` 会撞成同一个 name。
 *
 * 三段组合在所有场景下唯一。分隔符用 `|`：Windows 文件名非法字符，不会与内容冲突。
 */
export function materialUid(m: MaterialInfo): string {
  return `${m.material_type}|${m.name}|${m.path}`
}

export function useMaterials() {
  const materials = ref<MaterialInfo[]>([])
  const loading = ref(false)
  const error = ref<string | null>(null)

  async function loadMaterials(taskPath: string) {
    loading.value = true
    error.value = null
    try {
      materials.value = await invoke<MaterialInfo[]>('scan_materials', { taskPath })
    } catch (e) {
      error.value = String(e)
      console.error('扫描素材失败:', e)
    } finally {
      loading.value = false
    }
  }

  return { materials, loading, error, loadMaterials }
}
