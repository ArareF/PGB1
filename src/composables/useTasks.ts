import { ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'

export interface TaskInfo {
  name: string
  path: string
  size_bytes: number
  has_subtasks: boolean
  material_total: number
  material_uploaded: number
  video_total: number
  video_uploaded: number
  priority: string | null
}

export function useTasks() {
  const tasks = ref<TaskInfo[]>([])
  const loading = ref(false)
  const error = ref<string | null>(null)

  async function loadTasks(projectPath: string) {
    loading.value = true
    error.value = null
    try {
      tasks.value = await invoke<TaskInfo[]>('scan_tasks', {
        projectPath,
      })
    } catch (e) {
      error.value = String(e)
      console.error('扫描任务失败:', e)
    } finally {
      loading.value = false
    }
  }

  return {
    tasks,
    loading,
    error,
    loadTasks,
  }
}
