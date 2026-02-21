<script setup lang="ts">
import { ref, onMounted, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { invoke } from '@tauri-apps/api/core'
import { useNavigation } from '../composables/useNavigation'
import { useSettings } from '../composables/useSettings'
import { useTheme } from '../composables/useTheme'
import { useScale } from '../composables/useScale'
import type { AppSettings } from '../composables/useSettings'
import { reloadConfig as reloadStatusBarConfig } from '../composables/useStatusBar'
import { APP_NAME, APP_VERSION, APP_DEVELOPER } from '../config/app'

const route = useRoute()
const router = useRouter()
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
let initialPassword = ''
let initialUsername = ''

async function init() {
  // 加载 AppSettings
  const current = await loadSettings()
  if (current) {
    editSettings.value = JSON.parse(JSON.stringify(current))
  }

  // 加载日报打卡配置
  try {
    const config = await invoke<{
      attendance: { clock_in_time: string; clock_out_time: string; url: string; lunch_start_time?: string; lunch_end_time?: string }
      daily_report: { time: string; url: string }
      username: string
    }>('load_attendance_config')
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

  setNavigation({
    title: '程序设置',
    showBackButton: true,
    onBack: () => router.back(),
  })
}

onMounted(init)

/** 监听 AppSettings 变化标记为脏 */
watch(editSettings, () => {
  isDirty.value = true
}, { deep: true })

/** 监听出勤字段变化标记为脏 */
watch([clockInTime, clockOutTime, attendanceUrl, lunchStartTime, lunchEndTime, dailyReportTime, dailyReportUrl, attendanceUsername, attendancePassword], () => {
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
    attendanceError.value = '账号格式不正确，请输入有效的邮箱地址'
    attendanceSaving.value = false
    return
  }

  try {
    await invoke('save_attendance_config', {
      config: {
        attendance: {
          clock_in_time: clockInTime.value,
          clock_out_time: clockOutTime.value,
          url: attendanceUrl.value.trim(),
          lunch_start_time: lunchStartTime.value || null,
          lunch_end_time: lunchEndTime.value || null,
        },
        daily_report: {
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
  const path = await pickFile('选择可执行文件', [{ name: 'Executable', extensions: ['exe'] }])
  if (path) editSettings.value.workflow[field] = path
}

async function browseDir() {
  if (!editSettings.value) return
  const path = await pickDir('选择项目根目录')
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
</script>

<template>
  <div class="settings-page">
    <div v-if="editSettings" class="settings-container">
      <!-- 左侧 Tab 导航 -->
      <aside class="settings-sidebar">
        <button class="tab-btn" :class="{ active: activeTab === 'workflow' }" @click="activeTab = 'workflow'">
          工作流设置
        </button>
        <button class="tab-btn" :class="{ active: activeTab === 'translation' }" @click="activeTab = 'translation'">
          翻译设置
        </button>
        <button class="tab-btn" :class="{ active: activeTab === 'attendance' }" @click="activeTab = 'attendance'">
          日报打卡
        </button>
        <button class="tab-btn" :class="{ active: activeTab === 'general' }" @click="activeTab = 'general'">
          通用设置
        </button>
        <button class="tab-btn" :class="{ active: activeTab === 'about' }" @click="activeTab = 'about'">
          关于
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
            {{ saving ? '正在保存...' : '保存修改' }}
          </button>
          <button
            v-else
            class="save-btn"
            :class="{ 'save-btn-success': attendanceSaved }"
            :disabled="!attendanceDirty || attendanceSaving"
            @click="handleAttendanceSave"
          >
            {{ attendanceSaving ? '正在保存...' : attendanceSaved ? '已保存 ✓' : '保存修改' }}
          </button>
        </div>
      </aside>

      <!-- 右侧内容区 -->
      <main class="settings-content">
        <!-- 工作流设置 -->
        <div v-if="activeTab === 'workflow'" class="settings-section">
          <h2 class="section-title">工作流设置</h2>

          <div class="form-group">
            <label class="form-label">Imagine 路径</label>
            <div class="path-input-group">
              <input v-model="editSettings.workflow.imaginePath" type="text" class="form-input" placeholder="请选择 imagine.exe" />
              <button class="browse-btn" @click="browseFile('imaginePath')">浏览...</button>
            </div>
            <p class="form-hint">用于静帧转换 webp</p>
          </div>

          <div class="form-group">
            <label class="form-label">TexturePacker CLI 路径</label>
            <div class="path-input-group">
              <input v-model="editSettings.workflow.texturePackerCliPath" type="text" class="form-input" placeholder="请选择 TexturePacker.exe" />
              <button class="browse-btn" @click="browseFile('texturePackerCliPath')">浏览...</button>
            </div>
          </div>

          <div class="form-group">
            <label class="form-label">TexturePacker GUI 路径</label>
            <div class="path-input-group">
              <input v-model="editSettings.workflow.texturePackerGuiPath" type="text" class="form-input" placeholder="通常与 CLI 路径相同" />
              <button class="browse-btn" @click="browseFile('texturePackerGuiPath')">浏览...</button>
            </div>
          </div>
        </div>

        <!-- 翻译设置 -->
        <div v-if="activeTab === 'translation'" class="settings-section">
          <h2 class="section-title">翻译设置</h2>

          <div class="form-group">
            <label class="form-label">Gemini API Key</label>
            <input v-model="editSettings.translation.apiKey" type="password" class="form-input" placeholder="粘贴您的 API Key" />
          </div>

          <div class="form-group">
            <label class="form-label">AI 模型</label>
            <select v-model="editSettings.translation.model" class="form-select">
              <option value="gemini-2.5-flash">Gemini 2.5 Flash (推荐)</option>
              <option value="gemini-2.5-flash-lite">Gemini 2.5 Flash Lite (更快更省)</option>
              <option value="gemini-3-flash-preview">Gemini 3 Flash (最新预览)</option>
              <option value="gemini-2.5-pro">Gemini 2.5 Pro (最强)</option>
            </select>
          </div>

          <div class="form-group">
            <label class="form-label">全局快捷键</label>
            <input v-model="editSettings.translation.shortcut" type="text" class="form-input" placeholder="例如 Control+Shift+T" />
          </div>

          <div class="form-group">
            <label class="checkbox-label">
              <input type="checkbox" v-model="editSettings.translation.useCalculatorKey" />
              拦截计算器键 (Calculator Key) 呼出翻译
            </label>
          </div>

          <div class="form-group">
            <label class="form-label">默认语言对</label>
            <div class="lang-pair-row">
              <select v-model="editSettings.translation.langA" class="form-select lang-select">
                <option value="zh-CN">中文</option>
                <option value="en">English</option>
                <option value="ja">日本語</option>
              </select>
              <span class="lang-separator">↔</span>
              <select v-model="editSettings.translation.langB" class="form-select lang-select">
                <option value="zh-CN">中文</option>
                <option value="en">English</option>
                <option value="ja">日本語</option>
              </select>
            </div>
          </div>
        </div>

        <!-- 日报打卡 -->
        <div v-if="activeTab === 'attendance'" class="settings-section">
          <h2 class="section-title">日报打卡</h2>

          <div class="attendance-group">
            <p class="attendance-group-title">考勤设置</p>
            <div class="form-group">
              <label class="form-label">出勤提醒时间</label>
              <input v-model="clockInTime" type="time" class="form-input form-input-time" />
            </div>
            <div class="form-group">
              <label class="form-label">退勤提醒时间</label>
              <input v-model="clockOutTime" type="time" class="form-input form-input-time" />
            </div>
            <div class="form-group">
              <label class="form-label">午休开始时间</label>
              <input v-model="lunchStartTime" type="time" class="form-input form-input-time" />
            </div>
            <div class="form-group">
              <label class="form-label">午休结束时间</label>
              <input v-model="lunchEndTime" type="time" class="form-input form-input-time" />
            </div>
            <div class="form-group">
              <label class="form-label">打卡网站 URL</label>
              <input v-model="attendanceUrl" type="text" class="form-input" placeholder="https://timecard.example.com/login.html" />
            </div>
          </div>

          <div class="attendance-group">
            <p class="attendance-group-title">日报设置</p>
            <div class="form-group">
              <label class="form-label">日报提醒时间</label>
              <input v-model="dailyReportTime" type="time" class="form-input form-input-time" />
            </div>
            <div class="form-group">
              <label class="form-label">日报网站 URL</label>
              <input v-model="dailyReportUrl" type="text" class="form-input" placeholder="https://docs.google.com/..." />
            </div>
          </div>

          <div class="attendance-group">
            <p class="attendance-group-title">账号设置</p>
            <div class="form-group">
              <label class="form-label">账号</label>
              <input v-model="attendanceUsername" type="text" class="form-input" placeholder="your@email.com" />
            </div>
            <div class="form-group">
              <label class="form-label">密码</label>
              <div class="path-input-group">
                <input v-model="attendancePassword" :type="showPassword ? 'text' : 'password'" class="form-input" placeholder="••••••••" />
                <button class="browse-btn" @click="showPassword = !showPassword">
                  {{ showPassword ? '隐藏' : '显示' }}
                </button>
              </div>
            </div>
          </div>

          <p v-if="attendanceError" class="error-text">{{ attendanceError }}</p>
        </div>

        <!-- 通用设置 -->
        <div v-if="activeTab === 'general'" class="settings-section">
          <h2 class="section-title">通用设置</h2>

          <div class="form-group">
            <label class="form-label">项目根目录</label>
            <div class="path-input-group">
              <input v-model="editSettings.general.projectRootDir" type="text" class="form-input" placeholder="选择存放所有项目的文件夹" />
              <button class="browse-btn" @click="browseDir">浏览...</button>
            </div>
            <p class="form-hint">软件将扫描此目录下的文件夹作为项目</p>
          </div>

          <div class="form-group">
            <label class="form-label">界面主题</label>
            <div class="theme-toggle-row">
              <span class="theme-current">当前：{{ theme === 'light' ? '浅色' : '深色' }}</span>
              <button class="browse-btn" @click="toggleTheme">切换为{{ theme === 'light' ? '深色' : '浅色' }}</button>
            </div>
          </div>

          <div class="form-group">
            <label class="form-label">序列帧默认帧率</label>
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
            <p class="form-hint">序列帧未标注帧率时使用此值播放预览动画</p>
          </div>

          <div class="form-group">
            <label class="checkbox-label">
              <input type="checkbox" v-model="editSettings.preview.backgroundTransparent" />
              序列帧预览使用透明背景
            </label>
            <p class="form-hint">关闭时使用黑色背景，开启时显示棋盘格（透明）</p>
          </div>

          <div class="form-group">
            <label class="checkbox-label">
              <input type="checkbox" v-model="editSettings.general.autoStart" />
              开机自动启动
            </label>
            <p class="form-hint">将应用添加到 Windows 启动项，开机后自动运行</p>
          </div>

          <div class="form-group">
            <label class="form-label">UI 缩放</label>
            <select
              class="form-select"
              :value="editSettings.general.uiScale"
              @change="onScaleChange"
            >
              <option :value="0">自动（跟随窗口）</option>
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
          <h2 class="section-title">关于本软件</h2>
          <div class="about-card">
            <div class="about-row">
              <span class="about-label">软件名称</span>
              <span class="about-value">{{ APP_NAME }}</span>
            </div>
            <div class="about-row">
              <span class="about-label">版本号</span>
              <span class="about-value">{{ APP_VERSION }}</span>
            </div>
            <div class="about-row">
              <span class="about-label">开发者</span>
              <span class="about-value">{{ APP_DEVELOPER }}</span>
            </div>
          </div>
        </div>
      </main>
    </div>
    <div v-else class="loading-state">
      加载中...
    </div>
  </div>
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
</style>
