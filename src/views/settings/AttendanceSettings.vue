<script setup lang="ts">
import { ref, watch, onMounted, onUnmounted, computed } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { useI18n } from 'vue-i18n'

const { t } = useI18n()

// ─── 出勤表单状态 ───────────────────────────────────────────
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
const attendanceMode = ref<'off' | 'auto' | 'record_only'>('auto')
const dailyReportEnabled = ref(true)
let initialPassword = ''
let initialUsername = ''

// ─── 保存状态 ───────────────────────────────────────────────
const attendanceSaving = ref(false)
const attendanceError = ref('')
const attendanceSaved = ref(false)
const attendanceDirty = ref(false)

// ─── 测试打卡状态 ───────────────────────────────────────────
const clockTesting = ref(false)
const clockTestStep = ref('')
const clockTestMessage = ref('')
const clockTestResult = ref<'success' | 'error' | ''>('')
let unlistenTest: UnlistenFn | null = null

async function init() {
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
  // init 完立即重置 dirty（init 赋值会触发 watch）
  attendanceDirty.value = false
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

// 监听出勤字段变化标记为脏
watch([clockInTime, clockOutTime, attendanceUrl, lunchStartTime, lunchEndTime,
       dailyReportTime, dailyReportUrl, attendanceUsername, attendancePassword,
       attendanceMode, dailyReportEnabled], () => {
  attendanceDirty.value = true
  attendanceSaved.value = false
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

async function save() {
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

// 暴露给父组件的最小 API（父组件 sidebar-footer 的保存按钮需要这些状态）
defineExpose({
  save,
  isDirty: computed(() => attendanceDirty.value),
  isSaving: computed(() => attendanceSaving.value),
  saved: computed(() => attendanceSaved.value),
})
</script>

<template>
  <div class="settings-section">
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
</template>
