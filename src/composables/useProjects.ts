import { ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { useSettings } from './useSettings'

export interface ProjectInfo {
  name: string
  path: string
  deadline: string | null
  tasks: string[]
  task_count: number
  enabled_tasks: string[]
  completed_subtasks: string[]
  upload_prompted_tasks: string[]
  completed_tasks: string[]
  default_ae_file: string | null
  app_icon: string | null
}

export function useProjects() {
  const projects = ref<ProjectInfo[]>([])
  const loading = ref(false)
  const error = ref<string | null>(null)
  const { loadSettings } = useSettings()

  async function loadProjects() {
    loading.value = true
    error.value = null
    try {
      const s = await loadSettings()
      const rootDir = s?.general.projectRootDir ?? ''
      projects.value = await invoke<ProjectInfo[]>('scan_projects', {
        rootDir,
      })
    } catch (e) {
      error.value = String(e)
      console.error('扫描项目失败:', e)
    } finally {
      loading.value = false
    }
  }

  return {
    projects,
    loading,
    error,
    loadProjects,
  }
}
