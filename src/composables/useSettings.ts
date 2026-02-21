import { ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { open } from '@tauri-apps/plugin-dialog'

export interface WorkflowSettings {
  imaginePath: string
  texturePackerCliPath: string
  texturePackerGuiPath: string
}

export interface TranslationSettings {
  apiKey: string
  model: string
  shortcut: string
  useCalculatorKey: boolean
  langA: string
  langB: string
}

export interface GeneralSettings {
  projectRootDir: string
  uiScale: number  // UI 缩放比例（1.0 = 100%，默认 1.0）
  autoStart: boolean
}

export interface PreviewSettings {
  defaultFps: number
  backgroundTransparent: boolean
}

export interface AppSettings {
  workflow: WorkflowSettings
  translation: TranslationSettings
  general: GeneralSettings
  preview: PreviewSettings
}

/** 模块级单例状态 */
const settings = ref<AppSettings | null>(null)
const loading = ref(false)
const error = ref<string | null>(null)

export function useSettings() {
  /** 加载设置 */
  async function loadSettings() {
    if (settings.value) return settings.value
    
    loading.value = true
    error.value = null
    try {
      const res = await invoke<AppSettings>('load_settings')
      settings.value = res
      return res
    } catch (e) {
      error.value = String(e)
      console.error('加载设置失败:', e)
      return null
    } finally {
      loading.value = false
    }
  }

  /** 保存设置 */
  async function saveSettings(newSettings: AppSettings) {
    loading.value = true
    error.value = null
    try {
      await invoke('save_settings', { settings: newSettings })
      settings.value = JSON.parse(JSON.stringify(newSettings)) // 深拷贝以解绑引用
    } catch (e) {
      error.value = String(e)
      console.error('保存设置失败:', e)
      throw e
    } finally {
      loading.value = false
    }
  }

  /** 拾取文件 */
  async function pickFile(title: string, filters?: { name: string, extensions: string[] }[]): Promise<string | null> {
    try {
      const selected = await open({
        title,
        multiple: false,
        directory: false,
        filters,
      })
      return (selected as string) || null
    } catch (e) {
      console.error('选择文件失败:', e)
      return null
    }
  }

  /** 拾取目录 */
  async function pickDir(title: string): Promise<string | null> {
    try {
      const selected = await open({
        title,
        multiple: false,
        directory: true,
      })
      return (selected as string) || null
    } catch (e) {
      console.error('选择目录失败:', e)
      return null
    }
  }

  return {
    settings,
    loading,
    error,
    loadSettings,
    saveSettings,
    pickFile,
    pickDir,
  }
}
