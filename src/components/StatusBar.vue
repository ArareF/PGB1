<script setup lang="ts">
import { ref, onMounted, onUnmounted, nextTick } from 'vue'
import { useStatusBar } from '../composables/useStatusBar'
import type { CalendarRegion } from '../composables/useStatusBar'

const {
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
  pomodoroDisplay,
  onPomodoroClick,
} = useStatusBar()

const REGION_LABELS: Record<CalendarRegion, string> = {
  auto: '自动',
  CN:   '中国',
  JP:   '日本',
  none: '不显示',
}

function onRegionChange(e: Event) {
  config.value.calendarRegion = (e.target as HTMLSelectElement).value as CalendarRegion
  saveConfig()
  reloadHoliday()
}

// 配置项标签映射（boolean 开关项）
type BoolConfigKey = 'showTime' | 'showDate' | 'showWorked' | 'showCountdown' | 'showPomodoro'
const configLabels: Record<BoolConfigKey, string> = {
  showTime:      '显示时间',
  showDate:      '显示日期',
  showWorked:    '显示已工作',
  showCountdown: '显示倒计时',
  showPomodoro:  '番茄钟',
}

// 配置面板开关
const showConfigPanel = ref(false)

// 根元素引用（用于外部点击检测 + 长按定位）
const rootEl = ref<HTMLElement | null>(null)
const panelStyle = ref({ top: '0px', right: '0px' })

const LONG_PRESS_MS = 500
let longPressTimer: ReturnType<typeof setTimeout> | null = null
let longPressTriggered = false

function onOutsideClick(event: MouseEvent) {
  if (rootEl.value && !rootEl.value.contains(event.target as Node)) {
    showConfigPanel.value = false
  }
}

function onBodyPointerDown(e: PointerEvent) {
  if (e.button !== 0) return
  longPressTriggered = false
  longPressTimer = setTimeout(async () => {
    longPressTriggered = true
    showConfigPanel.value = true
    await nextTick()
    if (rootEl.value) {
      const rect = rootEl.value.getBoundingClientRect()
      panelStyle.value = {
        top: `${rect.bottom + 8}px`,
        right: `${window.innerWidth - rect.right}px`,
      }
    }
  }, LONG_PRESS_MS)
}

function onBodyPointerUp() {
  if (longPressTimer !== null) {
    clearTimeout(longPressTimer)
    longPressTimer = null
  }
}

function onBodyPointerLeave() {
  if (longPressTimer !== null) {
    clearTimeout(longPressTimer)
    longPressTimer = null
  }
}

function onPanelClick(event: MouseEvent) {
  event.stopPropagation()
}

function toggleConfig(key: BoolConfigKey) {
  config.value[key] = !config.value[key]
  saveConfig()
}

onMounted(() => {
  document.addEventListener('click', onOutsideClick)
})

onUnmounted(() => {
  document.removeEventListener('click', onOutsideClick)
})
</script>

<template>
  <div
    ref="rootEl"
    class="status-bar"
    @pointerdown="onBodyPointerDown"
    @pointerup="onBodyPointerUp"
    @pointerleave="onBodyPointerLeave"
  >
    <!-- 主内容区：左列 + 右列 -->
    <div class="status-bar__body">
      <!-- 左列：时间 / 日期 / 节假日标签 -->
      <div class="status-bar__left">
        <span v-if="config.showTime" class="status-bar__time">{{ timeStr }}</span>
        <span v-if="config.showDate" class="status-bar__date">{{ dateStr }}</span>
        <span v-if="holidayLabel" class="status-bar__holiday-tag">{{ holidayLabel }}</span>
      </div>

      <!-- 右列：胶囊组 -->
      <div class="status-bar__right">
        <!-- 已工作胶囊 -->
        <div
          v-if="config.showWorked && workedMinutes !== null"
          class="status-bar__capsule status-bar__capsule--worked"
        >
          已工作 {{ formatMinutes(workedMinutes) }}
        </div>

        <!-- 倒计时胶囊：未打卡不显示 -->
        <div
          v-if="config.showCountdown && hasClockIn"
          class="status-bar__capsule"
          :class="{
            'status-bar__capsule--lunch': isLunch,
            'status-bar__capsule--off': hasClockOut,
            'status-bar__capsule--countdown': !isLunch && !hasClockOut,
          }"
        >
          <template v-if="hasClockOut">下班咯</template>
          <template v-else-if="isLunch">
            午休{{ lunchLeftMinutes ? ` ${formatMinutes(lunchLeftMinutes)}` : '中' }}
          </template>
          <template v-else-if="toLunchMinutes !== null">
            距午休 {{ formatMinutes(toLunchMinutes) }}
          </template>
          <template v-else-if="countdownMinutes !== null">
            距下班 {{ formatMinutes(countdownMinutes) }}
          </template>
        </div>

      </div>

      <!-- 番茄钟按钮 -->
      <button
        v-if="config.showPomodoro"
        class="status-bar__pomodoro"
        :class="{
          'status-bar__pomodoro--work': pomodoroPhase === 'work',
          'status-bar__pomodoro--work-done': pomodoroPhase === 'work-done',
          'status-bar__pomodoro--break': pomodoroPhase === 'break',
          'status-bar__pomodoro--break-done': pomodoroPhase === 'break-done',
        }"
        :title="pomodoroPhase === 'idle' ? '开始专注' : pomodoroPhase === 'work' ? '点击取消' : pomodoroPhase === 'work-done' ? '开始休息' : pomodoroPhase === 'break' ? '点击取消' : '结束休息'"
        @click.stop="onPomodoroClick"
      >
        <template v-if="pomodoroPhase === 'idle'">专注</template>
        <template v-else-if="pomodoroPhase === 'work'">{{ pomodoroDisplay }}</template>
        <template v-else-if="pomodoroPhase === 'work-done'">休息</template>
        <template v-else-if="pomodoroPhase === 'break'">{{ pomodoroDisplay }}</template>
        <template v-else-if="pomodoroPhase === 'break-done'">完成</template>
      </button>
    </div>

    <!-- 配置面板 — Teleport 到 body 避免被父层 overflow 裁剪 -->
    <Teleport to="body">
      <div
        v-if="showConfigPanel"
        class="status-bar__config-panel"
        :style="panelStyle"
        @click="onPanelClick"
      >
        <div class="status-bar__config-title">状态栏配置</div>
        <label
          v-for="[key, label] in (Object.entries(configLabels) as [BoolConfigKey, string][])"
          :key="key"
          class="status-bar__config-item"
        >
          <input
            type="checkbox"
            :checked="config[key]"
            @change="toggleConfig(key)"
          />
          <span>{{ label }}</span>
        </label>
        <template v-if="config.showPomodoro">
          <div class="status-bar__config-divider" />
          <label class="status-bar__config-item">
            <span>工作</span>
            <input
              type="number"
              :value="config.pomodoroWork"
              min="1"
              max="120"
              class="status-bar__config-number"
              @change="config.pomodoroWork = Number(($event.target as HTMLInputElement).value); saveConfig()"
            />
            <span>分钟</span>
          </label>
          <label class="status-bar__config-item">
            <span>休息</span>
            <input
              type="number"
              :value="config.pomodoroBreak"
              min="1"
              max="60"
              class="status-bar__config-number"
              @change="config.pomodoroBreak = Number(($event.target as HTMLInputElement).value); saveConfig()"
            />
            <span>分钟</span>
          </label>
        </template>
        <div class="status-bar__config-divider" />
        <label class="status-bar__config-item">
          <span>假日日历</span>
          <select
            class="status-bar__config-select"
            :value="config.calendarRegion"
            @change="onRegionChange($event)"
          >
            <option v-for="(label, key) in REGION_LABELS" :key="key" :value="key">{{ label }}</option>
          </select>
        </label>
      </div>
    </Teleport>
  </div>
</template>

<style scoped>
/* =============================================
   StatusBar 根容器
   ============================================= */
.status-bar {
  position: relative;
  display: flex;
  align-items: center;
  gap: var(--spacing-2);
  padding: var(--padding-xs) var(--padding-sm);
  border-radius: var(--radius-lg);
  background: transparent;
}

/* =============================================
   主体：左右两列
   ============================================= */
.status-bar__body {
  display: flex;
  align-items: center;
  gap: var(--spacing-3);
  flex: 1;
  min-width: 0;
}

/* =============================================
   左列：时间 / 日期 / 节假日
   ============================================= */
.status-bar__left {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: var(--spacing-1);
  line-height: var(--leading-tight);
}

.status-bar__time {
  font-size: var(--text-xl);
  font-weight: var(--font-semibold);
  color: var(--text-primary);
  letter-spacing: var(--tracking-tight);
  line-height: var(--leading-none);
}

.status-bar__date {
  font-size: var(--text-xs);
  font-weight: var(--font-normal);
  color: var(--text-secondary);
  line-height: var(--leading-none);
}

.status-bar__holiday-tag {
  font-size: var(--text-xs);
  font-weight: var(--font-normal);
  color: var(--color-success);
  line-height: var(--leading-none);
  white-space: nowrap;
}

/* =============================================
   右列：胶囊纵向排列
   ============================================= */
.status-bar__right {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: var(--spacing-1);
}

/* =============================================
   胶囊通用样式
   ============================================= */
.status-bar__capsule {
  display: inline-flex;
  align-items: center;
  padding: var(--padding-xs) var(--padding-sm);
  border-radius: var(--radius-tag);
  font-size: var(--text-xs);
  font-weight: var(--font-medium);
  white-space: nowrap;
  line-height: var(--leading-none);
  background: var(--bg-hover);
  color: var(--text-secondary);
  transition: background 0.2s ease, color 0.2s ease;
}

/* 已工作胶囊 */
.status-bar__capsule--worked {
  color: var(--color-info);
}

/* 下班倒计时胶囊 */
.status-bar__capsule--countdown {
  color: var(--color-success);
}

/* 下班胶囊 */
.status-bar__capsule--off {
  color: var(--text-tertiary);
}

/* 午休胶囊 */
.status-bar__capsule--lunch {
  background: color-mix(in srgb, var(--color-orange) 15%, transparent);
  color: var(--color-orange);
}

/* =============================================
   番茄钟按钮 — 无形态，纯光晕
   ============================================= */
.status-bar__pomodoro {
  position: relative;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  padding: var(--spacing-2) var(--spacing-4);
  border: none;
  background: transparent;
  font-size: var(--text-2xl);
  font-weight: var(--font-medium);
  line-height: var(--leading-none);
  white-space: nowrap;
  color: var(--text-secondary);
  cursor: pointer;
  flex-shrink: 0;
  font-variant-numeric: tabular-nums;
  height: calc(var(--text-2xl) + var(--spacing-2) * 2);
  transition: color 0.3s ease;
  isolation: isolate;
}

/* 光晕层 */
.status-bar__pomodoro::before {
  content: '';
  position: absolute;
  inset: -4px -8px;
  border-radius: 50%;
  background: #ffffff;
  filter: blur(16px);
  opacity: 0.08;           /* 空闲：白色极淡光晕 */
  z-index: -1;
  pointer-events: none;
  transition: opacity 0.4s ease, background 0.4s ease;
}

.status-bar__pomodoro:hover::before {
  opacity: 0.14;
}

/* 专注中：蓝色 */
.status-bar__pomodoro--work {
  color: var(--color-primary-500);
}
.status-bar__pomodoro--work::before {
  background: var(--color-primary-500);
  opacity: 0.45;
}

/* 超时：红绿交替 */
.status-bar__pomodoro--work-done {
  color: var(--text-primary);
}
.status-bar__pomodoro--work-done::before {
  animation: pomodoro-glow-alert 1.2s ease-in-out infinite;
}

/* 休息中：绿色 */
.status-bar__pomodoro--break {
  color: var(--color-success);
}
.status-bar__pomodoro--break::before {
  background: var(--color-success);
  opacity: 0.45;
}

/* 休息结束：绿色呼吸 */
.status-bar__pomodoro--break-done {
  color: var(--color-success);
}
.status-bar__pomodoro--break-done::before {
  background: var(--color-success);
  animation: pomodoro-glow-breathe 1.5s ease-in-out infinite;
}

@keyframes pomodoro-glow-alert {
  0%, 100% {
    background: var(--color-danger);
    opacity: 0.55;
    transform: scale(1.0);
  }
  50% {
    background: var(--color-success);
    opacity: 0.55;
    transform: scale(1.15);
  }
}

@keyframes pomodoro-glow-breathe {
  0%, 100% {
    opacity: 0.15;
    transform: scale(0.85);
  }
  50% {
    opacity: 0.55;
    transform: scale(1.15);
  }
}

</style>

<style>
/* 配置面板 — Teleport 到 body，必须用全局样式 */
.status-bar__config-panel {
  position: fixed;
  z-index: var(--z-dropdown);
  min-width: 160px;
  padding: var(--padding-sm);
  border-radius: var(--radius-md);
  background: var(--glass-medium-bg);
  border: 1px solid var(--border-light);
  box-shadow: var(--shadow-lg);
  backdrop-filter: blur(var(--panel-blur));
  -webkit-backdrop-filter: blur(var(--panel-blur));
  display: flex;
  flex-direction: column;
  gap: var(--spacing-1);
}

.status-bar__config-title {
  font-size: var(--text-xs);
  font-weight: var(--font-semibold);
  color: var(--text-secondary);
  padding: var(--padding-xs) var(--padding-xs);
  margin-bottom: var(--spacing-1);
  border-bottom: 1px solid var(--border-light);
}

.status-bar__config-item {
  display: flex;
  align-items: center;
  gap: var(--spacing-2);
  padding: var(--padding-xs) var(--padding-xs);
  border-radius: var(--radius-sm);
  font-size: var(--text-xs);
  color: var(--text-primary);
  cursor: pointer;
  user-select: none;
  transition: background 0.15s ease;
}

.status-bar__config-item:hover {
  background: var(--bg-hover);
}

.status-bar__config-item input[type='checkbox'] {
  width: 14px;
  height: 14px;
  accent-color: var(--color-primary-500);
  cursor: pointer;
  flex-shrink: 0;
}

.status-bar__config-divider {
  height: 1px;
  background: var(--border-light);
  margin: var(--spacing-1) 0;
}

.status-bar__config-number {
  width: 48px;
  padding: 2px 4px;
  border: 1px solid var(--border-light);
  border-radius: var(--radius-sm);
  background: transparent;
  color: var(--text-primary);
  font-size: var(--text-xs);
  text-align: center;
  outline: none;
}

.status-bar__config-number:focus {
  border-color: var(--color-primary-500);
}

/* 隐藏 number 输入框的上下箭头 */
.status-bar__config-number::-webkit-inner-spin-button,
.status-bar__config-number::-webkit-outer-spin-button {
  -webkit-appearance: none;
  margin: 0;
}

.status-bar__config-select {
  margin-left: auto;
  padding: 2px 4px;
  border: 1px solid var(--border-light);
  border-radius: var(--radius-sm);
  background: var(--bg-hover);
  color: var(--text-primary);
  font-size: var(--text-xs);
  outline: none;
  cursor: pointer;
}

.status-bar__config-select:focus {
  border-color: var(--color-primary-500);
}
</style>
