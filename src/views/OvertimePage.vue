<script setup lang="ts">
import { ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { invoke } from '@tauri-apps/api/core'
import { getCurrentWindow } from '@tauri-apps/api/window'

useI18n()

const customTime = ref('')
const loading = ref(false)

async function closeWindow() {
  try {
    await getCurrentWindow().close()
  } catch {
    // 忽略关闭错误
  }
}

async function scheduleOvertime(minutes: number) {
  loading.value = true
  try {
    await invoke('schedule_overtime_reminder', { minutes })
    await closeWindow()
  } catch (e) {
    console.error('设置加班提醒失败:', e)
    loading.value = false
  }
}

async function handleCustom() {
  if (!customTime.value) return

  const [h, m] = customTime.value.split(':').map(Number)
  const now = new Date()
  const targetMinutes = h * 60 + m
  const currentMinutes = now.getHours() * 60 + now.getMinutes()

  let diffMinutes = targetMinutes - currentMinutes
  if (diffMinutes <= 0) {
    // 如果选的时间已过，当作明天
    diffMinutes += 24 * 60
  }

  await scheduleOvertime(diffMinutes)
}
</script>

<template>
  <div class="overtime-container glass-strong" data-tauri-drag-region>
    <!-- 关闭按钮 -->
    <button class="close-btn" :title="$t('common.close')" @click="closeWindow">
      ×
    </button>

    <!-- 标题 -->
    <p class="overtime-title">{{ $t('overtime.title') }}</p>

    <!-- 分隔线 -->
    <div class="divider"></div>

    <!-- 快捷按钮 -->
    <div class="quick-btns">
      <button class="quick-btn" :disabled="loading" @click="scheduleOvertime(30)">
        {{ $t('overtime.plus30min') }}
      </button>
      <button class="quick-btn" :disabled="loading" @click="scheduleOvertime(60)">
        {{ $t('overtime.plus1hour') }}
      </button>
      <button class="quick-btn" :disabled="loading" @click="scheduleOvertime(120)">
        {{ $t('overtime.plus2hours') }}
      </button>
    </div>

    <!-- 自定义时间 -->
    <div class="custom-row">
      <span class="custom-label">{{ $t('overtime.custom') }}</span>
      <input
        v-model="customTime"
        class="custom-input"
        type="time"
        :disabled="loading"
      />
      <button
        class="confirm-btn"
        :disabled="loading || !customTime"
        @click="handleCustom"
      >
        {{ $t('common.ok') }}
      </button>
    </div>
  </div>
</template>

<style scoped>
.overtime-container {
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

.overtime-title {
  font-size: var(--text-xl);
  font-weight: var(--font-weight-heading);
  color: var(--text-primary);
}

.divider {
  width: 80%;
  height: 1px;
  background: var(--border-medium);
}

.quick-btns {
  display: flex;
  gap: var(--spacing-3);
}

.quick-btn {
  display: inline-flex;
  align-items: center;
  height: var(--button-height);
  padding: 0 var(--spacing-4);
  font-size: var(--text-base);
  font-weight: var(--font-weight-heading);
  color: var(--text-primary);
  background: var(--bg-elevated);
  border: 1px solid var(--border-medium);
  border-radius: var(--radius-md);
  cursor: pointer;
  transition: all var(--transition-fast);
}

.quick-btn:hover:not(:disabled) {
  background: color-mix(in srgb, var(--color-primary-500) 75%, transparent);
  backdrop-filter: blur(var(--glass-subtle-blur));
  -webkit-backdrop-filter: blur(var(--glass-subtle-blur));
  color: var(--color-neutral-0);
  border-color: transparent;
}

.quick-btn:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.custom-row {
  display: flex;
  align-items: center;
  gap: var(--spacing-3);
}

.custom-label {
  font-size: var(--text-sm);
  color: var(--text-secondary);
}

.custom-input {
  height: var(--button-height);
  padding: 0 var(--spacing-3);
  font-size: var(--text-sm);
  color: var(--text-primary);
  background: var(--bg-elevated);
  border: 1px solid var(--border-medium);
  border-radius: var(--radius-md);
  outline: none;
  transition: border-color var(--transition-fast);
}

.custom-input:focus {
  border-color: var(--color-primary);
}

.confirm-btn {
  display: inline-flex;
  align-items: center;
  height: var(--button-height);
  padding: 0 var(--spacing-4);
  font-size: var(--text-base);
  font-weight: var(--font-weight-heading);
  background: color-mix(in srgb, var(--color-primary-500) 75%, transparent);
  backdrop-filter: blur(var(--glass-subtle-blur));
  -webkit-backdrop-filter: blur(var(--glass-subtle-blur));
  color: var(--color-neutral-0);
  border: none;
  border-radius: var(--radius-md);
  cursor: pointer;
  transition: all var(--transition-fast);
}

.confirm-btn:hover:not(:disabled) {
  background: color-mix(in srgb, var(--color-primary-500) 90%, transparent);
}

.confirm-btn:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}
</style>
