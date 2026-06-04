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
