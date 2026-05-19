import { ref, toRaw } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { open } from '@tauri-apps/plugin-dialog'

export interface WorkflowSettings {
  imaginePath: string
  texturePackerCliPath: string
  texturePackerGuiPath: string
  tpScale: number
  tpWebpQuality: number
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
  language: 'zh-CN' | 'en'
  onboarded: boolean
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
      // N-21：Vue reactive Proxy 含 __v_isRef 等 Symbol key，直接 structuredClone 会抛
      // DataCloneError。先 toRaw 脱壳，再 structuredClone 深拷贝——比 JSON.parse(JSON.stringify)
      // 更鲁棒：未来若 AppSettings 里加 Date 字段，Date 也能正确保留（JSON roundtrip 会变成字符串）。
      // 前提：AppSettings 所有字段为 JSON-兼容类型或 structuredClone-兼容类型（Date/Map/Set）。
      const plain = structuredClone(toRaw(newSettings)) as AppSettings
      await invoke('save_settings', { settings: plain })
      settings.value = plain
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
