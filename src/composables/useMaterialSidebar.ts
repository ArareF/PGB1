import { ref, watch, nextTick, type Ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import type { MaterialInfo } from './useMaterials'
import type { MaterialVersion } from '../types/material'

export type SidebarDialog = 'none' | 'rename' | 'delete' | 'reset' | 'not-sequence'

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
  /** 「更新」时生成写入笔记的制作参数块（由 TaskPage 提供，带 i18n） */
  formatResetNote: (material: MaterialInfo) => string
  /** TexturePacker GUI 路径（「修改」序列帧工程时阻塞打开；为空则退回系统关联打开，不做尺寸重整理） */
  texturePackerGuiPathRef: Ref<string>
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
  /** 弹窗操作错误提示（重命名/删除失败后展示给用户，N-12） */
  const sidebarDialogError = ref<string | null>(null)
  const sidebarDialogBusy = ref(false)

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

    // 参考点选择：必须用 data-path 精确锁定"同一张卡片"，否则 before/after
    // 会查到两张不同的卡片导致 delta 变成"卡片间距"而不是"位移"。
    // - 切换场景：用 旧 卡片作参考（action 后它失去 .selected 但 data-path 还在）
    // - 首次打开：用 目标 卡片作参考（action 后它获得 .selected，data-path 不变）
    const referencePath = selectedMaterial.value?.path ?? material.path

    preserveCardPosition(
      `.material-card[data-path="${CSS.escape(referencePath)}"]`,
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
    const prevPath = selectedMaterial.value?.path
    if (!prevPath) {
      selectedMaterial.value = null
      return
    }
    // 用 data-path 锁定被关闭的卡片作为参考点：action 后它失去 .selected
    // 但 data-path 仍在，delta 反映的是"侧边栏消失导致的网格重排位移"。
    // 原版用 .material-card.selected 会导致 after 查不到任何元素，补偿
    // 直接被 `if (!afterCard) return` 跳过。
    preserveCardPosition(
      `.material-card[data-path="${CSS.escape(prevPath)}"]`,
      () => {
        selectedMaterial.value = null
      },
    )
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
    sidebarDialogError.value = null
    nextTick(() => {
      // N-13：限定在当前 overlay 内查，避免未来多个同名输入并存时误命中
      const overlay = document.querySelector('.sidebar-dialog-overlay')
      ;(overlay?.querySelector('.sidebar-dialog-input') as HTMLInputElement | null)?.focus()
    })
  }

  function openDeleteDialog() {
    sidebarDialog.value = 'delete'
    sidebarDialogError.value = null
  }

  function openResetDialog() {
    sidebarDialog.value = 'reset'
    sidebarDialogError.value = null
  }

  function openNotSequenceDialog() {
    sidebarDialog.value = 'not-sequence'
    sidebarDialogError.value = null
  }

  function closeSidebarDialog() {
    sidebarDialog.value = 'none'
    renameInput.value = ''
    sidebarDialogError.value = null
    sidebarDialogBusy.value = false
  }

  async function confirmRename() {
    const mat = selectedMaterial.value
    if (!mat || !renameInput.value.trim() || renameInput.value.trim() === mat.name) {
      closeSidebarDialog()
      return
    }
    sidebarDialogBusy.value = true
    sidebarDialogError.value = null
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
      // N-12：不再静默关闭弹窗，让用户看到错误原因并可重试
      console.error('[useMaterialSidebar] 重命名失败:', e)
      sidebarDialogError.value = String(e)
    } finally {
      sidebarDialogBusy.value = false
    }
  }

  async function confirmDelete() {
    const mat = selectedMaterial.value
    if (!mat) return
    sidebarDialogBusy.value = true
    sidebarDialogError.value = null
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
      // N-12：不再静默关闭弹窗，让用户看到错误原因并可重试
      console.error('[useMaterialSidebar] 删除失败:', e)
      sidebarDialogError.value = String(e)
    } finally {
      sidebarDialogBusy.value = false
    }
  }

  /** 「更新」：把制作参数记入笔记后，清除派生版本（保留 00_original） */
  async function confirmReset() {
    const mat = selectedMaterial.value
    if (!mat) return
    sidebarDialogBusy.value = true
    sidebarDialogError.value = null
    try {
      // 先把制作参数追加进笔记——清派生版本后 scale/帧率 这些信息就没了
      const key = 'card:' + mat.name.toLowerCase()
      const existing = opts.getNote(key) ?? ''
      const block = opts.formatResetNote(mat)
      await opts.saveNote(key, existing ? `${existing}\n\n${block}` : block)
      // 清派生版本（01_scale + 02_done 归档时光机，nextcloud 标记直删，保留 00_original）
      await invoke('reset_material_versions', {
        taskPath: opts.taskFolderPathRef.value,
        baseName: mat.name,
        materialType: mat.material_type,
      })
      closeSidebarDialog()
      closeSidebar()
      await opts.refresh()
    } catch (e) {
      // N-12 同款：不静默，展示错误让用户可重试
      console.error('[useMaterialSidebar] 更新(清派生版本)失败:', e)
      sidebarDialogError.value = String(e)
    } finally {
      sidebarDialogBusy.value = false
    }
  }

  function startEditFps() {
    const mat = selectedMaterial.value
    if (!mat || mat.fps == null) return
    fpsInput.value = String(mat.fps)
    editingFps.value = true
    nextTick(() => {
      // N-13：限定在 sidebar-shell 容器内查（fps-input 属于侧边栏详情区）
      const shell = document.querySelector('.sidebar-shell')
      ;(shell?.querySelector('.fps-input') as HTMLInputElement | null)?.select()
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

  /** 标记当前素材为「非序列帧」：写入 00_original/非序列帧.txt 后刷新 */
  async function confirmMarkNotSequence() {
    const mat = selectedMaterial.value
    if (!mat) return
    sidebarDialogBusy.value = true
    sidebarDialogError.value = null
    try {
      await invoke('mark_not_sequence', {
        taskPath: opts.taskFolderPathRef.value,
        baseName: mat.name,
      })
      closeSidebarDialog()
      closeSidebar()
      await opts.refresh()
    } catch (e) {
      console.error('[useMaterialSidebar] 标记非序列帧失败:', e)
      sidebarDialogError.value = String(e)
    } finally {
      sidebarDialogBusy.value = false
    }
  }

  /** 「修改」序列帧工程（.tps）：阻塞打开 TexturePacker，关闭后按新 scale 重整理并刷新 */
  async function openTpsFile() {
    const mat = selectedMaterial.value
    if (!mat) return
    const doneVersion = versions.value.find(v => v.stage === '02_done')
    if (!doneVersion) return
    const tpsPath = doneVersion.folder_path.replace(/\\/g, '/') + '/' + mat.name + '.tps'
    try {
      // 阻塞等 TP 关闭；若用户改了 scale，后端会把 [an-旧-fps] 目录重命名为 [an-新-fps]
      // （命令内会先自愈 .tps 内 sprite 源路径深度，兼顾历史错位一级的坏文件）
      await invoke('edit_sequence_tps', {
        tpsPath,
        guiPath: opts.texturePackerGuiPathRef.value,
      })
      // 尺寸可能已变——刷新重新归类，并把侧边栏选中同步到刷新后的素材
      await opts.refresh()
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
      console.error('修改工程文件失败:', e)
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
    sidebarDialogError,
    sidebarDialogBusy,
    // Functions
    selectMaterial,
    closeSidebar,
    onSidebarNoteSave,
    openRenameDialog,
    openDeleteDialog,
    openResetDialog,
    openNotSequenceDialog,
    closeSidebarDialog,
    confirmRename,
    confirmDelete,
    confirmReset,
    confirmMarkNotSequence,
    startEditFps,
    cancelEditFps,
    confirmEditFps,
    openTpsFile,
  }
}
