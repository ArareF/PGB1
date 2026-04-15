import { ref, computed, watch, onMounted } from 'vue'
import { invoke, convertFileSrc } from '@tauri-apps/api/core'
import { open as openDialog } from '@tauri-apps/plugin-dialog'
import { useI18n } from 'vue-i18n'

export type ShortcutType = 'app' | 'folder' | 'web'

export interface AppShortcut {
  name: string
  target_path: string
}

export interface ShortcutSavePayload {
  shortcut_type: string
  name: string
  path: string
  custom_icon: string | null
}

/**
 * 快捷方式对话框表单：三类快捷方式（app/folder/web）的创建流程
 *
 * 抽出 composable 的动机：
 * - 把 `ShortcutDialog.vue` 的类型切换 / 路径选择 / 图标预览 / 列表扫描从 SFC 剥离
 * - 组件只保留 template & style，所有副作用逻辑可测
 *
 * @param onSave 外部保存回调（由组件包装为 emit('save', ...)）
 */
export function useShortcutForm(onSave: (data: ShortcutSavePayload) => void) {
  const { t } = useI18n()

  // ─── 核心表单状态 ───────────────────────────────────────
  const type = ref<ShortcutType>('app')
  const path = ref('')
  const name = ref('')
  const customIconPath = ref<string | null>(null)
  const previewLoading = ref(false)

  const customIconUrl = computed(() =>
    customIconPath.value
      ? convertFileSrc(customIconPath.value.replace(/\\/g, '/'))
      : null
  )

  // ─── 应用列表（仅 type=app 时使用）──────────────────────
  const appList = ref<AppShortcut[]>([])
  const appSearch = ref('')
  const appListLoading = ref(false)
  const selectedApp = ref<AppShortcut | null>(null)

  const filteredApps = computed(() => {
    const q = appSearch.value.trim().toLowerCase()
    if (!q) return appList.value
    return appList.value.filter(a => a.name.toLowerCase().includes(q))
  })

  const canFetchPreview = computed(() =>
    (type.value === 'app' && !!selectedApp.value) ||
    (type.value === 'web' && path.value.trim().length > 0)
  )

  const canSave = computed(() => path.value.trim().length > 0 && name.value.trim().length > 0)

  // ─── 初始化 & 类型切换时的列表加载 ──────────────────────
  async function loadAppList() {
    appListLoading.value = true
    try {
      appList.value = await invoke<AppShortcut[]>('scan_app_shortcuts')
    } catch (e) {
      console.error('扫描应用列表失败', e)
    } finally {
      appListLoading.value = false
    }
  }

  // 切换为应用类型时加载列表（首次挂载 onMounted 会直接调一次）
  watch(type, async (newType) => {
    if (newType === 'app' && appList.value.length === 0 && !appListLoading.value) {
      await loadAppList()
    }
  })

  onMounted(async () => {
    // 默认是应用类型，直接加载
    if (type.value === 'app') {
      await loadAppList()
    }
  })

  // ─── 应用列表交互 ────────────────────────────────────────
  function selectApp(app: AppShortcut) {
    selectedApp.value = app
    path.value = app.target_path
    name.value = app.name
    customIconPath.value = null  // 换应用时重置自定义图标
  }

  // 切换类型时清空表单
  function selectType(newType: ShortcutType) {
    type.value = newType
    path.value = ''
    name.value = ''
    selectedApp.value = null
    appSearch.value = ''
    customIconPath.value = null
  }

  // ─── 图标预览（抓 exe/favicon）──────────────────────────
  async function fetchIconPreview() {
    if (previewLoading.value || !canFetchPreview.value) return
    const tempId = crypto.randomUUID()
    previewLoading.value = true
    try {
      let result: string | null = null
      if (type.value === 'app') {
        result = await invoke<string>('extract_exe_icon', { exePath: path.value, iconId: tempId })
      } else if (type.value === 'web') {
        result = await invoke<string | null>('fetch_favicon', { url: path.value, iconId: tempId })
      }
      if (result) customIconPath.value = result
    } catch (e) {
      console.error('图标获取失败', e)
    } finally {
      previewLoading.value = false
    }
  }

  async function browseCustomIcon() {
    try {
      const selected = await openDialog({
        multiple: false,
        filters: [{ name: t('shortcut.imageFilter'), extensions: ['png', 'jpg', 'jpeg', 'ico', 'bmp', 'webp'] }],
      })
      if (selected && typeof selected === 'string') {
        const tempId = crypto.randomUUID()
        const cachedPath = await invoke<string>('copy_icon_to_cache', {
          srcPath: selected,
          iconId: tempId,
        })
        customIconPath.value = cachedPath
      }
    } catch (e) {
      console.error('图标复制到缓存失败', e)
    }
  }

  // ─── 文件夹 / exe 路径选择 ──────────────────────────────
  async function browse() {
    try {
      if (type.value === 'folder') {
        const selected = await openDialog({ multiple: false, directory: true })
        if (selected && typeof selected === 'string') {
          path.value = selected
          const parts = selected.replace(/\\/g, '/').split('/')
          name.value = parts[parts.length - 1] || selected
        }
      }
    } catch (e) {
      console.error('文件夹选择失败', e)
    }
  }

  async function browseExe() {
    try {
      const selected = await openDialog({
        multiple: false,
        filters: [{ name: t('shortcut.typeApp'), extensions: ['exe'] }],
      })
      if (selected && typeof selected === 'string') {
        path.value = selected
        const parts = selected.replace(/\\/g, '/').split('/')
        const filename = parts[parts.length - 1]
        name.value = filename.replace(/\.exe$/i, '')
        selectedApp.value = { name: name.value, target_path: selected }
      }
    } catch (e) {
      console.error('文件选择失败', e)
    }
  }

  // ─── 保存 ────────────────────────────────────────────────
  function handleSave() {
    if (!canSave.value) return
    onSave({
      shortcut_type: type.value,
      name: name.value.trim(),
      path: path.value.trim(),
      custom_icon: customIconPath.value,
    })
  }

  return {
    // state
    type,
    path,
    name,
    customIconPath,
    customIconUrl,
    previewLoading,
    appList,
    appSearch,
    appListLoading,
    selectedApp,
    // computed
    filteredApps,
    canFetchPreview,
    canSave,
    // actions
    selectApp,
    selectType,
    fetchIconPreview,
    browseCustomIcon,
    browse,
    browseExe,
    handleSave,
  }
}
