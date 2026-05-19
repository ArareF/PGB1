import { ref, computed, onMounted } from 'vue'
import { useI18n } from 'vue-i18n'
import { invoke } from '@tauri-apps/api/core'
import { open } from '@tauri-apps/plugin-dialog'
import { useSettings } from './useSettings'
import type { AppSettings } from './useSettings'

export type StepId = 'language' | 'project-dir' | 'tool-paths' | 'attendance'
export const STEPS: StepId[] = ['language', 'project-dir', 'tool-paths', 'attendance']

interface AppShortcutEntry {
  name: string
  target_path: string
}

/**
 * 新手引导表单：4 步向导 + 自动扫描系统已装应用补工具路径 + 完成时统一保存
 *
 * 抽出 composable 的动机：将 `OnboardingDialog.vue` 的状态机 / 系统扫描逻辑 / 保存闭环
 * 从 SFC 剥离，让组件专注 template & style，同时方便后续单测扫描逻辑。
 */
export function useOnboardingForm(
  onComplete: (mode: 'off' | 'auto' | 'record_only') => void
) {
  const { t, locale } = useI18n()
  const { pickFile } = useSettings()

  // ─── 步骤状态 ───────────────────────────────────────────
  const currentStepIndex = ref(0)
  const currentStep = computed(() => STEPS[currentStepIndex.value])
  const isLastStep = computed(() => currentStepIndex.value === STEPS.length - 1)

  // ─── 表单数据 ───────────────────────────────────────────
  const formLanguage = ref<'zh-CN' | 'en'>(locale.value as 'zh-CN' | 'en')
  const formProjectDir = ref('')
  const formImaginePath = ref('')
  const formTpCliPath = ref('')
  const formTpGuiPath = ref('')
  const formAttendanceMode = ref<'off' | 'auto' | 'record_only'>('off')

  // 工具自动检测标记（用于 UI 展示 "自动检测到" 标签）
  const imagineAutoDetected = ref(false)
  const tpCliAutoDetected = ref(false)
  const tpGuiAutoDetected = ref(false)

  // 保存状态（防止失败后静默 onComplete 导致引导弹窗二次出现 + 覆盖已修改设置）
  const isSaving = ref(false)
  const saveError = ref<string | null>(null)

  // ─── 初始化：扫描系统已装应用补齐未填路径 ──────────────────
  onMounted(async () => {
    // 1. 先读已有设置（老用户重置 onboarded 场景）
    try {
      const s = await invoke<AppSettings>('load_settings')
      if (s.general.projectRootDir) formProjectDir.value = s.general.projectRootDir
      if (s.workflow.imaginePath) formImaginePath.value = s.workflow.imaginePath
      if (s.workflow.texturePackerCliPath) formTpCliPath.value = s.workflow.texturePackerCliPath
      if (s.workflow.texturePackerGuiPath) formTpGuiPath.value = s.workflow.texturePackerGuiPath
    } catch (_) { /* 首次运行可能没有设置文件 */ }

    // 2. 扫描系统已安装应用，补充未填充的路径
    if (!formImaginePath.value || !formTpCliPath.value || !formTpGuiPath.value) {
      try {
        const apps = await invoke<AppShortcutEntry[]>('scan_app_shortcuts')

        if (!formImaginePath.value) {
          const imagine = apps.find(a => /^imagine$/i.test(a.name))
          if (imagine) {
            formImaginePath.value = imagine.target_path
            imagineAutoDetected.value = true
          }
        }

        if (!formTpCliPath.value || !formTpGuiPath.value) {
          // 从开始菜单快捷方式中找 TexturePacker
          const tpApps = apps.filter(a => /texturepacker/i.test(a.name))
          for (const app of tpApps) {
            const p = app.target_path.toLowerCase()
            if (!formTpCliPath.value && p.includes('\\bin\\') && p.endsWith('texturepacker.exe')) {
              formTpCliPath.value = app.target_path
              tpCliAutoDetected.value = true
            } else if (!formTpGuiPath.value && p.endsWith('texturepackergui.exe')) {
              formTpGuiPath.value = app.target_path
              tpGuiAutoDetected.value = true
            }
          }
          // 互推：知道一个就能推出另一个（同一安装目录）
          if (formTpGuiPath.value && !formTpCliPath.value) {
            // GUI: .../TexturePackerGUI.exe → CLI: .../bin/TexturePacker.exe
            const guiDir = formTpGuiPath.value.replace(/[/\\][^/\\]+$/, '')
            // GUI 已在 bin/ 下（非标准布局）时，不再追加 bin/
            formTpCliPath.value = /[/\\]bin$/i.test(guiDir)
              ? guiDir + '\\TexturePacker.exe'
              : guiDir + '\\bin\\TexturePacker.exe'
            tpCliAutoDetected.value = true
          } else if (formTpCliPath.value && !formTpGuiPath.value) {
            // CLI: .../bin/TexturePacker.exe → GUI: .../TexturePackerGUI.exe
            const dir = formTpCliPath.value.replace(/[/\\]bin[/\\][^/\\]+$/, '')
            formTpGuiPath.value = dir + '\\TexturePackerGUI.exe'
            tpGuiAutoDetected.value = true
          }
        }
      } catch (e) {
        console.error('扫描系统应用失败:', e)
      }
    }
  })

  // ─── 校验：当前步骤是否填好了 ─────────────────────────────
  const canProceed = computed(() => {
    switch (currentStep.value) {
      case 'project-dir':
        return !!formProjectDir.value
      case 'tool-paths':
        return !!formImaginePath.value && !!formTpCliPath.value
      default:
        return true
    }
  })

  // ─── 导航 ───────────────────────────────────────────────
  function goNext() {
    if (canProceed.value && currentStepIndex.value < STEPS.length - 1) {
      currentStepIndex.value++
    }
  }

  function goPrev() {
    if (currentStepIndex.value > 0) {
      currentStepIndex.value--
    }
  }

  // ─── 语言切换即时生效 ───────────────────────────────────
  function setLanguage(lang: 'zh-CN' | 'en') {
    formLanguage.value = lang
    locale.value = lang
  }

  // ─── 选择路径 ───────────────────────────────────────────
  async function selectProjectDir() {
    try {
      const dir = await open({
        title: t('settings.selectProjectRootDir'),
        multiple: false,
        directory: true,
      })
      if (dir) formProjectDir.value = dir as string
    } catch (e) {
      console.error('选择目录失败:', e)
    }
  }

  async function selectImaginePath() {
    const path = await pickFile(t('settings.selectExecutable'), [
      { name: 'Executable', extensions: ['exe'] },
    ])
    if (path) {
      formImaginePath.value = path
      imagineAutoDetected.value = false
    }
  }

  async function selectTpCliPath() {
    const path = await pickFile(t('settings.selectExecutable'), [
      { name: 'Executable', extensions: ['exe'] },
    ])
    if (path) {
      formTpCliPath.value = path
      tpCliAutoDetected.value = false
    }
  }

  async function selectTpGuiPath() {
    const path = await pickFile(t('settings.selectExecutable'), [
      { name: 'Executable', extensions: ['exe'] },
    ])
    if (path) {
      formTpGuiPath.value = path
      tpGuiAutoDetected.value = false
    }
  }

  // ─── 完成引导：合并表单 → 保存设置 + 打卡配置 → emit complete ──
  async function finish() {
    if (isSaving.value) return
    isSaving.value = true
    saveError.value = null
    try {
      const current = await invoke<AppSettings>('load_settings')

      // 合并引导数据
      current.general.language = formLanguage.value
      current.general.onboarded = true
      if (formProjectDir.value) {
        current.general.projectRootDir = formProjectDir.value
      }
      if (formImaginePath.value) {
        current.workflow.imaginePath = formImaginePath.value
      }
      if (formTpCliPath.value) {
        current.workflow.texturePackerCliPath = formTpCliPath.value
      }
      if (formTpGuiPath.value) {
        current.workflow.texturePackerGuiPath = formTpGuiPath.value
      }

      // 关键：设置写入必须成功，否则 onboarded=true 不会持久化，引导弹窗会二次出现
      await invoke('save_settings', { settings: current })

      // 保存打卡模式（非关键路径：失败不阻断引导完成，但要告知用户）
      if (formAttendanceMode.value !== 'off') {
        try {
          const config = await invoke<Record<string, unknown>>('load_attendance_config')
          ;(config as Record<string, unknown>).mode = formAttendanceMode.value
          await invoke('save_attendance_config', { config })
        } catch (e) {
          console.error('保存打卡配置失败:', e)
          // 打卡配置失败不阻断整体流程，但把错误浮出来让用户知情
          saveError.value = t('onboarding.saveAttendanceFailed', { error: String(e) })
        }
      }

      onComplete(formAttendanceMode.value)
    } catch (e) {
      console.error('保存引导设置失败:', e)
      // 关键路径失败：不 onComplete，让弹窗保持打开，用户可重试
      saveError.value = t('onboarding.saveFailed', { error: String(e) })
    } finally {
      isSaving.value = false
    }
  }

  return {
    // 步骤状态
    currentStepIndex,
    currentStep,
    isLastStep,
    STEPS,
    // 表单数据
    formLanguage,
    formProjectDir,
    formImaginePath,
    formTpCliPath,
    formTpGuiPath,
    formAttendanceMode,
    imagineAutoDetected,
    tpCliAutoDetected,
    tpGuiAutoDetected,
    // 校验 & 导航
    canProceed,
    goNext,
    goPrev,
    setLanguage,
    // 路径选择
    selectProjectDir,
    selectImaginePath,
    selectTpCliPath,
    selectTpGuiPath,
    // 完成 + 保存状态
    finish,
    isSaving,
    saveError,
  }
}
