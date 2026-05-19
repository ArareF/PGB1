import { ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import type { ArchivedMaterialVersion } from '../types/task'

/** 素材归档时光机数据源：list / restore / delete */
export function useArchivedMaterials(projectPath: () => string) {
  const versions = ref<ArchivedMaterialVersion[]>([])
  const loading = ref(false)

  async function load() {
    const p = projectPath()
    if (!p) return
    loading.value = true
    try {
      versions.value = await invoke<ArchivedMaterialVersion[]>('list_archived_materials', {
        projectPath: p,
      })
    } catch (e) {
      console.error('[useArchivedMaterials] 加载素材归档失败:', e)
      versions.value = []
    } finally {
      loading.value = false
    }
  }

  async function restore(version: ArchivedMaterialVersion): Promise<void> {
    await invoke('restore_archived_material', {
      projectPath: projectPath(),
      taskName: version.task_name,
      baseName: version.base_name,
      timestamp: version.timestamp,
    })
  }

  async function remove(version: ArchivedMaterialVersion): Promise<void> {
    await invoke('delete_archived_material_version', {
      projectPath: projectPath(),
      taskName: version.task_name,
      baseName: version.base_name,
      timestamp: version.timestamp,
    })
  }

  return { versions, loading, load, restore, remove }
}
