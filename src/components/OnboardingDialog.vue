<script setup lang="ts">
import { ref, computed, onMounted } from 'vue'
import { useI18n } from 'vue-i18n'
import { invoke } from '@tauri-apps/api/core'
import { open } from '@tauri-apps/plugin-dialog'
import { useSettings } from '../composables/useSettings'
import type { AppSettings } from '../composables/useSettings'

defineProps<{ show: boolean }>()
const emit = defineEmits<{ complete: [mode: 'off' | 'auto' | 'record_only'] }>()

const { t, locale } = useI18n()
const { pickFile } = useSettings()

// ─── 步骤定义 ───────────────────────────────────────────
type StepId = 'language' | 'project-dir' | 'tool-paths' | 'attendance'
const STEPS: StepId[] = ['language', 'project-dir', 'tool-paths', 'attendance']
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

// 工具自动检测标记
const imagineAutoDetected = ref(false)
const tpCliAutoDetected = ref(false)
const tpGuiAutoDetected = ref(false)

// ─── 初始化：从系统已安装应用中扫描工具路径 ──────────────────────
interface AppShortcutEntry { name: string; target_path: string }

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

// ─── 导航 ───────────────────────────────────────────
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

// ─── 语言切换即时生效 ───────────────────────────────────────
function setLanguage(lang: 'zh-CN' | 'en') {
  formLanguage.value = lang
  locale.value = lang
}

// ─── 选择目录 ───────────────────────────────────────────
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

// ─── 完成引导 ───────────────────────────────────────────
async function finish() {
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

    await invoke('save_settings', { settings: current })

    // 保存打卡模式
    if (formAttendanceMode.value !== 'off') {
      try {
        const config = await invoke<Record<string, unknown>>('load_attendance_config')
        ;(config as Record<string, unknown>).mode = formAttendanceMode.value
        await invoke('save_attendance_config', { config })
      } catch (e) {
        console.error('保存打卡配置失败:', e)
      }
    }

    emit('complete', formAttendanceMode.value)
  } catch (e) {
    console.error('保存引导设置失败:', e)
    emit('complete', formAttendanceMode.value)
  }
}
</script>

<template>
  <Teleport to="body">
    <Transition name="onboarding">
      <div v-if="show" class="onboarding-overlay">
        <div class="onboarding-dialog glass-strong">
          <!-- 步骤内容 -->
          <div class="step-content">
            <!-- Step 1: 语言选择 -->
            <div v-if="currentStep === 'language'" class="step-body">
              <h2 class="step-title">{{ t('onboarding.languageTitle') }}</h2>
              <p class="step-desc">{{ t('onboarding.languageDesc') }}</p>
              <div class="language-options">
                <button
                  class="lang-btn"
                  :class="{ active: formLanguage === 'zh-CN' }"
                  @click="setLanguage('zh-CN')"
                >
                  简体中文
                </button>
                <button
                  class="lang-btn"
                  :class="{ active: formLanguage === 'en' }"
                  @click="setLanguage('en')"
                >
                  English
                </button>
              </div>
            </div>

            <!-- Step 2: 项目目录 -->
            <div v-else-if="currentStep === 'project-dir'" class="step-body">
              <h2 class="step-title">{{ t('onboarding.projectDirTitle') }}</h2>
              <p class="step-desc">{{ t('onboarding.projectDirDesc') }}</p>
              <p class="step-hint">{{ t('onboarding.projectDirHint') }}</p>
              <div class="dir-picker">
                <span class="dir-display">{{ formProjectDir || t('onboarding.notSetYet') }}</span>
                <button class="pick-btn" @click="selectProjectDir">{{ t('onboarding.selectDir') }}</button>
              </div>
            </div>

            <!-- Step 3: 工具路径 -->
            <div v-else-if="currentStep === 'tool-paths'" class="step-body">
              <h2 class="step-title">{{ t('onboarding.toolPathsTitle') }}</h2>
              <p class="step-desc">{{ t('onboarding.toolPathsDesc') }}</p>
              <div class="tool-row">
                <label class="tool-label">Imagine</label>
                <div class="tool-picker">
                  <span class="tool-display" :class="{ detected: formImaginePath }">
                    <template v-if="formImaginePath">
                      <span v-if="imagineAutoDetected" class="detect-tag">{{ t('onboarding.autoDetected') }}</span>
                      {{ formImaginePath }}
                    </template>
                    <template v-else>{{ t('onboarding.toolNotFound') }}</template>
                  </span>
                  <button class="pick-btn" @click="selectImaginePath">{{ t('common.browse') }}</button>
                </div>
              </div>
              <div class="tool-row">
                <label class="tool-label">{{ t('settings.tpCliPath') }}</label>
                <div class="tool-picker">
                  <span class="tool-display" :class="{ detected: formTpCliPath }">
                    <template v-if="formTpCliPath">
                      <span v-if="tpCliAutoDetected" class="detect-tag">{{ t('onboarding.autoDetected') }}</span>
                      {{ formTpCliPath }}
                    </template>
                    <template v-else>{{ t('onboarding.toolNotFound') }}</template>
                  </span>
                  <button class="pick-btn" @click="selectTpCliPath">{{ t('common.browse') }}</button>
                </div>
              </div>
              <div class="tool-row">
                <label class="tool-label">{{ t('settings.tpGuiPath') }}</label>
                <div class="tool-picker">
                  <span class="tool-display" :class="{ detected: formTpGuiPath }">
                    <template v-if="formTpGuiPath">
                      <span v-if="tpGuiAutoDetected" class="detect-tag">{{ t('onboarding.autoDetected') }}</span>
                      {{ formTpGuiPath }}
                    </template>
                    <template v-else>{{ t('onboarding.toolNotFound') }}</template>
                  </span>
                  <button class="pick-btn" @click="selectTpGuiPath">{{ t('common.browse') }}</button>
                </div>
              </div>
            </div>

            <!-- Step 4: 打卡模式 -->
            <div v-else-if="currentStep === 'attendance'" class="step-body">
              <h2 class="step-title">{{ t('onboarding.attendanceTitle') }}</h2>
              <p class="step-desc">{{ t('onboarding.attendanceDesc') }}</p>
              <div class="attendance-options">
                <button
                  class="mode-btn"
                  :class="{ active: formAttendanceMode === 'off' }"
                  @click="formAttendanceMode = 'off'"
                >
                  {{ t('onboarding.attendanceModeOff') }}
                </button>
                <button
                  class="mode-btn"
                  :class="{ active: formAttendanceMode === 'auto' }"
                  @click="formAttendanceMode = 'auto'"
                >
                  {{ t('onboarding.attendanceModeAuto') }}
                </button>
                <button
                  class="mode-btn"
                  :class="{ active: formAttendanceMode === 'record_only' }"
                  @click="formAttendanceMode = 'record_only'"
                >
                  {{ t('onboarding.attendanceModeRecord') }}
                </button>
              </div>
            </div>
          </div>

          <!-- 底部：圆点指示器 + 按钮 -->
          <div class="step-footer">
            <div class="step-dots">
              <span
                v-for="i in STEPS.length"
                :key="i"
                class="dot"
                :class="{ active: i - 1 === currentStepIndex, visited: i - 1 < currentStepIndex }"
              />
            </div>
            <div class="step-actions">
              <button
                v-if="currentStepIndex > 0"
                class="action-btn secondary"
                @click="goPrev"
              >
                {{ t('onboarding.prev') }}
              </button>

              <button
                v-if="!isLastStep"
                class="action-btn primary"
                :disabled="!canProceed"
                @click="goNext"
              >
                {{ t('onboarding.next') }}
              </button>

              <button
                v-if="isLastStep"
                class="action-btn primary"
                @click="finish"
              >
                {{ t('onboarding.startUsing') }}
              </button>
            </div>
          </div>
        </div>
      </div>
    </Transition>
  </Teleport>
</template>

<style scoped>
.onboarding-overlay {
  position: fixed;
  inset: 0;
  z-index: var(--z-modal-backdrop);
  background: var(--overlay-backdrop);
  display: flex;
  align-items: center;
  justify-content: center;
}

.onboarding-dialog {
  width: 580px;
  max-width: 90vw;
  max-height: 85vh;
  display: flex;
  flex-direction: column;
  border-radius: var(--radius-2xl);
  overflow: hidden;
}

/* ─── 步骤内容 ───────────────────────────────────── */
.step-content {
  flex: 1;
  overflow-y: auto;
  padding: var(--spacing-8) var(--spacing-8) var(--spacing-4);
}

.step-body {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: var(--spacing-4);
  text-align: center;
}

.step-title {
  font-size: var(--text-2xl);
  font-weight: var(--font-weight-heading);
  color: var(--text-primary);
}

.step-desc {
  font-size: var(--text-base);
  color: var(--text-secondary);
  line-height: 1.6;
  max-width: 420px;
}

/* ─── 语言选择 ───────────────────────────────────── */
.language-options {
  display: flex;
  gap: var(--spacing-4);
  margin-top: var(--spacing-4);
}

.lang-btn {
  padding: var(--spacing-3) var(--spacing-8);
  border-radius: var(--radius-lg);
  border: 1px solid var(--border-medium);
  background: transparent;
  color: var(--text-secondary);
  font-size: var(--text-base);
  cursor: pointer;
  transition: var(--transition-all);
}

.lang-btn:hover {
  border-color: var(--color-primary);
  color: var(--text-primary);
}

.lang-btn.active {
  background: var(--color-primary);
  border-color: var(--color-primary);
  color: #fff;
}

/* ─── 目录选择 ───────────────────────────────────── */
.dir-picker,
.tool-picker {
  display: flex;
  align-items: center;
  gap: var(--spacing-3);
  width: 100%;
  max-width: 420px;
}

.dir-display,
.tool-display {
  flex: 1;
  text-align: left;
  padding: var(--spacing-2) var(--spacing-3);
  border-radius: var(--radius-md);
  background: var(--bg-elevated);
  border: 1px solid var(--border-light);
  color: var(--text-secondary);
  font-size: var(--text-sm);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.pick-btn {
  padding: var(--spacing-2) var(--spacing-4);
  border-radius: var(--radius-md);
  border: 1px solid var(--border-medium);
  background: transparent;
  color: var(--text-secondary);
  font-size: var(--text-sm);
  cursor: pointer;
  transition: var(--transition-all);
  white-space: nowrap;
  flex-shrink: 0;
}

.pick-btn:hover {
  border-color: var(--color-primary);
  color: var(--text-primary);
}

/* ─── 工具路径 ───────────────────────────────────── */
.tool-row {
  width: 100%;
  max-width: 420px;
  display: flex;
  flex-direction: column;
  gap: var(--spacing-2);
  align-items: flex-start;
}

.tool-label {
  font-size: var(--text-sm);
  color: var(--text-tertiary);
}

/* ─── 打卡模式 ───────────────────────────────────── */
.attendance-options {
  display: flex;
  gap: var(--spacing-3);
  margin-top: var(--spacing-4);
}

.mode-btn {
  padding: var(--spacing-3) var(--spacing-6);
  border-radius: var(--radius-lg);
  border: 1px solid var(--border-medium);
  background: transparent;
  color: var(--text-secondary);
  font-size: var(--text-sm);
  cursor: pointer;
  transition: var(--transition-all);
}

.mode-btn:hover {
  border-color: var(--color-primary);
  color: var(--text-primary);
}

.mode-btn.active {
  background: var(--color-primary);
  border-color: var(--color-primary);
  color: #fff;
}

/* ─── 提示文本 ───────────────────────────────────── */
.step-hint {
  font-size: var(--text-sm);
  color: var(--text-tertiary);
  max-width: 420px;
  margin-top: calc(-1 * var(--spacing-2));
}

/* ─── 自动检测标签 ───────────────────────────────── */
.detect-tag {
  font-size: var(--text-xs);
  color: var(--color-success, #4caf50);
  margin-right: var(--spacing-1);
}

.tool-display.detected {
  border-color: var(--color-success, #4caf50);
}

/* ─── 底部 ───────────────────────────────────── */
.step-footer {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: var(--spacing-4);
  padding: var(--spacing-4) var(--spacing-8) var(--spacing-6);
}

.step-dots {
  display: flex;
  gap: 6px;
}

.dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background: var(--border-medium);
  transition: var(--transition-all);
}

.dot.visited {
  background: var(--color-primary);
  opacity: 0.4;
}

.dot.active {
  background: var(--color-primary);
  opacity: 1;
  width: 18px;
  border-radius: 3px;
}

.step-actions {
  display: flex;
  gap: var(--spacing-3);
  align-items: center;
}

.action-btn {
  padding: var(--spacing-2) var(--spacing-6);
  border-radius: var(--radius-lg);
  font-size: var(--text-sm);
  cursor: pointer;
  transition: var(--transition-all);
  border: 1px solid transparent;
}

.action-btn.primary {
  background: var(--color-primary);
  color: #fff;
  border-color: var(--color-primary);
}

.action-btn.primary:hover:not(:disabled) {
  opacity: 0.9;
}

.action-btn.primary:disabled {
  opacity: 0.35;
  cursor: not-allowed;
}

.action-btn.secondary {
  background: transparent;
  color: var(--text-secondary);
  border-color: var(--border-medium);
}

.action-btn.secondary:hover {
  color: var(--text-primary);
  border-color: var(--border-heavy);
}

.action-btn.tertiary {
  background: transparent;
  color: var(--text-tertiary);
  border-color: transparent;
}

.action-btn.tertiary:hover {
  color: var(--text-secondary);
}

/* ─── 进出场动画 ───────────────────────────────────── */
.onboarding-enter-active {
  transition: opacity var(--duration-normal) var(--ease-out);
}
.onboarding-enter-active .onboarding-dialog {
  animation: dialog-enter var(--duration-normal) var(--ease-out);
}
.onboarding-leave-active {
  transition: opacity var(--duration-fast) var(--ease-in);
}
.onboarding-leave-active .onboarding-dialog {
  animation: dialog-leave var(--duration-fast) var(--ease-in);
}
.onboarding-enter-from,
.onboarding-leave-to {
  opacity: 0;
}
</style>
