<script setup lang="ts">
import { computed, ref, onMounted, onUnmounted } from 'vue'
import { useRoute } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { invoke } from '@tauri-apps/api/core'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'

const { t } = useI18n()
const route = useRoute()
const reminderType = computed(() => route.params.type as string)

const loading = ref(false)
const errorMsg = ref('')

// 打卡进度状态
const clockingMode = ref(false)
const progressStep = ref('')
const progressMessage = ref('')
const clockResult = ref<'success' | 'already-done' | 'error' | ''>('')
const resultMessage = ref('')

let unlisten: UnlistenFn | null = null

onMounted(async () => {
  // 路由渲染完成后再显示窗口，消除闪烁（窗口创建时 visible(false)）
  await getCurrentWindow().show()

  unlisten = await listen<{ step: string; message: string }>('clock-progress', (event) => {
    const { step, message } = event.payload
    progressStep.value = step
    progressMessage.value = message

    if (step === 'success' || step === 'already-done') {
      clockResult.value = step
      resultMessage.value = message
      loading.value = false
    } else if (step === 'error') {
      clockResult.value = 'error'
      resultMessage.value = message
      loading.value = false
    }
  })
})

onUnmounted(() => {
  if (unlisten) unlisten()
})

// 根据 type 决定弹窗内容
const config = computed(() => {
  switch (reminderType.value) {
    case 'clock-in':
      return {
        title: t('reminder.clockInTitle'),
        body: t('reminder.clockInBody'),
        buttons: [{ label: t('reminder.clockIn'), action: 'clockIn', primary: true }],
      }
    case 'clock-out':
      return {
        title: t('reminder.clockOutTitle'),
        body: t('reminder.clockOutBody'),
        buttons: [
          { label: t('reminder.normalClockOut'), action: 'clockOut', primary: true },
          { label: t('reminder.overtime'), action: 'overtime', primary: false },
        ],
      }
    case 'daily-report':
      return {
        title: t('reminder.dailyReportTitle'),
        body: t('reminder.dailyReportBody'),
        buttons: [{ label: t('reminder.openReport'), action: 'openReport', primary: true }],
      }
    case 'overtime':
      return {
        title: t('reminder.overtimeTitle'),
        body: t('reminder.overtimeBody'),
        buttons: [{ label: t('reminder.normalClockOut'), action: 'clockOut', primary: true }],
      }
    default:
      return {
        title: t('reminder.defaultTitle'),
        body: '',
        buttons: [{ label: t('common.close'), action: 'dismiss', primary: true }],
      }
  }
})

async function closeWindow() {
  try {
    // 如果是出勤提醒，记录今日已关闭（防止重启后补打检测再次弹出）
    if (reminderType.value === 'clock-in') {
      await invoke('dismiss_clock_in_reminder').catch(() => {})
    }
    await getCurrentWindow().close()
  } catch {
    // 忽略关闭错误
  }
}

async function handleAction(action: string) {
  loading.value = true
  errorMsg.value = ''

  try {
    switch (action) {
      case 'clockIn':
        clockingMode.value = true
        progressMessage.value = t('reminder.preparing')
        await invoke('execute_clock_action', { action: 'clock_in' })
        break

      case 'clockOut':
        clockingMode.value = true
        progressMessage.value = t('reminder.preparing')
        await invoke('execute_clock_action', { action: 'clock_out' })
        break

      case 'overtime':
        await closeWindow()
        await invoke('show_overtime_dialog')
        break

      case 'openReport':
        await invoke('open_daily_report')
        await closeWindow()
        break

      case 'showResult':
        await invoke('show_clock_webview')
        await closeWindow()
        break

      case 'dismiss':
        await closeWindow()
        break
    }
  } catch (e) {
    errorMsg.value = String(e)
    loading.value = false
  }
}
</script>

<template>
  <div class="reminder-container glass-strong" data-tauri-drag-region>
    <!-- 关闭按钮 -->
    <button class="close-btn" :title="$t('common.close')" @click="closeWindow">×</button>

    <!-- 打卡进度模式 -->
    <template v-if="clockingMode">
      <!-- 有结果了 -->
      <template v-if="clockResult">
        <p class="reminder-title">
          {{ clockResult === 'error' ? $t('reminder.clockFailed') : $t('reminder.clockComplete') }}
        </p>
        <div class="divider"></div>
        <p class="reminder-body" :class="{ 'error-body': clockResult === 'error' }">
          {{ resultMessage }}
        </p>
        <div class="btn-group">
          <template v-if="clockResult === 'error'">
            <button class="action-btn action-btn-secondary" @click="closeWindow">{{ $t('common.close') }}</button>
          </template>
          <template v-else>
            <button class="action-btn action-btn-primary" @click="closeWindow">{{ $t('common.confirm') }}</button>
            <button class="action-btn action-btn-secondary" @click="handleAction('showResult')">
              {{ $t('reminder.viewResult') }}
            </button>
          </template>
        </div>
      </template>

      <!-- 进行中 -->
      <template v-else>
        <p class="reminder-title">{{ $t('reminder.clocking') }}</p>
        <div class="divider"></div>
        <p class="progress-message">{{ progressMessage }}</p>
        <div class="progress-bar-track">
          <div class="progress-bar-fill"></div>
        </div>
      </template>
    </template>

    <!-- 正常提醒模式 -->
    <template v-else>
      <p class="reminder-title">{{ config.title }}</p>
      <div class="divider"></div>
      <p class="reminder-body">
        <template v-for="(line, i) in config.body.split('\n')" :key="i">
          {{ line }}<br v-if="i < config.body.split('\n').length - 1" />
        </template>
      </p>
      <p v-if="errorMsg" class="error-text">{{ errorMsg }}</p>
      <div class="btn-group">
        <button
          v-for="btn in config.buttons"
          :key="btn.action"
          class="action-btn"
          :class="{ 'action-btn-primary': btn.primary, 'action-btn-secondary': !btn.primary }"
          :disabled="loading"
          @click="handleAction(btn.action)"
        >
          {{ btn.label }}
        </button>
      </div>
    </template>
  </div>
</template>

<style scoped>
.reminder-container {
  width: 100vw;
  height: 100vh;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: var(--spacing-4);
  padding: var(--spacing-6);
  position: relative;
  box-sizing: border-box;
  user-select: none;
}

.close-btn {
  position: absolute;
  top: var(--spacing-3);
  right: var(--spacing-3);
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 28px;
  height: 28px;
  font-size: var(--text-xl);
  color: var(--text-tertiary);
  background: transparent;
  border: none;
  border-radius: var(--radius-sm);
  cursor: pointer;
  transition: all var(--transition-fast);
}

.close-btn:hover {
  background: var(--bg-hover);
  color: var(--text-primary);
}

.reminder-title {
  font-size: var(--text-xl);
  font-weight: var(--font-weight-heading);
  color: var(--text-primary);
}

.divider {
  width: 80%;
  height: 1px;
  background: var(--border-medium);
}

.reminder-body {
  font-size: var(--text-base);
  color: var(--text-secondary);
  text-align: center;
  line-height: 1.6;
}

.error-body {
  color: var(--color-danger);
}

.error-text {
  font-size: var(--text-sm);
  color: var(--color-danger);
}

/* 进度条 */
.progress-message {
  font-size: var(--text-base);
  color: var(--text-secondary);
  text-align: center;
}

.progress-bar-track {
  width: 80%;
  height: 4px;
  background: var(--border-medium);
  border-radius: 2px;
  overflow: hidden;
}

.progress-bar-fill {
  width: 30%;
  height: 100%;
  background: color-mix(in srgb, var(--color-primary-500) 80%, transparent);
  border-radius: 2px;
  animation: indeterminate 1.5s ease-in-out infinite;
}

@keyframes indeterminate {
  0% {
    transform: translateX(-100%);
    width: 30%;
  }
  50% {
    transform: translateX(100%);
    width: 60%;
  }
  100% {
    transform: translateX(350%);
    width: 30%;
  }
}

/* 按钮 */
.btn-group {
  display: flex;
  gap: var(--spacing-3);
  margin-top: var(--spacing-2);
}

.action-btn {
  display: inline-flex;
  align-items: center;
  height: var(--button-height);
  padding: 0 var(--spacing-5);
  font-size: var(--text-base);
  font-weight: var(--font-weight-heading);
  border-radius: var(--radius-md);
  border: none;
  cursor: pointer;
  transition: all var(--transition-fast);
}

.action-btn:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.action-btn-primary {
  background: color-mix(in srgb, var(--color-primary-500) 75%, transparent);
  backdrop-filter: blur(var(--glass-subtle-blur));
  -webkit-backdrop-filter: blur(var(--glass-subtle-blur));
  color: var(--color-neutral-0);
}

.action-btn-primary:hover:not(:disabled) {
  background: color-mix(in srgb, var(--color-primary-500) 90%, transparent);
}

.action-btn-secondary {
  background: transparent;
  color: var(--text-secondary);
  border: 1px solid var(--border-medium);
}

.action-btn-secondary:hover:not(:disabled) {
  background: var(--bg-hover);
  color: var(--text-primary);
}
</style>
