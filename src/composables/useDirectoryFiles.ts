import { ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'

export interface FileEntry {
  name: string
  path: string
  is_dir: boolean
  size_bytes: number
  extension: string
}

export function useDirectoryFiles() {
  const files = ref<FileEntry[]>([])
  const loading = ref(false)
  const error = ref<string | null>(null)

  async function loadFiles(dirPath: string) {
    loading.value = true
    error.value = null
    try {
      files.value = await invoke<FileEntry[]>('scan_directory', { dirPath })
    } catch (e) {
      error.value = String(e)
      console.error('扫描目录失败:', e)
    } finally {
      loading.value = false
    }
  }

  async function openInExplorer(path: string) {
    try {
      await invoke('open_in_explorer', { path })
    } catch (e) {
      console.error('打开文件管理器失败:', e)
    }
  }

  return { files, loading, error, loadFiles, openInExplorer }
}
