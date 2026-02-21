# TitleBar 常驻状态栏 Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 在 TitleBar 中间悬浮岛加入常驻状态栏，显示时间、日期、工时、下班倒计时、休息提醒，用户可自选展示项。

**Architecture:** 新建 `StatusBar.vue` 组件，替换 TitleBar 中间悬浮岛的 v-else 空占位（无 actions 时显示状态栏，有 actions 时状态栏也始终显示在右侧）。状态数据由新建 `useStatusBar.ts` composable 管理，从 `load_attendance_config` 读取考勤时间，节假日从公开 API 获取（当天缓存 localStorage）。用户配置（显示哪些项）存 localStorage。

**Tech Stack:** Vue 3 Composition API, Tauri invoke, fetch API（节假日），setInterval（分钟级刷新）

---

## Task 1: 创建 `useStatusBar.ts` composable

**Files:**
- Create: `src/composables/useStatusBar.ts`

### 功能说明

管理所有状态栏数据：
- 当前时间（分钟级，每分钟更新）
- 今天日期 + 星期
- 节假日信息（API 获取，当天缓存）
- 已工作时长（从 clock_in_time 算起）
- 下班倒计时（到 clock_out_time）
- 休息提醒（工作满 N 分钟变色，点击重置）
- 用户配置（显示哪些项，存 localStorage）

### 节假日 API

使用 `https://timor.tools/api/holiday/info/{YYYY-M-D}`

返回示例：
```json
{ "type": { "type": 0, "name": "工作日" }, "holiday": null }
// type: 0=工作日, 1=节假日, 2=调休
// holiday.after 表示下一个假日信息
```

当天日期变化时重新请求，结果缓存 `localStorage` key: `holiday_cache_{YYYY-MM-DD}`。

### 实现代码

```typescript
import { ref, computed, onMounted, onUnmounted } from 'vue'
import { invoke } from '@tauri-apps/api/core'

export interface StatusBarConfig {
  showTime: boolean
  showDate: boolean
  showWorked: boolean
  showCountdown: boolean
  showBreak: boolean
}

const DEFAULT_CONFIG: StatusBarConfig = {
  showTime: true,
  showDate: true,
  showWorked: true,
  showCountdown: true,
  showBreak: true,
}

const CONFIG_KEY = 'status_bar_config'
const BREAK_INTERVAL_MINUTES = 60

// 模块级单例（与 useNavigation 相同模式）
const config = ref<StatusBarConfig>(loadConfig())
const now = ref(new Date())
const clockInTime = ref<string | null>(null)   // "09:00"
const clockOutTime = ref<string | null>(null)  // "18:30"
const holidayLabel = ref<string | null>(null)  // 如 "明天休息"
const breakStartTime = ref<Date>(new Date())   // 上次重置休息计时的时间

let timer: ReturnType<typeof setInterval> | null = null
let refCount = 0

function loadConfig(): StatusBarConfig {
  try {
    const raw = localStorage.getItem(CONFIG_KEY)
    if (raw) return { ...DEFAULT_CONFIG, ...JSON.parse(raw) }
  } catch {}
  return { ...DEFAULT_CONFIG }
}

function saveConfig() {
  localStorage.setItem(CONFIG_KEY, JSON.stringify(config.value))
}

async function loadAttendanceConfig() {
  try {
    const cfg = await invoke<{
      attendance: { clock_in_time: string; clock_out_time: string }
    }>('load_attendance_config')
    clockInTime.value = cfg.attendance.clock_in_time
    clockOutTime.value = cfg.attendance.clock_out_time
  } catch {
    // 未配置考勤，相关项不显示
  }
}

async function loadHoliday(date: Date) {
  const key = `holiday_cache_${date.getFullYear()}-${date.getMonth()+1}-${date.getDate()}`
  const cached = localStorage.getItem(key)
  if (cached) {
    holidayLabel.value = JSON.parse(cached)
    return
  }
  try {
    const url = `https://timor.tools/api/holiday/info/${date.getFullYear()}-${date.getMonth()+1}-${date.getDate()}`
    const res = await fetch(url)
    const data = await res.json()
    let label: string | null = null
    if (data.type?.type === 1) label = '今天休息 🎉'
    else if (data.type?.type === 2) label = '今天调休'
    else if (data.holiday) {
      // 检查明天是否休息
      const tomorrow = new Date(date)
      tomorrow.setDate(tomorrow.getDate() + 1)
      const tKey = `holiday_cache_${tomorrow.getFullYear()}-${tomorrow.getMonth()+1}-${tomorrow.getDate()}`
      // 异步获取明天数据
      fetch(`https://timor.tools/api/holiday/info/${tomorrow.getFullYear()}-${tomorrow.getMonth()+1}-${tomorrow.getDate()}`)
        .then(r => r.json())
        .then(d => {
          if (d.type?.type === 1) {
            const tomorrow_label = '明天休息 🎉'
            localStorage.setItem(tKey, JSON.stringify(tomorrow_label))
            // 如果今天没有标签，显示明天休息
            if (!holidayLabel.value) holidayLabel.value = tomorrow_label
          }
        })
        .catch(() => {})
    }
    localStorage.setItem(key, JSON.stringify(label))
    holidayLabel.value = label
  } catch {
    // 网络失败静默忽略
  }
}

function tick() {
  const prev = now.value
  now.value = new Date()
  // 日期变化时重新加载节假日
  if (prev.getDate() !== now.value.getDate()) {
    loadHoliday(now.value)
  }
}

function startTimer() {
  if (timer) return
  // 对齐到下一分钟整点
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

// 将 "HH:MM" 解析为今天该时刻的 Date
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
      loadHoliday(now.value)
      startTimer()
    }
    refCount++
  })

  onUnmounted(() => {
    refCount--
    if (refCount === 0) stopTimer()
  })

  // 已工作时长（分钟）
  const workedMinutes = computed(() => {
    if (!clockInTime.value) return null
    const start = parseTimeToday(clockInTime.value)
    const diff = now.value.getTime() - start.getTime()
    if (diff < 0) return null
    return Math.floor(diff / 60_000)
  })

  // 下班倒计时（分钟）
  const countdownMinutes = computed(() => {
    if (!clockOutTime.value) return null
    const end = parseTimeToday(clockOutTime.value)
    const diff = end.getTime() - now.value.getTime()
    if (diff <= 0) return 0 // 已下班
    return Math.floor(diff / 60_000)
  })

  // 是否已下班
  const isOffWork = computed(() => countdownMinutes.value === 0)

  // 休息提醒：距上次重置超过 BREAK_INTERVAL_MINUTES
  const breakMinutes = computed(() => {
    const diff = now.value.getTime() - breakStartTime.value.getTime()
    return Math.floor(diff / 60_000)
  })
  const breakAlert = computed(() => breakMinutes.value >= BREAK_INTERVAL_MINUTES)

  function resetBreak() {
    breakStartTime.value = new Date()
  }

  function formatMinutes(mins: number): string {
    const h = Math.floor(mins / 60)
    const m = mins % 60
    if (h > 0) return `${h}h${m.toString().padStart(2, '0')}m`
    return `${m}m`
  }

  // 时间显示 "14:37"
  const timeStr = computed(() =>
    `${now.value.getHours().toString().padStart(2, '0')}:${now.value.getMinutes().toString().padStart(2, '0')}`
  )

  // 日期显示 "2月20日 周四"
  const dateStr = computed(() => {
    const days = ['周日', '周一', '周二', '周三', '周四', '周五', '周六']
    return `${now.value.getMonth()+1}月${now.value.getDate()}日 ${days[now.value.getDay()]}`
  })

  return {
    config,
    saveConfig,
    timeStr,
    dateStr,
    holidayLabel,
    workedMinutes,
    countdownMinutes,
    isOffWork,
    breakMinutes,
    breakAlert,
    resetBreak,
    formatMinutes,
  }
}
```

**验证：** 保存后 TypeScript 无报错即可（`vue-tsc --noEmit` 或 dev server 启动无 error）。

---

## Task 2: 创建 `StatusBar.vue` 组件

**Files:**
- Create: `src/components/StatusBar.vue`

### 布局结构

```
┌──────────────────────────────────────┐
│  14:37          [已工作 5h40m]       │
│  2月20日 周四   [下班还有 2:23]      │
│  明天休息       [休息 🔴 23m]        │
└──────────────────────────────────────┘
```

两列布局：左列（时间大字 + 日期小字 + 节假日标签），右列（胶囊纵向排列）。右上角齿轮图标点击弹出配置面板（勾选显示项）。

### 实现代码

```vue
<script setup lang="ts">
import { ref } from 'vue'
import { useStatusBar } from '../composables/useStatusBar'

const {
  config, saveConfig,
  timeStr, dateStr, holidayLabel,
  workedMinutes, countdownMinutes, isOffWork,
  breakAlert, breakMinutes, resetBreak, formatMinutes,
} = useStatusBar()

const showConfig = ref(false)

function toggleConfig() {
  showConfig.value = !showConfig.value
}

function onConfigChange() {
  saveConfig()
}
</script>

<template>
  <div class="status-bar">
    <!-- 左列：时间 + 日期 + 节假日 -->
    <div class="status-left">
      <span v-if="config.showTime" class="status-time">{{ timeStr }}</span>
      <span v-if="config.showDate" class="status-date">{{ dateStr }}</span>
      <span v-if="holidayLabel" class="status-holiday">{{ holidayLabel }}</span>
    </div>

    <!-- 右列：胶囊 -->
    <div class="status-pills">
      <span
        v-if="config.showWorked && workedMinutes !== null"
        class="pill"
      >已工作 {{ formatMinutes(workedMinutes) }}</span>

      <span
        v-if="config.showCountdown && countdownMinutes !== null"
        class="pill"
        :class="{ 'pill--done': isOffWork }"
      >{{ isOffWork ? '已下班' : `下班还有 ${formatMinutes(countdownMinutes)}` }}</span>

      <button
        v-if="config.showBreak"
        class="pill pill--btn"
        :class="{ 'pill--alert': breakAlert }"
        :title="`已工作 ${breakMinutes} 分钟，点击重置休息计时`"
        @click="resetBreak"
      >休息{{ breakAlert ? ' 🔴' : '' }} {{ breakMinutes }}m</button>
    </div>

    <!-- 右上角配置按钮 -->
    <button class="config-btn" title="配置显示项" @click="toggleConfig">
      <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
        <circle cx="12" cy="12" r="3"/>
        <path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83-2.83l.06-.06A1.65 1.65 0 0 0 4.68 15a1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 2.83-2.83l.06.06A1.65 1.65 0 0 0 9 4.68a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 2.83l-.06.06A1.65 1.65 0 0 0 19.4 9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z"/>
      </svg>
    </button>

    <!-- 配置下拉面板 -->
    <div v-if="showConfig" class="config-panel glass-medium" @click.stop>
      <label v-for="(key, label) in configLabels" :key="key" class="config-item">
        <input
          type="checkbox"
          :checked="config[key]"
          @change="config[key] = ($event.target as HTMLInputElement).checked; onConfigChange()"
        />
        {{ label }}
      </label>
    </div>
  </div>
</template>

<script lang="ts">
// configLabels 需要在 setup 外定义供模板用
const configLabels: Record<string, string> = {
  showTime: '当前时间',
  showDate: '今天日期',
  showWorked: '已工作时长',
  showCountdown: '下班倒计时',
  showBreak: '休息提醒',
}
</script>

<style scoped>
.status-bar {
  position: relative;
  display: flex;
  align-items: center;
  gap: var(--spacing-3);
  padding: var(--spacing-2) var(--spacing-4);
}

.status-left {
  display: flex;
  flex-direction: column;
  gap: 0;
  line-height: 1.2;
}

.status-time {
  font-size: var(--text-4xl);
  font-weight: var(--font-weight-bold);
  color: var(--text-primary);
  line-height: 1;
}

.status-date {
  font-size: var(--text-sm);
  color: var(--text-secondary);
}

.status-holiday {
  font-size: var(--text-xs);
  color: var(--color-success);
}

.status-pills {
  display: flex;
  flex-direction: column;
  gap: var(--spacing-1);
  align-items: flex-start;
}

.pill {
  font-size: var(--text-xs);
  padding: 1px var(--spacing-2);
  border-radius: var(--radius-full);
  background: var(--bg-hover);
  color: var(--text-secondary);
  white-space: nowrap;
  border: none;
}

.pill--btn {
  cursor: pointer;
  transition: var(--transition-all);
}

.pill--btn:hover {
  background: var(--glass-subtle-bg);
}

.pill--alert {
  background: rgba(244, 67, 54, 0.15);
  color: var(--color-danger);
}

.pill--done {
  color: var(--color-success);
}

.config-btn {
  position: absolute;
  top: var(--spacing-1);
  right: var(--spacing-2);
  display: flex;
  align-items: center;
  justify-content: center;
  width: 20px;
  height: 20px;
  border: none;
  background: transparent;
  color: var(--text-tertiary);
  cursor: pointer;
  border-radius: var(--radius-sm);
  opacity: 0;
  transition: var(--transition-all);
}

.status-bar:hover .config-btn {
  opacity: 1;
}

.config-btn:hover {
  background: var(--bg-hover);
  color: var(--text-primary);
}

.config-panel {
  position: absolute;
  top: calc(100% + var(--spacing-2));
  right: 0;
  z-index: 100;
  padding: var(--spacing-3) var(--spacing-4);
  border-radius: var(--radius-lg);
  display: flex;
  flex-direction: column;
  gap: var(--spacing-2);
  min-width: 140px;
}

.config-item {
  display: flex;
  align-items: center;
  gap: var(--spacing-2);
  font-size: var(--text-sm);
  color: var(--text-primary);
  cursor: pointer;
  white-space: nowrap;
}
</style>
```

**注意：** Vue SFC 中不能同时有两个 `<script>` 块（一个 setup，一个普通）。`configLabels` 需要移入 `<script setup>` 内作为普通常量，或单独定义在文件顶部。实现时合并为一个 `<script setup>` 块。

---

## Task 3: 修改 `TitleBar.vue` 集成 StatusBar

**Files:**
- Modify: `src/components/TitleBar.vue`

### 修改逻辑

当前 TitleBar 中间悬浮岛：
- 有 actions → 显示 action 按钮
- 无 actions → 显示 "快捷功能区" 占位

**改为：**
- 有 actions → 显示 action 按钮 **+ StatusBar 紧跟在右边单独一个悬浮岛**
- 无 actions → 只显示 StatusBar 悬浮岛

即 StatusBar **始终显示**，替换原来的 v-else 占位，同时在有 actions 时也保留。

### 修改内容

1. 在 `<script setup>` 顶部引入 StatusBar：
```typescript
import StatusBar from './StatusBar.vue'
```

2. 将模板中间区域从：
```html
<!-- 中部悬浮岛：快捷功能区 -->
<div v-if="actions.length > 0" class="title-bar-center glass-medium">
  ...actions...
</div>
<!-- 无快捷功能时：空占位保持布局 -->
<div v-else class="title-bar-center glass-medium">
  <span class="placeholder-text">快捷功能区</span>
</div>
```

改为：
```html
<!-- 中部悬浮岛：快捷功能区（有 actions 时才显示） -->
<div v-if="actions.length > 0" class="title-bar-center glass-medium">
  ...actions...（保持不变）
</div>

<!-- 常驻状态栏悬浮岛（始终显示） -->
<div class="title-bar-status glass-medium">
  <StatusBar />
</div>
```

3. 在 `<style scoped>` 中添加 `.title-bar-status` 样式（复用 `.title-bar-center` 的布局属性，但不需要 gap/padding，由 StatusBar 内部控制）：
```css
.title-bar-status {
  display: flex;
  align-items: center;
  padding: 0;
  border-radius: var(--floating-navbar-radius);
  flex-shrink: 0;
  margin-left: auto;
  margin-right: var(--spacing-5);
  margin-top: var(--spacing-3);
  -webkit-app-region: no-drag;
}
```

**注意：** 当 actions 存在时，`.title-bar-center` 的 `margin-left: auto` 要移除（改为 StatusBar 承担右推功能）。

---

## Task 4: 处理配置面板点击外关闭

**Files:**
- Modify: `src/components/StatusBar.vue`

在 `onMounted` 中添加全局 click 监听，点击 `.status-bar` 外部时关闭配置面板：

```typescript
import { ref, onMounted, onUnmounted } from 'vue'

const barEl = ref<HTMLElement | null>(null)

function onOutsideClick(e: MouseEvent) {
  if (barEl.value && !barEl.value.contains(e.target as Node)) {
    showConfig.value = false
  }
}

onMounted(() => {
  document.addEventListener('click', onOutsideClick)
})
onUnmounted(() => {
  document.removeEventListener('click', onOutsideClick)
})
```

模板中给根元素加 ref：`<div class="status-bar" ref="barEl">`

---

## Task 5: 验证

**检查清单：**

1. dev server 启动无 TypeScript/编译错误：`npm run dev`（或项目使用的命令）
2. 首页：中间悬浮岛显示时间 + 日期 + 胶囊（无 actions）
3. 项目详情页：actions 悬浮岛 + 状态栏悬浮岛同时显示
4. 悬浮 → 出现齿轮 → 点击 → 配置面板弹出
5. 取消某项 → 对应信息消失 → 刷新页面后配置保持
6. 点击休息胶囊 → 分钟重置为 0
7. 若未配置考勤，「已工作」「下班倒计时」胶囊不显示（不崩溃）
8. 节假日 API 失败 → 静默，不显示标签，不影响其他功能

---

## 布局注意事项

- 当有 actions 时，**actions 悬浮岛和状态栏悬浮岛都靠右**，用 `gap` 隔开，整体 `margin-left: auto` 在 actions 岛上（或用 flex spacer）
- `.title-bar` 是 `display: flex`，两个右侧悬浮岛并排即可
- 状态栏卡片比 action 按钮高，注意 `align-items: stretch` vs `center` 的选择（建议 center，状态栏自己撑高度）
