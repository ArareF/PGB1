<script setup lang="ts">
import { ref, onMounted, onUnmounted, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { useI18n } from 'vue-i18n'
import { useNavigation } from '../composables/useNavigation'
import { useSettings } from '../composables/useSettings'
import { useTheme } from '../composables/useTheme'
import { useScale } from '../composables/useScale'
import type { AppSettings } from '../composables/useSettings'
import { reloadConfig as reloadStatusBarConfig } from '../composables/useStatusBar'
import { APP_NAME, APP_VERSION, APP_DEVELOPER } from '../config/app'
import { useUpdater } from '../composables/useUpdater'
import PageGuideOverlay from '../components/PageGuideOverlay.vue'
import { PAGE_GUIDE_ANNOTATIONS } from '../config/onboarding'

const { checking, checkResult, manualCheck } = useUpdater()

const route = useRoute()
const router = useRouter()
const { t, locale } = useI18n()
const { setNavigation } = useNavigation()
const { loadSettings, saveSettings, pickFile, pickDir } = useSettings()
const { theme, toggleTheme } = useTheme()
const { setManualScale } = useScale()

// 直接读写 localStorage，避免在页面级组件里调用单例 composable 导致 refCount 异常
const STATUS_BAR_CONFIG_KEY = 'status_bar_config'
const statusBarConfig = ref<{ showTime: boolean; showDate: boolean; showWorked: boolean; showCountdown: boolean }>((() => {
  try {
    const raw = localStorage.getItem(STATUS_BAR_CONFIG_KEY)
    if (raw) return JSON.parse(raw)
  } catch {}
  return { showTime: true, showDate: true, showWorked: true, showCountdown: true }
})())

type TabId = 'workflow' | 'translation' | 'attendance' | 'general' | 'about'
const activeTab = ref<TabId>('workflow')

/** 本地编辑中的副本 */
const editSettings = ref<AppSettings | null>(null)
const isDirty = ref(false)
const saving = ref(false)
const saveError = ref('')

// ─── 日报打卡状态 ───────────────────────────────────────────
const clockInTime = ref('09:50')
const clockOutTime = ref('19:00')
const lunchStartTime = ref('')
const lunchEndTime = ref('')
const attendanceUrl = ref('')
const dailyReportTime = ref('18:30')
const dailyReportUrl = ref('')
const attendanceUsername = ref('')
const attendancePassword = ref('')
const showPassword = ref(false)
const attendanceSaving = ref(false)
const attendanceError = ref('')
const attendanceSaved = ref(false)
const attendanceDirty = ref(false)
const attendanceMode = ref<'off' | 'auto' | 'record_only'>('auto')
const dailyReportEnabled = ref(true)
let initialPassword = ''
let initialUsername = ''

// ─── 测试打卡状态 ───────────────────────────────────────────
const clockTesting = ref(false)
const showGuide = ref(false)
const guideIsAttendance = ref(false)
const clockTestStep = ref('')
const clockTestMessage = ref('')
const clockTestResult = ref<'success' | 'error' | ''>('')
let unlistenTest: UnlistenFn | null = null

async function init() {
  // 加载 AppSettings
  const current = await loadSettings()
  if (current) {
    editSettings.value = JSON.parse(JSON.stringify(current))
  }

  // 加载日报打卡配置
  try {
    const config = await invoke<{
      mode?: string
      attendance: { clock_in_time: string; clock_out_time: string; url: string; lunch_start_time?: string; lunch_end_time?: string }
      daily_report: { enabled?: boolean; time: string; url: string }
      username: string
    }>('load_attendance_config')
    attendanceMode.value = (config.mode ?? 'auto') as 'off' | 'auto' | 'record_only'
    dailyReportEnabled.value = config.daily_report.enabled ?? true
    clockInTime.value = config.attendance.clock_in_time
    clockOutTime.value = config.attendance.clock_out_time
    attendanceUrl.value = config.attendance.url
    lunchStartTime.value = config.attendance.lunch_start_time ?? ''
    lunchEndTime.value = config.attendance.lunch_end_time ?? ''
    dailyReportTime.value = config.daily_report.time
    dailyReportUrl.value = config.daily_report.url
    attendanceUsername.value = config.username
    initialUsername = config.username
    if (config.username) {
      const savedPwd = await invoke<string>('load_attendance_password', { username: config.username })
      attendancePassword.value = savedPwd
      initialPassword = savedPwd
    }
  } catch (e) {
    console.error('加载日报打卡配置失败:', e)
  }

  // 从路由 query 读取初始 tab（更多菜单跳转时传入）
  if (route.query.tab === 'attendance') {
    activeTab.value = 'attendance'
  }

  // 新手引导完成后跳转过来，自动弹出出勤配置指引
  if (route.query.guide === 'attendance') {
    guideIsAttendance.value = true
    showGuide.value = true
  }

  setNavigation({
    title: t('settings.title'),
    showBackButton: true,
    onBack: () => router.back(),
    moreMenuItems: [
      { id: 'page-guide', label: t('common.pageGuide'), handler: () => { showGuide.value = true } },
    ],
  })
}

onMounted(async () => {
  await init()
  // 监听测试打卡进度
  unlistenTest = await listen<{ step: string; message: string }>('clock-test-progress', (event) => {
    const { step, message } = event.payload
    clockTestStep.value = step
    clockTestMessage.value = message
    if (step === 'success') {
      clockTestResult.value = 'success'
      clockTesting.value = false
    } else if (step === 'error') {
      clockTestResult.value = 'error'
      clockTesting.value = false
    }
  })
})

onUnmounted(() => {
  if (unlistenTest) unlistenTest()
})

async function handleTestClock() {
  clockTesting.value = true
  clockTestResult.value = ''
  clockTestStep.value = ''
  clockTestMessage.value = t('settings.startingTest')
  try {
    await invoke('test_clock_action')
  } catch (e) {
    clockTestMessage.value = String(e)
    clockTestResult.value = 'error'
    clockTesting.value = false
  }
}

async function handleTestDailyReminder() {
  attendanceError.value = ''
  try {
    await invoke('test_reminder', { reminderType: 'daily-report' })
  } catch (e) {
    attendanceError.value = String(e)
  }
}

/** 监听 AppSettings 变化标记为脏 */
watch(editSettings, () => {
  isDirty.value = true
}, { deep: true })

/** 监听出勤字段变化标记为脏 */
watch([clockInTime, clockOutTime, attendanceUrl, lunchStartTime, lunchEndTime,
       dailyReportTime, dailyReportUrl, attendanceUsername, attendancePassword,
       attendanceMode, dailyReportEnabled], () => {
  attendanceDirty.value = true
  attendanceSaved.value = false
})


async function handleSave() {
  if (!editSettings.value) return
  saving.value = true
  saveError.value = ''
  try {
    await saveSettings(editSettings.value)
    isDirty.value = false
  } catch (e) {
    saveError.value = String(e)
    console.error('保存设置失败:', e)
  } finally {
    saving.value = false
  }
}

async function handleAttendanceSave() {
  attendanceSaving.value = true
  attendanceError.value = ''

  // 保存前 trim，防止误输入空格等
  attendanceUsername.value = attendanceUsername.value.trim()

  // 简单校验邮箱格式
  if (attendanceUsername.value && !attendanceUsername.value.includes('@')) {
    attendanceError.value = t('settings.emailFormatError')
    attendanceSaving.value = false
    return
  }

  try {
    await invoke('save_attendance_config', {
      config: {
        mode: attendanceMode.value,
        attendance: {
          clock_in_time: clockInTime.value,
          clock_out_time: clockOutTime.value,
          url: attendanceUrl.value.trim(),
          lunch_start_time: lunchStartTime.value || null,
          lunch_end_time: lunchEndTime.value || null,
        },
        daily_report: {
          enabled: dailyReportEnabled.value,
          time: dailyReportTime.value,
          url: dailyReportUrl.value.trim(),
        },
        username: attendanceUsername.value,
      },
    })
    if (attendancePassword.value !== initialPassword || attendanceUsername.value !== initialUsername) {
      if (attendanceUsername.value && attendancePassword.value) {
        await invoke('save_attendance_password', {
          username: attendanceUsername.value,
          password: attendancePassword.value,
        })
      }
    }
    await invoke('reschedule_attendance')
    localStorage.setItem(STATUS_BAR_CONFIG_KEY, JSON.stringify(statusBarConfig.value))
    reloadStatusBarConfig()
    initialPassword = attendancePassword.value
    initialUsername = attendanceUsername.value
    attendanceDirty.value = false
    attendanceSaved.value = true
    setTimeout(() => { attendanceSaved.value = false }, 2000)
  } catch (e) {
    attendanceError.value = String(e)
  } finally {
    attendanceSaving.value = false
  }
}

async function browseFile(field: 'imaginePath' | 'texturePackerCliPath' | 'texturePackerGuiPath') {
  if (!editSettings.value) return
  const path = await pickFile(t('settings.selectExecutable'), [{ name: 'Executable', extensions: ['exe'] }])
  if (path) editSettings.value.workflow[field] = path
}

async function browseDir() {
  if (!editSettings.value) return
  const path = await pickDir(t('settings.selectProjectRootDir'))
  if (path) editSettings.value.general.projectRootDir = path
}

async function onScaleChange(e: Event) {
  const val = parseFloat((e.target as HTMLSelectElement).value)
  editSettings.value!.general.uiScale = val
  setManualScale(val)
  // 缩放是运行时偏好，立即持久化，无需手动点保存
  await saveSettings(editSettings.value!)
  isDirty.value = false
}

async function onLanguageChange(e: Event) {
  const val = (e.target as HTMLSelectElement).value as 'zh-CN' | 'en'
  locale.value = val
  editSettings.value!.general.language = val
  // 语言是运行时偏好，立即持久化
  await saveSettings(editSettings.value!)
  isDirty.value = false
  // 刷新导航标题
  setNavigation({
    title: t('settings.title'),
    showBackButton: true,
    onBack: () => router.back(),
    moreMenuItems: [
      { id: 'page-guide', label: t('common.pageGuide'), handler: () => { showGuide.value = true } },
    ],
  })
}
</script>

<template>
  <div class="settings-page">
    <div v-if="editSettings" class="settings-container">
      <!-- 左侧 Tab 导航 -->
      <aside class="settings-sidebar">
        <button class="tab-btn" :class="{ active: activeTab === 'workflow' }" @click="activeTab = 'workflow'">
          {{ $t('settings.workflowTab') }}
        </button>
        <button class="tab-btn" :class="{ active: activeTab === 'translation' }" @click="activeTab = 'translation'">
          {{ $t('settings.translationTab') }}
        </button>
        <button class="tab-btn" :class="{ active: activeTab === 'attendance' }" @click="activeTab = 'attendance'">
          {{ $t('settings.attendanceTab') }}
        </button>
        <button class="tab-btn" :class="{ active: activeTab === 'general' }" @click="activeTab = 'general'">
          {{ $t('settings.generalTab') }}
        </button>
        <button class="tab-btn" :class="{ active: activeTab === 'about' }" @click="activeTab = 'about'">
          {{ $t('settings.aboutTab') }}
        </button>

        <div class="sidebar-footer">
          <!-- 日报打卡 Tab 有独立保存按钮；关于 Tab 无按钮；其他 Tab 共用保存按钮 -->
          <p v-if="saveError && activeTab !== 'attendance' && activeTab !== 'about'" class="error-text">{{ saveError }}</p>
          <button
            v-if="activeTab !== 'attendance' && activeTab !== 'about'"
            class="save-btn"
            :disabled="!isDirty || saving"
            @click="handleSave"
          >
            {{ saving ? $t('common.saving') : $t('common.saveChanges') }}
          </button>
          <button
            v-else
            class="save-btn"
            :class="{ 'save-btn-success': attendanceSaved }"
            :disabled="!attendanceDirty || attendanceSaving"
            @click="handleAttendanceSave"
          >
            {{ attendanceSaving ? $t('common.saving') : attendanceSaved ? $t('common.saved') : $t('common.saveChanges') }}
          </button>
        </div>
      </aside>

      <!-- 右侧内容区 -->
      <main class="settings-content">
        <!-- 工作流设置 -->
        <div v-if="activeTab === 'workflow'" class="settings-section">
          <h2 class="section-title">{{ $t('settings.workflowTitle') }}</h2>

          <div class="form-group">
            <label class="form-label">{{ $t('settings.imaginePath') }}</label>
            <div class="path-input-group">
              <input v-model="editSettings.workflow.imaginePath" type="text" class="form-input" :placeholder="$t('settings.imaginePathPlaceholder')" />
              <button class="browse-btn" @click="browseFile('imaginePath')">{{ $t('common.browse') }}</button>
            </div>
            <p class="form-hint">{{ $t('settings.imaginePathHint') }}</p>
          </div>

          <div class="form-group">
            <label class="form-label">{{ $t('settings.tpCliPath') }}</label>
            <div class="path-input-group">
              <input v-model="editSettings.workflow.texturePackerCliPath" type="text" class="form-input" :placeholder="$t('settings.tpCliPathPlaceholder')" />
              <button class="browse-btn" @click="browseFile('texturePackerCliPath')">{{ $t('common.browse') }}</button>
            </div>
          </div>

          <div class="form-group">
            <label class="form-label">{{ $t('settings.tpGuiPath') }}</label>
            <div class="path-input-group">
              <input v-model="editSettings.workflow.texturePackerGuiPath" type="text" class="form-input" :placeholder="$t('settings.tpGuiPathPlaceholder')" />
              <button class="browse-btn" @click="browseFile('texturePackerGuiPath')">{{ $t('common.browse') }}</button>
            </div>
          </div>
        </div>

        <!-- 翻译设置 -->
        <div v-if="activeTab === 'translation'" class="settings-section">
          <h2 class="section-title">{{ $t('settings.translationTitle') }}</h2>

          <div class="form-group">
            <label class="form-label">{{ $t('settings.apiKey') }}</label>
            <input v-model="editSettings.translation.apiKey" type="password" class="form-input" :placeholder="$t('settings.apiKeyPlaceholder')" />
          </div>

          <div class="form-group">
            <label class="form-label">{{ $t('settings.aiModel') }}</label>
            <select v-model="editSettings.translation.model" class="form-select">
              <option value="gemini-2.5-flash">Gemini 2.5 Flash ({{ $t('settings.modelFlashRecommended') }})</option>
              <option value="gemini-2.5-flash-lite">Gemini 2.5 Flash Lite ({{ $t('settings.modelFlashLiteFaster') }})</option>
              <option value="gemini-3-flash-preview">Gemini 3 Flash ({{ $t('settings.modelFlashPreview') }})</option>
              <option value="gemini-2.5-pro">Gemini 2.5 Pro ({{ $t('settings.modelProStrongest') }})</option>
            </select>
          </div>

          <div class="form-group">
            <label class="form-label">{{ $t('settings.globalShortcut') }}</label>
            <input v-model="editSettings.translation.shortcut" type="text" class="form-input" :placeholder="$t('settings.shortcutPlaceholder')" />
          </div>

          <div class="form-group">
            <label class="checkbox-label">
              <input type="checkbox" v-model="editSettings.translation.useCalculatorKey" />
              {{ $t('settings.interceptCalcKey') }}
            </label>
          </div>

          <div class="form-group">
            <label class="form-label">{{ $t('settings.defaultLangPair') }}</label>
            <div class="lang-pair-row">
              <select v-model="editSettings.translation.langA" class="form-select lang-select">
                <option value="zh-CN">{{ $t('settings.langChinese') }}</option>
                <option value="en">English</option>
                <option value="ja">日本語</option>
              </select>
              <span class="lang-separator">↔</span>
              <select v-model="editSettings.translation.langB" class="form-select lang-select">
                <option value="zh-CN">{{ $t('settings.langChinese') }}</option>
                <option value="en">English</option>
                <option value="ja">日本語</option>
              </select>
            </div>
          </div>
        </div>

        <!-- 日报打卡 -->
        <div v-if="activeTab === 'attendance'" class="settings-section">
          <h2 class="section-title">{{ $t('settings.attendanceTitle') }}</h2>

          <div class="attendance-group">
            <p class="attendance-group-title">{{ $t('settings.attendanceGroup') }}</p>
            <div class="form-group">
              <label class="form-label">{{ $t('settings.clockMode') }}</label>
              <div class="mode-btn-group">
                <button
                  class="mode-btn"
                  :class="{ active: attendanceMode === 'off' }"
                  @click="attendanceMode = 'off'"
                >{{ $t('settings.clockModeOff') }}</button>
                <button
                  class="mode-btn"
                  :class="{ active: attendanceMode === 'auto' }"
                  @click="attendanceMode = 'auto'"
                >{{ $t('settings.clockModeAuto') }}</button>
                <button
                  class="mode-btn"
                  :class="{ active: attendanceMode === 'record_only' }"
                  @click="attendanceMode = 'record_only'"
                >{{ $t('settings.clockModeRecordOnly') }}</button>
              </div>
              <p v-if="attendanceMode === 'record_only'" class="form-hint">
                {{ $t('settings.clockModeRecordOnlyHint') }}
              </p>
            </div>
            <div class="form-group">
              <label class="form-label">{{ $t('settings.clockInTime') }}</label>
              <input v-model="clockInTime" type="time" class="form-input form-input-time" />
            </div>
            <div class="form-group">
              <label class="form-label">{{ $t('settings.clockOutTime') }}</label>
              <input v-model="clockOutTime" type="time" class="form-input form-input-time" />
            </div>
            <div class="form-group">
              <label class="form-label">{{ $t('settings.lunchStartTime') }}</label>
              <input v-model="lunchStartTime" type="time" class="form-input form-input-time" />
            </div>
            <div class="form-group">
              <label class="form-label">{{ $t('settings.lunchEndTime') }}</label>
              <input v-model="lunchEndTime" type="time" class="form-input form-input-time" />
            </div>
            <div class="form-group" :class="{ 'form-group-disabled': attendanceMode !== 'auto' }">
              <label class="form-label">{{ $t('settings.attendanceUrl') }}</label>
              <input v-model="attendanceUrl" type="text" class="form-input" placeholder="https://timecard.example.com/login.html" />
            </div>
          </div>

          <div class="attendance-group">
            <div class="group-title-row">
              <p class="attendance-group-title">{{ $t('settings.dailyReportGroup') }}</p>
              <label class="toggle-label">
                <input type="checkbox" v-model="dailyReportEnabled" />
                {{ $t('settings.dailyReportEnabled') }}
              </label>
            </div>
            <div :class="{ 'form-group-disabled': !dailyReportEnabled }">
            <div class="form-group">
              <label class="form-label">{{ $t('settings.dailyReportTime') }}</label>
              <input v-model="dailyReportTime" type="time" class="form-input form-input-time" />
            </div>
            <div class="form-group">
              <label class="form-label">{{ $t('settings.dailyReportUrl') }}</label>
              <input v-model="dailyReportUrl" type="text" class="form-input" placeholder="https://docs.google.com/..." />
            </div>
            </div>
            <button class="test-clock-btn" @click="handleTestDailyReminder">
              {{ $t('settings.testDailyReminder') }}
            </button>
          </div>

          <div class="attendance-group" :class="{ 'form-group-disabled': attendanceMode !== 'auto' }">
            <p class="attendance-group-title">{{ $t('settings.accountGroup') }}</p>
            <div class="form-group">
              <label class="form-label">{{ $t('settings.account') }}</label>
              <input v-model="attendanceUsername" type="text" class="form-input" placeholder="your@email.com" />
            </div>
            <div class="form-group">
              <label class="form-label">{{ $t('settings.password') }}</label>
              <div class="path-input-group">
                <input v-model="attendancePassword" :type="showPassword ? 'text' : 'password'" class="form-input" placeholder="••••••••" />
                <button class="browse-btn" @click="showPassword = !showPassword">
                  {{ showPassword ? $t('settings.hidePassword') : $t('settings.showPassword') }}
                </button>
              </div>
            </div>
          </div>

          <!-- 测试打卡连接 -->
          <div class="attendance-group" :class="{ 'form-group-disabled': attendanceMode !== 'auto' }">
            <p class="attendance-group-title">{{ $t('settings.connectionTest') }}</p>
            <p class="form-hint">{{ $t('settings.connectionTestHint') }}</p>
            <button
              class="test-clock-btn"
              :disabled="clockTesting || !attendanceUrl || !attendanceUsername"
              @click="handleTestClock"
            >
              {{ clockTesting ? $t('settings.testing') : $t('settings.testConnection') }}
            </button>
            <div v-if="clockTestMessage" class="test-progress" :class="{ 'test-success': clockTestResult === 'success', 'test-error': clockTestResult === 'error' }">
              {{ clockTestMessage }}
            </div>
          </div>

          <p v-if="attendanceError" class="error-text">{{ attendanceError }}</p>
        </div>

        <!-- 通用设置 -->
        <div v-if="activeTab === 'general'" class="settings-section">
          <h2 class="section-title">{{ $t('settings.generalTitle') }}</h2>

          <div class="form-group">
            <label class="form-label">{{ $t('settings.projectRootDir') }}</label>
            <div class="path-input-group">
              <input v-model="editSettings.general.projectRootDir" type="text" class="form-input" :placeholder="$t('settings.projectRootDirPlaceholder')" />
              <button class="browse-btn" @click="browseDir">{{ $t('common.browse') }}</button>
            </div>
            <p class="form-hint">{{ $t('settings.projectRootDirHint') }}</p>
          </div>

          <div class="form-group">
            <label class="form-label">{{ $t('settings.language') }}</label>
            <select
              class="form-select"
              :value="editSettings.general.language"
              @change="onLanguageChange"
            >
              <option value="zh-CN">中文</option>
              <option value="en">English</option>
            </select>
          </div>

          <div class="form-group">
            <label class="form-label">{{ $t('settings.uiTheme') }}</label>
            <div class="theme-toggle-row">
              <span class="theme-current">{{ $t('settings.themeCurrent') }}{{ theme === 'light' ? $t('settings.themeLight') : $t('settings.themeDark') }}</span>
              <button class="browse-btn" @click="toggleTheme">{{ $t('settings.themeSwitchTo') }}{{ theme === 'light' ? $t('settings.themeDark') : $t('settings.themeLight') }}</button>
            </div>
          </div>

          <div class="form-group">
            <label class="form-label">{{ $t('settings.defaultFps') }}</label>
            <div class="fps-input-row">
              <input
                v-model.number="editSettings.preview.defaultFps"
                type="number"
                min="1"
                max="120"
                class="form-input fps-input"
              />
              <span class="fps-unit">fps</span>
            </div>
            <p class="form-hint">{{ $t('settings.defaultFpsHint') }}</p>
          </div>

          <div class="form-group">
            <label class="checkbox-label">
              <input type="checkbox" v-model="editSettings.preview.backgroundTransparent" />
              {{ $t('settings.transparentBg') }}
            </label>
            <p class="form-hint">{{ $t('settings.transparentBgHint') }}</p>
          </div>

          <div class="form-group">
            <label class="checkbox-label">
              <input type="checkbox" v-model="editSettings.general.autoStart" />
              {{ $t('settings.autoStart') }}
            </label>
            <p class="form-hint">{{ $t('settings.autoStartHint') }}</p>
          </div>

          <div class="form-group">
            <label class="form-label">{{ $t('settings.uiScale') }}</label>
            <select
              class="form-select"
              :value="editSettings.general.uiScale"
              @change="onScaleChange"
            >
              <option :value="0.75">75%</option>
              <option :value="0.80">80%</option>
              <option :value="0.90">90%</option>
              <option :value="1.0">100%</option>
              <option :value="1.1">110%</option>
              <option :value="1.2">120%</option>
              <option :value="1.5">150%</option>
            </select>
          </div>
        </div>
        <!-- 关于 -->
        <div v-if="activeTab === 'about'" class="settings-section about-section">
          <h2 class="section-title">{{ $t('settings.aboutTitle') }}</h2>
          <div class="about-card">
            <div class="about-row">
              <span class="about-label">{{ $t('settings.softwareName') }}</span>
              <span class="about-value">{{ APP_NAME }}</span>
            </div>
            <div class="about-row">
              <span class="about-label">{{ $t('settings.versionLabel') }}</span>
              <span class="about-value">{{ APP_VERSION }}</span>
            </div>
            <div class="about-row">
              <span class="about-label">{{ $t('settings.developerLabel') }}</span>
              <span class="about-value">{{ APP_DEVELOPER }}</span>
            </div>
          </div>
          <button class="check-update-btn" :disabled="checking" @click="manualCheck">
            <template v-if="checking">{{ $t('update.checking') }}</template>
            <template v-else-if="checkResult === 'latest'">{{ $t('update.isLatest') }}</template>
            <template v-else-if="checkResult === 'error'">{{ $t('update.checkFailed') }}</template>
            <template v-else>{{ $t('update.checkUpdate') }}</template>
          </button>
        </div>
      </main>
    </div>
    <div v-else class="loading-state">
      {{ $t('common.loading') }}
    </div>
  </div>

  <PageGuideOverlay
    :show="showGuide"
    :annotations="guideIsAttendance ? PAGE_GUIDE_ANNOTATIONS.settingsAttendance : PAGE_GUIDE_ANNOTATIONS.settings"
    @close="showGuide = false; guideIsAttendance = false"
  />
</template>

<style scoped>
.settings-page {
  height: 100%;
  overflow: hidden;
}

.settings-container {
  display: flex;
  height: 100%;
}

/* 侧边栏 */
.settings-sidebar {
  width: 200px;
  border-right: 1px solid var(--border-medium);
  padding: var(--spacing-4);
  display: flex;
  flex-direction: column;
  gap: var(--spacing-2);
}

.tab-btn {
  height: 40px;
  padding: 0 var(--spacing-4);
  border-radius: var(--radius-md);
  border: none;
  background: transparent;
  color: var(--text-secondary);
  text-align: left;
  font-weight: var(--font-medium);
  cursor: pointer;
  transition: all var(--duration-fast);
}

.tab-btn:hover {
  background: var(--bg-hover);
  color: var(--text-primary);
}

.tab-btn.active {
  background: var(--color-primary-500);
  color: white;
}

.sidebar-footer {
  margin-top: auto;
  padding-top: var(--spacing-4);
}

.save-btn {
  width: 100%;
  height: 40px;
  border-radius: var(--radius-md);
  border: none;
  background: var(--color-success);
  color: white;
  font-weight: var(--font-bold);
  cursor: pointer;
  transition: all var(--duration-fast);
}

.save-btn:hover:not(:disabled) {
  background: var(--color-success-dark);
}

.save-btn:disabled {
  opacity: 0.5;
  cursor: not-allowed;
  background: var(--color-neutral-400);
}

/* 保存成功短暂高亮 */
.save-btn-success {
  background: var(--color-success) !important;
}

/* 内容区 */
.settings-content {
  flex: 1;
  padding: var(--spacing-8);
  overflow-y: auto;
}

.settings-section {
  max-width: 600px;
  display: flex;
  flex-direction: column;
  gap: var(--spacing-6);
}

.section-title {
  font-size: var(--text-2xl);
  font-weight: var(--font-bold);
  color: var(--text-primary);
  margin-bottom: var(--spacing-2);
}

/* 日报打卡分组 */
.attendance-group {
  display: flex;
  flex-direction: column;
  gap: var(--spacing-3);
  padding-bottom: var(--spacing-4);
  border-bottom: 1px solid var(--border-medium);
}

.attendance-group:last-of-type {
  border-bottom: none;
}

.attendance-group-title {
  font-size: var(--text-base);
  font-weight: var(--font-bold);
  color: var(--text-primary);
}

/* 表单组件 */
.form-group {
  display: flex;
  flex-direction: column;
  gap: var(--spacing-2);
}

.form-label {
  font-size: var(--text-sm);
  font-weight: var(--font-bold);
  color: var(--text-secondary);
}

.form-input, .form-select {
  height: 40px;
  padding: 0 var(--spacing-3);
  border-radius: var(--radius-md);
  border: 1px solid var(--border-heavy);
  background: var(--bg-tertiary);
  color: var(--text-primary);
  font-size: var(--text-sm);
}

.form-input:focus {
  outline: none;
  border-color: var(--color-primary-500);
}

.form-input-time {
  width: 160px;
}

.form-hint {
  font-size: var(--text-xs);
  color: var(--text-tertiary);
}

.path-input-group {
  display: flex;
  gap: var(--spacing-2);
}

.path-input-group .form-input {
  flex: 1;
}

.browse-btn {
  height: 40px;
  padding: 0 var(--spacing-4);
  border-radius: var(--radius-md);
  border: 1px solid var(--border-medium);
  background: var(--bg-secondary);
  color: var(--text-primary);
  cursor: pointer;
  white-space: nowrap;
}

.browse-btn:hover {
  background: var(--bg-hover);
}

.theme-toggle-row {
  display: flex;
  align-items: center;
  gap: var(--spacing-4);
}

.theme-current {
  font-size: var(--text-sm);
  color: var(--text-secondary);
}

.checkbox-label {
  display: flex;
  align-items: center;
  gap: var(--spacing-3);
  cursor: pointer;
  font-size: var(--text-sm);
  font-weight: var(--font-medium);
}

.error-text {
  font-size: var(--text-sm);
  color: var(--color-danger);
}

.loading-state {
  display: flex;
  align-items: center;
  justify-content: center;
  height: 100%;
  color: var(--text-tertiary);
}

.lang-pair-row {
  display: flex;
  align-items: center;
  gap: var(--space-2);
}
.lang-select {
  flex: 1;
}
.lang-separator {
  color: var(--text-secondary);
  font-size: var(--text-lg);
  flex-shrink: 0;
}

.fps-input-row {
  display: flex;
  align-items: center;
  gap: var(--spacing-2);
}

.fps-input {
  width: 100px;
}

.fps-unit {
  font-size: var(--text-sm);
  color: var(--text-secondary);
}

/* 关于页 */
.about-section {
  justify-content: flex-start;
}

.about-card {
  display: flex;
  flex-direction: column;
  gap: 0;
  border: 1px solid var(--border-medium);
  border-radius: var(--radius-lg);
  overflow: hidden;
}

.about-row {
  display: flex;
  align-items: center;
  padding: var(--spacing-4) var(--spacing-6);
  border-bottom: 1px solid var(--border-medium);
}

.about-row:last-child {
  border-bottom: none;
}

.about-label {
  width: 100px;
  font-size: var(--text-sm);
  font-weight: var(--font-bold);
  color: var(--text-secondary);
  flex-shrink: 0;
}

.about-value {
  font-size: var(--text-sm);
  color: var(--text-primary);
}

.check-update-btn {
  margin-top: var(--spacing-4);
  padding: var(--spacing-2) var(--spacing-5);
  border-radius: var(--radius-button);
  border: 1px solid var(--border-medium);
  background: transparent;
  color: var(--text-secondary);
  font-size: var(--text-sm);
  font-family: inherit;
  cursor: pointer;
  transition: all var(--duration-fast);
}
.check-update-btn:hover:not(:disabled) {
  background: var(--bg-hover);
  color: var(--text-primary);
  border-color: var(--color-primary);
}
.check-update-btn:disabled {
  opacity: 0.6;
  cursor: default;
}

/* 测试打卡按钮 */
.test-clock-btn {
  height: 40px;
  padding: 0 var(--spacing-5);
  border-radius: var(--radius-md);
  border: 1px solid var(--color-primary-500);
  background: transparent;
  color: var(--color-primary-500);
  font-weight: var(--font-bold);
  cursor: pointer;
  transition: all var(--duration-fast);
  align-self: flex-start;
}

.test-clock-btn:hover:not(:disabled) {
  background: var(--color-primary-500);
  color: white;
}

.test-clock-btn:disabled {
  opacity: 0.4;
  cursor: not-allowed;
}

.test-progress {
  font-size: var(--text-sm);
  color: var(--text-secondary);
  padding: var(--spacing-2) var(--spacing-3);
  border-radius: var(--radius-sm);
  background: var(--bg-tertiary);
}

.test-progress.test-success {
  color: var(--color-success);
}

.test-progress.test-error {
  color: var(--color-danger);
}

/* 打卡模式三段按钮 */
.mode-btn-group {
  display: flex;
  gap: var(--spacing-1);
}

.mode-btn {
  flex: 1;
  height: var(--button-height);
  font-size: var(--text-sm);
  font-weight: var(--font-weight-heading);
  color: var(--text-secondary);
  background: var(--bg-hover);
  border: 1px solid var(--border-light);
  border-radius: var(--radius-sm);
  cursor: pointer;
  transition: all var(--transition-fast);
}

.mode-btn:hover {
  color: var(--text-primary);
  border-color: var(--border-medium);
}

.mode-btn.active {
  color: var(--color-primary-300);
  background: color-mix(in srgb, var(--color-primary-500) 15%, transparent);
  border-color: color-mix(in srgb, var(--color-primary-500) 40%, transparent);
}

/* 日报 group 标题行（标题 + toggle 同行） */
.group-title-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: var(--spacing-3);
}

.group-title-row .attendance-group-title {
  margin-bottom: 0;
}

.toggle-label {
  display: flex;
  align-items: center;
  gap: var(--spacing-2);
  font-size: var(--text-sm);
  color: var(--text-secondary);
  cursor: pointer;
  user-select: none;
}

/* 灰化状态（mode 不适用时） */
.form-group-disabled {
  opacity: 0.35;
  pointer-events: none;
}
</style>