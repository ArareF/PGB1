import { ref, watch, nextTick, type Ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import type { MaterialInfo } from './useMaterials'
import type { MaterialVersion } from '../types/material'

export type SidebarDialog = 'none' | 'rename' | 'delete'

export interface UseMaterialSidebarOptions {
  /** 当前任务目录（Export/<taskId>） */
  taskFolderPathRef: Ref<string>
  /** 滚动容器引用（用于 preserveCardPosition 补偿滚动） */
  scrollRef: Ref<HTMLElement | null>
  /** 当前素材列表（confirmEditFps 刷新后需用来找到更新后的素材） */
  materials: Ref<MaterialInfo[]>
  /** 读笔记（useNotes 的 getNote，不存在时返回 null） */
  getNote: (key: string) => string | null
  /** 写笔记（useNotes 的 saveNote） */
  saveNote: (key: string, text: string) => Promise<void>
  /** 父组件的刷新方法（重命名 / 删除 / 改帧率后调用） */
  refresh: () => Promise<void>
  /** 选中素材时需关闭预览视频侧边栏（由 usePreviewVideos 提供 clearSelection） */
  onPreviewSelectionCleared: () => void
}

export function useMaterialSidebar(opts: UseMaterialSidebarOptions) {
  /** 当前选中的素材（驱动侧边栏） */
  const selectedMaterial = ref<MaterialInfo | null>(null)
  /** 选中素材的其他版本列表 */
  const versions = ref<MaterialVersion[]>([])
  /** 侧边栏内联弹窗（rename / delete） */
  const sidebarDialog = ref<SidebarDialog>('none')
  const renameInput = ref('')
  /** 侧边栏笔记文本 */
  const sidebarNoteText = ref('')
  /** 帧率内联编辑状态 */
  const editingFps = ref(false)
  const fpsInput = ref('')

  /** 记录卡片布局变化前的屏幕 Y 坐标，变化后补偿滚动避免视觉跳变 */
  function preserveCardPosition(cardSelector: string, action: () => void) {
    const container = opts.scrollRef.value
    const card = container?.querySelector(cardSelector) as HTMLElement | null
    const beforeY = card?.getBoundingClientRect().top ?? null

    action()

    nextTick(() => {
      requestAnimationFrame(() => {
        if (!container || beforeY === null) return
        const afterCard = container.querySelector(cardSelector) as HTMLElement | null
        if (!afterCard) return
        const afterY = afterCard.getBoundingClientRect().top
        const delta = afterY - beforeY
        container.scrollTop += delta
      })
    })
  }

  async function selectMaterial(material: MaterialInfo) {
    // 互斥：关闭预览视频侧边栏
    opts.onPreviewSelectionCleared()

    // 再次点击同一素材则关闭侧边栏
    if (selectedMaterial.value?.path === material.path) {
      closeSidebar()
      return
    }

    const wasOpen = !!selectedMaterial.value

    preserveCardPosition(
      wasOpen ? '.material-card.selected' : `.material-card[data-path="${CSS.escape(material.path)}"]`,
      () => {
        selectedMaterial.value = material
        versions.value = []
      },
    )

    try {
      versions.value = await invoke<MaterialVersion[]>('scan_material_versions', {
        taskPath: opts.taskFolderPathRef.value,
        baseName: material.name,
        materialType: material.material_type,
      })
    } catch (e) {
      console.error('加载版本失败:', e)
    }
  }

  function closeSidebar() {
    preserveCardPosition('.material-card.selected', () => {
      selectedMaterial.value = null
    })
  }

  // 侧边栏笔记同步：切换选中素材时重新载入该素材的笔记
  watch(() => selectedMaterial.value, (m: MaterialInfo | null) => {
    sidebarNoteText.value = m ? (opts.getNote('card:' + m.name.toLowerCase()) ?? '') : ''
  })

  async function onSidebarNoteSave() {
    const m = selectedMaterial.value
    if (!m) return
    await opts.saveNote('card:' + m.name.toLowerCase(), sidebarNoteText.value)
  }

  function openRenameDialog() {
    renameInput.value = selectedMaterial.value?.name ?? ''
    sidebarDialog.value = 'rename'
    nextTick(() => {
      (document.querySelector('.sidebar-dialog-input') as HTMLInputElement)?.focus()
    })
  }

  function openDeleteDialog() {
    sidebarDialog.value = 'delete'
  }

  function closeSidebarDialog() {
    sidebarDialog.value = 'none'
    renameInput.value = ''
  }

  async function confirmRename() {
    const mat = selectedMaterial.value
    if (!mat || !renameInput.value.trim() || renameInput.value.trim() === mat.name) {
      closeSidebarDialog()
      return
    }
    try {
      await invoke('rename_material', {
        taskPath: opts.taskFolderPathRef.value,
        baseName: mat.name,
        newBaseName: renameInput.value.trim(),
        materialType: mat.material_type,
      })
      closeSidebarDialog()
      closeSidebar()
      await opts.refresh()
    } catch (e) {
      console.error('重命名失败:', e)
    }
  }

  async function confirmDelete() {
    const mat = selectedMaterial.value
    if (!mat) return
    try {
      await invoke('delete_material', {
        taskPath: opts.taskFolderPathRef.value,
        baseName: mat.name,
        materialType: mat.material_type,
      })
      closeSidebarDialog()
      closeSidebar()
      await opts.refresh()
    } catch (e) {
      console.error('删除失败:', e)
    }
  }

  function startEditFps() {
    const mat = selectedMaterial.value
    if (!mat || mat.fps == null) return
    fpsInput.value = String(mat.fps)
    editingFps.value = true
    nextTick(() => {
      (document.querySelector('.fps-input') as HTMLInputElement)?.select()
    })
  }

  function cancelEditFps() {
    editingFps.value = false
    fpsInput.value = ''
  }

  async function confirmEditFps() {
    const mat = selectedMaterial.value
    if (!mat) return
    const newFps = parseInt(fpsInput.value, 10)
    if (!newFps || newFps <= 0 || newFps === mat.fps) {
      cancelEditFps()
      return
    }
    try {
      await invoke('rename_sequence_fps', {
        taskPath: opts.taskFolderPathRef.value,
        baseName: mat.name,
        oldFps: mat.fps,
        newFps,
      })
      cancelEditFps()
      await opts.refresh()
      // refresh 只更新 materials.value，selectedMaterial 需手动同步到新数据
      const updated = opts.materials.value.find(m => m.name === mat.name)
      if (updated) {
        selectedMaterial.value = updated
        versions.value = await invoke<MaterialVersion[]>('scan_material_versions', {
          taskPath: opts.taskFolderPathRef.value,
          baseName: updated.name,
          materialType: updated.material_type,
        })
      }
    } catch (e) {
      console.error('修改帧率失败:', e)
    }
  }

  /** 打开序列帧工程文件（.tps） */
  async function openTpsFile() {
    const mat = selectedMaterial.value
    if (!mat) return
    const doneVersion = versions.value.find(v => v.stage === '02_done')
    if (!doneVersion) return
    const tpsPath = doneVersion.folder_path.replace(/\\/g, '/') + '/' + mat.name + '.tps'
    try {
      await invoke('open_file', { path: tpsPath })
    } catch (e) {
      console.error('打开工程文件失败:', e)
    }
  }

  return {
    // State
    selectedMaterial,
    versions,
    sidebarDialog,
    renameInput,
    sidebarNoteText,
    editingFps,
    fpsInput,
    // Functions
    selectMaterial,
    closeSidebar,
    onSidebarNoteSave,
    openRenameDialog,
    openDeleteDialog,
    closeSidebarDialog,
    confirmRename,
    confirmDelete,
    startEditFps,
    cancelEditFps,
    confirmEditFps,
    openTpsFile,
  }
}
