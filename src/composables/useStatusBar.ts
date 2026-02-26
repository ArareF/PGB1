import { ref, computed, onMounted, onUnmounted } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { isPermissionGranted, requestPermission, sendNotification } from '@tauri-apps/plugin-notification'
import i18n from '../i18n'

export type CalendarRegion = 'auto' | 'CN' | 'JP' | 'none'

export interface StatusBarConfig {
  showTime: boolean
  showDate: boolean
  showWorked: boolean
  showCountdown: boolean
  showPomodoro: boolean  // 番茄钟开关
  pomodoroWork: number   // 番茄钟工作时长（分钟）
  pomodoroBreak: number  // 番茄钟休息时长（分钟）
  calendarRegion: CalendarRegion  // 节假日日历地区
}

const DEFAULT_CONFIG: StatusBarConfig = {
  showTime: true,
  showDate: true,
  showWorked: true,
  showCountdown: true,
  showPomodoro: false,
  pomodoroWork: 25,
  pomodoroBreak: 5,
  calendarRegion: 'auto',
}

const CONFIG_KEY = 'status_bar_config'

// 模块级单例
const config = ref<StatusBarConfig>(loadConfig())
const now = ref(new Date())
const clockInTime = ref<string | null>(null)
const clockOutTime = ref<string | null>(null)
const lunchStartTime = ref<string | null>(null)  // "12:00"
const lunchEndTime = ref<string | null>(null)    // "13:00"
const holidayLabel = ref<string | null>(null)
const hasClockIn = ref(false)   // 今天已执行上班打卡
const hasClockOut = ref(false)  // 今天已执行下班打卡
const actualClockInTime = ref<string | null>(null)   // 实际出勤时间 "HH:MM"
const actualClockOutTime = ref<string | null>(null)  // 实际退勤时间 "HH:MM"

// 番茄钟状态
export type PomodoroPhase = 'idle' | 'work' | 'work-done' | 'break' | 'break-done'
const pomodoroPhase = ref<PomodoroPhase>('idle')
const pomodoroEndTime = ref<number>(0)       // 倒计时目标时间戳（ms）
const pomodoroLeftSeconds = ref<number>(0)   // 剩余秒数（每秒更新）

let pomodoroTimer: ReturnType<typeof setInterval> | null = null

async function sendPomodoroNotification(title: string, body: string) {
  try {
    let granted = await isPermissionGranted()
    if (!granted) {
      const permission = await requestPermission()
      granted = permission === 'granted'
    }
    if (granted) {
      sendNotification({ title, body })
    }
  } catch { /* 通知不可用时静默 */ }
}

let timer: ReturnType<typeof setInterval> | null = null
let refCount = 0

function loadConfig(): StatusBarConfig {
  try {
    const raw = localStorage.getItem(CONFIG_KEY)
    if (raw) return { ...DEFAULT_CONFIG, ...JSON.parse(raw) }
  } catch { /* ignore */ }
  return { ...DEFAULT_CONFIG }
}

export function saveConfig() {
  localStorage.setItem(CONFIG_KEY, JSON.stringify(config.value))
}

export function reloadConfig() {
  config.value = loadConfig()
}

async function loadAttendanceRecord() {
  try {
    const record = await invoke<{
      last_clock_in: string | null
      last_clock_out: string | null
      actual_clock_in_time: string | null
      actual_clock_out_time: string | null
    }>('load_attendance_record')
    const today = dateToISO(new Date())
    hasClockIn.value = record.last_clock_in === today
    hasClockOut.value = record.last_clock_out === today
    // 仅当日期匹配时使用实际时间（跨天后清零）
    actualClockInTime.value = hasClockIn.value ? (record.actual_clock_in_time ?? null) : null
    actualClockOutTime.value = hasClockOut.value ? (record.actual_clock_out_time ?? null) : null
  } catch { /* 静默 */ }
}

async function loadAttendanceConfig() {
  try {
    const cfg = await invoke<{
      attendance: { clock_in_time: string; clock_out_time: string; lunch_start_time?: string; lunch_end_time?: string }
    }>('load_attendance_config')
    clockInTime.value = cfg.attendance.clock_in_time
    clockOutTime.value = cfg.attendance.clock_out_time
    lunchStartTime.value = cfg.attendance.lunch_start_time ?? null
    lunchEndTime.value = cfg.attendance.lunch_end_time ?? null
  } catch { /* 未配置考勤，静默 */ }
}

// ─── IP 地理位置检测 ────────────────────────────────────────
const IP_CACHE_KEY = 'holiday_ip_cache'
const IP_CACHE_TTL = 7 * 24 * 60 * 60 * 1000  // 7 天

// undefined = 未检测，null = 检测失败，string = 国家代码
let detectedCountry: string | null | undefined = undefined

async function detectCountry(): Promise<string | null> {
  try {
    const cached = localStorage.getItem(IP_CACHE_KEY)
    if (cached) {
      const { country, expires } = JSON.parse(cached)
      if (Date.now() < expires) return country
    }
    const res = await fetch('https://ipapi.co/country/')
    const country = (await res.text()).trim().toUpperCase()
    if (/^[A-Z]{2}$/.test(country)) {
      localStorage.setItem(IP_CACHE_KEY, JSON.stringify({ country, expires: Date.now() + IP_CACHE_TTL }))
      return country
    }
  } catch { /* 网络不可用时静默 */ }
  return null
}

async function getEffectiveRegion(): Promise<string> {
  if (config.value.calendarRegion !== 'auto') return config.value.calendarRegion
  if (detectedCountry === undefined) {
    detectedCountry = await detectCountry()
  }
  return detectedCountry ?? 'CN'  // 检测失败降级为中国
}

// ─── CN 节假日（timor.tech）────────────────────────────────
async function fetchTimorType(date: Date): Promise<number | null> {
  const key = `holiday_cache_${date.getFullYear()}-${String(date.getMonth()+1).padStart(2,'0')}-${String(date.getDate()).padStart(2,'0')}`
  const cached = localStorage.getItem(key)
  if (cached !== null) return JSON.parse(cached)
  try {
    const url = `https://timor.tech/api/holiday/info/${date.getFullYear()}-${date.getMonth()+1}-${date.getDate()}`
    const res = await fetch(url)
    const data = await res.json()
    const type: number | null = data?.type?.type ?? null
    localStorage.setItem(key, JSON.stringify(type))
    return type
  } catch {
    return null
  }
}

// ─── 通用节假日（date.nager.at，支持 JP 等国家）─────────────
function dateToISO(date: Date): string {
  return `${date.getFullYear()}-${String(date.getMonth()+1).padStart(2,'0')}-${String(date.getDate()).padStart(2,'0')}`
}

async function fetchNagerHolidays(year: number, countryCode: string): Promise<Set<string>> {
  const cacheKey = `holiday_nager_${countryCode}_${year}`
  const cached = localStorage.getItem(cacheKey)
  if (cached) return new Set(JSON.parse(cached))
  try {
    const res = await fetch(`https://date.nager.at/api/v3/PublicHolidays/${year}/${countryCode}`)
    if (!res.ok) return new Set()
    const data: Array<{ date: string }> = await res.json()
    const dates = data.map(h => h.date)
    localStorage.setItem(cacheKey, JSON.stringify(dates))
    return new Set(dates)
  } catch {
    return new Set()
  }
}

// ─── 统一入口 ────────────────────────────────────────────────
async function loadHoliday(date: Date) {
  const region = await getEffectiveRegion()

  if (region === 'none') {
    holidayLabel.value = null
    return
  }

  if (region === 'CN') {
    const todayType = await fetchTimorType(date)
    if (todayType === 1) { holidayLabel.value = i18n.global.t('status.holiday'); return }
    if (todayType === 2) { holidayLabel.value = i18n.global.t('status.workday'); return }
    const tomorrow = new Date(date)
    tomorrow.setDate(tomorrow.getDate() + 1)
    const tomorrowType = await fetchTimorType(tomorrow)
    holidayLabel.value = tomorrowType === 1 ? i18n.global.t('status.tomorrowOff') : null
    return
  }

  // 其他国家走 date.nager.at
  const countryCode = region.toUpperCase()
  const todayStr = dateToISO(date)
  const holidays = await fetchNagerHolidays(date.getFullYear(), countryCode)
  if (holidays.has(todayStr)) {
    holidayLabel.value = i18n.global.t('status.holiday')
    return
  }
  const tomorrow = new Date(date)
  tomorrow.setDate(tomorrow.getDate() + 1)
  const tomorrowStr = dateToISO(tomorrow)
  // 跨年时需取下一年的假日列表
  const tomorrowHolidays = tomorrow.getFullYear() !== date.getFullYear()
    ? await fetchNagerHolidays(tomorrow.getFullYear(), countryCode)
    : holidays
  holidayLabel.value = tomorrowHolidays.has(tomorrowStr) ? i18n.global.t('status.tomorrowOff') : null
}

function tick() {
  const prev = now.value
  now.value = new Date()
  if (prev.getDate() !== now.value.getDate()) {
    loadHoliday(now.value)
  }
  loadAttendanceRecord()
}

function startTimer() {
  if (timer) return
  const msToNextMinute = (60 - now.value.getSeconds()) * 1000 - now.value.getMilliseconds()
  setTimeout(() => {
    tick()
    timer = setInterval(tick, 60_000)
  }, msToNextMinute)
}

function stopTimer() {
  if (timer) {
    clearInterval(timer)
    timer = null
  }
}

function parseTimeToday(timeStr: string): Date {
  const [h, m] = timeStr.split(':').map(Number)
  const d = new Date()
  d.setHours(h, m, 0, 0)
  return d
}

export function useStatusBar() {
  onMounted(() => {
    if (refCount === 0) {
      loadAttendanceConfig()
      loadAttendanceRecord()
      loadHoliday(now.value)
      startTimer()
    }
    refCount++
  })

  onUnmounted(() => {
    refCount--
    if (refCount === 0) stopTimer()
  })

  // 午休时段判断
  const isLunch = computed(() => {
    if (!lunchStartTime.value || !lunchEndTime.value) return false
    const ls = parseTimeToday(lunchStartTime.value)
    const le = parseTimeToday(lunchEndTime.value)
    return now.value >= ls && now.value < le
  })

  // 距离午休还有多少分钟（午休前才有值）
  const toLunchMinutes = computed(() => {
    if (!lunchStartTime.value || !lunchEndTime.value) return null
    const ls = parseTimeToday(lunchStartTime.value)
    const diff = ls.getTime() - now.value.getTime()
    if (diff <= 60_000) return null  // 不足 1 分钟视为已到午休
    return Math.floor(diff / 60_000)
  })

  // 午休结束还有多少分钟
  const lunchLeftMinutes = computed(() => {
    if (!lunchEndTime.value) return null
    const le = parseTimeToday(lunchEndTime.value)
    const diff = le.getTime() - now.value.getTime()
    if (diff <= 0) return null
    return Math.floor(diff / 60_000)
  })

  // 午休总时长（分钟），用于扣除工时
  const lunchDurationMinutes = computed(() => {
    if (!lunchStartTime.value || !lunchEndTime.value) return 0
    const ls = parseTimeToday(lunchStartTime.value)
    const le = parseTimeToday(lunchEndTime.value)
    return Math.max(0, Math.floor((le.getTime() - ls.getTime()) / 60_000))
  })

  const workedMinutes = computed(() => {
    if (!hasClockIn.value || hasClockOut.value) return null
    // 优先使用实际打卡时间，降级到配置时间
    const effectiveTime = actualClockInTime.value || clockInTime.value
    if (!effectiveTime) return null
    const start = parseTimeToday(effectiveTime)
    const diff = now.value.getTime() - start.getTime()
    if (diff < 0) return null
    const raw = Math.floor(diff / 60_000)
    // 扣除已经过去的午休时长
    if (lunchStartTime.value && lunchEndTime.value) {
      const ls = parseTimeToday(lunchStartTime.value)
      const le = parseTimeToday(lunchEndTime.value)
      if (now.value >= le) {
        // 午休已结束，扣除全部午休时长
        return Math.max(0, raw - lunchDurationMinutes.value)
      } else if (now.value >= ls) {
        // 午休进行中，扣除已过的午休时间
        const passedLunch = Math.floor((now.value.getTime() - ls.getTime()) / 60_000)
        return Math.max(0, raw - passedLunch)
      }
    }
    return raw
  })

  const countdownMinutes = computed(() => {
    if (!clockOutTime.value) return null
    const end = parseTimeToday(clockOutTime.value)
    const diff = end.getTime() - now.value.getTime()
    if (diff <= 0) return 0
    return Math.floor(diff / 60_000)
  })

  function formatMinutes(mins: number): string {
    const h = Math.floor(mins / 60)
    const m = mins % 60
    if (h > 0) return `${h}h${m.toString().padStart(2, '0')}m`
    return `${m}m`
  }

  const timeStr = computed(() =>
    `${now.value.getHours().toString().padStart(2, '0')}:${now.value.getMinutes().toString().padStart(2, '0')}`
  )

  const dateStr = computed(() => {
    const t = i18n.global.t
    const dayKeys = ['status.sunday', 'status.monday', 'status.tuesday', 'status.wednesday', 'status.thursday', 'status.friday', 'status.saturday']
    const dayName = t(dayKeys[now.value.getDay()])
    const datePart = t('status.monthDay', { month: now.value.getMonth() + 1, day: now.value.getDate() })
    return `${datePart} ${dayName}`
  })

  // ─── 番茄钟 ────────────────────────────────────────────────

  function startPomodoroCountdown(minutes: number) {
    pomodoroEndTime.value = Date.now() + minutes * 60_000
    pomodoroLeftSeconds.value = minutes * 60
    if (pomodoroTimer) clearInterval(pomodoroTimer)
    pomodoroTimer = setInterval(() => {
      const left = Math.max(0, Math.ceil((pomodoroEndTime.value - Date.now()) / 1000))
      pomodoroLeftSeconds.value = left
      if (left <= 0) {
        clearInterval(pomodoroTimer!)
        pomodoroTimer = null
        // 倒计时结束，进入 done 状态并发送系统通知
        if (pomodoroPhase.value === 'work') {
          pomodoroPhase.value = 'work-done'
          sendPomodoroNotification(i18n.global.t('status.focusEndTitle'), i18n.global.t('status.focusEndBody'))
        } else if (pomodoroPhase.value === 'break') {
          pomodoroPhase.value = 'break-done'
          sendPomodoroNotification(i18n.global.t('status.breakEndTitle'), i18n.global.t('status.breakEndBody'))
        }
      }
    }, 1000)
  }

  /** 点击番茄钟按钮 */
  function onPomodoroClick() {
    switch (pomodoroPhase.value) {
      case 'idle':
        // 开始工作
        pomodoroPhase.value = 'work'
        startPomodoroCountdown(config.value.pomodoroWork)
        break
      case 'work':
        // 工作中点击 → 取消
        pomodoroPhase.value = 'idle'
        pomodoroLeftSeconds.value = 0
        if (pomodoroTimer) { clearInterval(pomodoroTimer); pomodoroTimer = null }
        break
      case 'work-done':
        // 工作结束，点击进入休息
        pomodoroPhase.value = 'break'
        startPomodoroCountdown(config.value.pomodoroBreak)
        break
      case 'break':
        // 休息中点击 → 取消
        pomodoroPhase.value = 'idle'
        pomodoroLeftSeconds.value = 0
        if (pomodoroTimer) { clearInterval(pomodoroTimer); pomodoroTimer = null }
        break
      case 'break-done':
        // 休息结束，点击回到空闲
        pomodoroPhase.value = 'idle'
        pomodoroLeftSeconds.value = 0
        break
    }
  }

  const pomodoroDisplay = computed(() => {
    const secs = pomodoroLeftSeconds.value
    const m = Math.floor(secs / 60)
    const s = secs % 60
    return `${m}:${s.toString().padStart(2, '0')}`
  })

  function reloadHoliday() {
    loadHoliday(now.value)
  }

  return {
    config,
    saveConfig,
    reloadHoliday,
    timeStr,
    dateStr,
    holidayLabel,
    hasClockIn,
    hasClockOut,
    workedMinutes,
    countdownMinutes,
    isLunch,
    toLunchMinutes,
    lunchLeftMinutes,
    formatMinutes,
    pomodoroPhase,
    pomodoroLeftSeconds,
    pomodoroDisplay,
    onPomodoroClick,
  }
}
